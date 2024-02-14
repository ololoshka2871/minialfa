mod controller;
mod display;
mod i2c_sensor;
mod klapan;
mod linear_regression;
mod support;
mod thyracont_sensor;

use crossbeam::channel::Sender;

use esp_idf_hal::gpio::{AnyIOPin, PinDriver};
use esp_idf_hal::gpio::{InputPin, OutputPin};
use esp_idf_hal::peripheral::Peripheral;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::timer::EspTimerService;

use std::time::Duration;
use std::time::Instant;

use esp_idf_hal::delay;
use esp_idf_hal::i2c;
use esp_idf_hal::i2c::I2cError;
use esp_idf_hal::prelude::*;
use esp_idf_hal::spi;
use esp_idf_hal::uart;

use thiserror::Error;

use esp_idf_sys as _;

#[derive(Error, Debug)]
pub enum FormatError {
    #[error("Empty responce")]
    EmptyResponce,
}

fn main() {
    // Temporary. Will disappear once ESP-IDF 4.4 is released, but for now it is necessary to call this function once,
    // or else some patches to the runtime implemented by esp-idf-sys might not link properly.
    esp_idf_sys::link_patches();

    let dp = Peripherals::take().unwrap();
    let timer_service = EspTimerService::new().unwrap();

    let mut controller = controller::Controller::new();

    let mut klapan = klapan::Klapan::new(PinDriver::output(dp.pins.gpio12).unwrap());

    println!("Initialising rotary encoder");
    let _encoder = create_encoder(
        PinDriver::input(dp.pins.gpio32).unwrap(),
        PinDriver::input(dp.pins.gpio33).unwrap(),
        PinDriver::input(dp.pins.gpio19).unwrap(),
        controller.command_chanel(),
        &timer_service,
    )
    .expect("Failed to create encoder");

    println!("Initialising SCTB sensors...");
    let mut sensors_timer = create_sensors(
        dp.i2c0,
        dp.pins.gpio26,
        dp.pins.gpio25,
        controller.sensor_chanel(),
        &timer_service,
    )
    .expect("Failed to create SCTB sensors");

    println!("Initialising Thyracont Sensor...");
    let res = {
        let config = uart::config::Config::new().baudrate(Hertz(9600));
        let uart = uart::UartDriver::new(
            dp.uart2,
            dp.pins.gpio17,
            dp.pins.gpio16,
            Option::<AnyIOPin>::None,
            Option::<AnyIOPin>::None,
            &config,
        )
        .unwrap();
        let mut re_de = PinDriver::output(dp.pins.gpio2).unwrap();
        re_de.set_low().unwrap();
        create_thyracont_sensor(uart, 1, re_de, controller.sensor_chanel(), &timer_service)
    };

    let mut thyracont_update_timer = match res {
        Ok((timer, addr)) => {
            println!("Thyracont Sensor found at address {addr}!");
            Some(timer)
        }
        Err(e) => {
            println!("Thyracont Sensor not found: {e}");
            None
        }
    };

    println!("Initialising display...");
    create_display(
        dp.spi2,
        dp.pins.gpio18,
        dp.pins.gpio23,
        dp.pins.gpio5.downgrade_output(),
        PinDriver::output(dp.pins.gpio21).unwrap(),
        PinDriver::output(dp.pins.gpio22).unwrap(),
        controller.display_chanel(),
    )
    .expect("Failed to create display");

    println!("Ready!");

    loop {
        controller.poll(&mut sensors_timer, &mut thyracont_update_timer, &mut klapan);
    }
}

fn create_encoder<'a, V1, V2, BTN>(
    mut v1: PinDriver<'static, V1, esp_idf_hal::gpio::Input>,
    mut v2: PinDriver<'static, V2, esp_idf_hal::gpio::Input>,
    mut btn: PinDriver<'static, BTN, esp_idf_hal::gpio::Input>,
    encoder_ch: Sender<controller::EncoderCommand>,
    timer_svc: &'a esp_idf_svc::timer::EspTaskTimerService,
) -> anyhow::Result<esp_idf_svc::timer::EspTimer<'a>>
where
    V1: InputPin + OutputPin,
    V2: InputPin + OutputPin,
    BTN: InputPin + OutputPin,
{
    let mut prev_btn_state = false;

    v1.set_pull(esp_idf_hal::gpio::Pull::Up)?;
    v2.set_pull(esp_idf_hal::gpio::Pull::Up)?;
    btn.set_pull(esp_idf_hal::gpio::Pull::Up)?;

    let mut enc = rotary_encoder_embedded::RotaryEncoder::new(v1, v2).into_standard_mode();

    let timer = timer_svc.timer(move || {
        use controller::EncoderCommand;
        use rotary_encoder_embedded::Direction;

        let now = Instant::now();

        let new_btn_state = btn.is_low();
        if new_btn_state != prev_btn_state {
            let cmd = match new_btn_state {
                true => EncoderCommand::Push,
                false => EncoderCommand::Pull,
            };
            if let Err(e) = encoder_ch.send_deadline(cmd, now + Duration::from_millis(1)) {
                println!("Failed to send button state: {e}");
            } else {
                prev_btn_state = new_btn_state;
            }
        }
        enc.update();

        let cmd = match enc.direction() {
            Direction::Clockwise => EncoderCommand::Increment,
            Direction::Anticlockwise => EncoderCommand::Decrement,
            Direction::None => return,
        };

        if let Err(e) = encoder_ch.send_deadline(cmd, now + Duration::from_millis(1)) {
            println!("Failed to send encoder event: {e}");
        }
    })?;

    timer.every(Duration::from_millis(2)).unwrap();

    Ok(timer)
}

fn create_sensors<'d, I2C>(
    i2c0: impl Peripheral<P = I2C> + 'static,
    sda: impl Peripheral<P = impl InputPin + OutputPin> + 'static,
    scl: impl Peripheral<P = impl InputPin + OutputPin> + 'static,
    sensor_channel: Sender<controller::SensorResult>,
    timer_svc: &esp_idf_svc::timer::EspTaskTimerService,
) -> anyhow::Result<esp_idf_svc::timer::EspTimer>
where
    I2C: i2c::I2c,
{
    /*
    extern "C" {
        fn i2c_set_timeout(
            i2c_num: esp_idf_sys::i2c_port_t,
            timeout: std::ffi::c_int,
        ) -> esp_idf_sys::esp_err_t;

        #[allow(unused)]
        fn i2c_get_timeout(
            i2c_num: esp_idf_sys::i2c_port_t,
            timeout: *mut std::ffi::c_int,
        ) -> esp_idf_sys::esp_err_t;
    }
    */

    fn print_read_failed(addr: u8, e: I2cError) {
        println!("Failed to read I2C sensor at {addr}: {e}");
    }

    let config = i2c::I2cConfig::new()
        .baudrate(100.kHz().into())
        .timeout(Duration::from_millis(5).into());
    let mut i2c = i2c::I2cDriver::new(i2c0, sda, scl, &config)?;

    /*
    unsafe {
        //let mut ct: esp_idf_sys::c_types::c_int = 0;
        //i2c_get_timeout(0, &mut ct);
        //println!("Current i2c strech timout: {}", ct);
        i2c_set_timeout(0, 50000);
    }
    */

    let p_sensor = i2c_sensor::I2CSensor::new(15);
    let f_sensor = i2c_sensor::I2CSensor::new(12);

    let timer = timer_svc.timer(move || {
        let p = match p_sensor.read(&mut i2c) {
            Ok(v) => v.pressure,
            Err(e) => {
                print_read_failed(p_sensor.address(), e);
                return;
            }
        };

        let f = match f_sensor.read(&mut i2c) {
            Ok(v) => v.f_p,
            Err(e) => {
                print_read_failed(f_sensor.address(), e);
                return;
            }
        };

        let now = Instant::now();
        if let Err(e) = sensor_channel.send_deadline(
            controller::SensorResult::SctbSensorResult { f, p },
            now + Duration::from_millis(1),
        ) {
            println!("Failed to send sensor result: {}", e);
        }
    })?;

    Ok(timer)
}

fn create_thyracont_sensor<P, E, PIN, PINE>(
    mut serial: P,
    addr: u8,
    mut re_de: PIN,
    sensor_channel: Sender<controller::SensorResult>,
    timer_svc: &esp_idf_svc::timer::EspTaskTimerService,
) -> anyhow::Result<(esp_idf_svc::timer::EspTimer, u8)>
where
    P: embedded_hal::serial::Read<u8, Error = E>
        + embedded_hal::serial::Write<u8, Error = E>
        + Send
        + 'static,
    PIN: embedded_hal::digital::v2::OutputPin<Error = PINE> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    let sensor = thyracont_sensor::TyracontSensor::new(addr);

    match sensor.get_id(&mut serial, &mut re_de) {
        Ok(Some(id)) => println!("Sensor id: {id}"),
        Ok(None) => Err(FormatError::EmptyResponce)?,
        Err(e) => Err(e)?,
    }

    let timer = timer_svc.timer(move || {
        let p = match sensor.read(&mut serial, &mut re_de) {
            Ok(Some(v)) => v,
            Ok(None) => return,
            Err(e) => {
                println!("Failed to read TyracontSensor: {e}");
                return;
            }
        };

        let now = Instant::now();
        if let Err(e) = sensor_channel.send_deadline(
            controller::SensorResult::TyracontSensorResult { p },
            now + Duration::from_millis(1),
        ) {
            println!("Failed to send TyracontSensor sensor result: {e}");
        }
    })?;

    Ok((timer, addr))
}

fn create_display<'d, SPI, DC, RESET, E>(
    spi: impl Peripheral<P = SPI> + 'static,
    sclk: impl Peripheral<P = impl OutputPin> + 'static,
    sdo: impl Peripheral<P = impl OutputPin> + 'static,
    cs: impl Peripheral<P = impl OutputPin> + 'static,
    dc: DC,
    mut reset: RESET,
    disp_channel: crossbeam::channel::Receiver<controller::DisplayCommand>,
) -> anyhow::Result<()>
where
    SPI: spi::SpiAnyPins,
    DC: embedded_hal::digital::v2::OutputPin<Error = E> + Send + 'static,
    RESET: embedded_hal::digital::v2::OutputPin<Error = E>,
    E: std::error::Error + Send + Sync + 'static,
{
    let config = spi::config::Config::new()
        .write_only(true)
        // mode 0 - defailt
        .baudrate(10.MHz().into());

    let di = display_interface_spi::SPIInterfaceNoCS::new(
        spi::SpiDeviceDriver::new_single(
            spi,
            sclk,
            sdo,
            Option::<AnyIOPin>::None,
            Some(cs),
            &spi::config::DriverConfig::default(),
            &config,
        )
        .expect("Failed to create spi device"),
        dc, // DC
    );

    let mut disp: ssd1309::prelude::GraphicsMode<_> = ssd1309::Builder::new().connect(di).into();
    {
        let mut delay_provider = delay::FreeRtos {};
        disp.reset(&mut reset, &mut delay_provider)?;
    }
    disp.init().unwrap();

    std::thread::Builder::new()
        .stack_size(12 * 1024)
        .name("Display".to_string())
        .spawn(move || display::dispaly_thread(disp, disp_channel))?;

    Ok(())
}
