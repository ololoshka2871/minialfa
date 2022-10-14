mod controller;
mod display;
mod i2c_sensor;

use crossbeam::channel::Sender;
use embedded_hal::digital::v2::InputPin;
use embedded_hal::digital::v2::OutputPin;
use embedded_svc::timer::PeriodicTimer;

use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::timer::EspTimerService;

use std::time::Duration;
use std::time::Instant;

use esp_idf_hal::delay;
use esp_idf_hal::gpio::Input;
use esp_idf_hal::gpio::Pull;
use esp_idf_hal::i2c;
use esp_idf_hal::i2c::I2cError;
use esp_idf_hal::prelude::*;
use esp_idf_hal::spi;

use embedded_svc::timer::TimerService;

use esp_idf_sys as _;

fn main() {
    // Temporary. Will disappear once ESP-IDF 4.4 is released, but for now it is necessary to call this function once,
    // or else some patches to the runtime implemented by esp-idf-sys might not link properly.
    esp_idf_sys::link_patches();

    let dp = Peripherals::take().unwrap();
    let mut timer_service = EspTimerService::new().unwrap();

    let mut controller = controller::Controller::new();

    println!("Initialising rotary encoder");
    let _encoder = create_encoder(
        dp.pins.gpio16.into_input().unwrap(),
        dp.pins.gpio4.into_input().unwrap(),
        dp.pins.gpio19.into_input().unwrap(),
        controller.command_chanel(),
        &mut timer_service,
    )
    .expect("Failed to create encoder");

    println!("Initialising sensors...");
    let _sensors = crate_sensors(
        dp.i2c0,
        i2c::MasterPins {
            sda: dp.pins.gpio26,
            scl: dp.pins.gpio25,
        },
        controller.sensor_chanel(),
        &mut timer_service,
    )
    .expect("Failed to create sensors");

    println!("Initialising display...");
    create_display(
        dp.spi2,
        spi::Pins {
            sclk: dp.pins.gpio18,
            sdo: dp.pins.gpio23,
            sdi: None,
            cs: Some(dp.pins.gpio5.into_output().unwrap()),
        },
        dp.pins.gpio21.into_output().unwrap(),
        dp.pins.gpio22.into_output().unwrap(),
        controller.display_chanel(),
    )
    .expect("Failed to create display");

    println!("Ready!");

    loop {
        controller.poll();
    }
}

fn create_encoder<V1, V2, BTN, E>(
    mut v1: V1,
    mut v2: V2,
    mut btn: BTN,
    encoder_ch: Sender<controller::EncoderCommand>,
    timer_svc: &mut esp_idf_svc::timer::EspTaskTimerService,
) -> anyhow::Result<esp_idf_svc::timer::EspTimer>
where
    V1: InputPin<Error = E> + Pull<Error = E> + Send + 'static,
    V2: InputPin<Error = E> + Pull<Error = E> + Send + 'static,
    BTN: InputPin<Error = E> + Pull<Error = E> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    let mut prev_btn_state = false;

    v1.set_pull_up()?;
    v2.set_pull_up()?;
    btn.set_pull_up()?;

    let mut enc = rotary_encoder_embedded::RotaryEncoder::new(v1, v2).into_standard_mode();

    let mut timer = timer_svc.timer(move || {
        use controller::EncoderCommand;
        use rotary_encoder_embedded::Direction;

        let now = Instant::now();

        let new_btn_state = btn.is_low().unwrap_or_default();
        if new_btn_state != prev_btn_state {
            let cmd = match new_btn_state {
                true => EncoderCommand::Push,
                false => EncoderCommand::Pull,
            };
            if let Err(e) = encoder_ch.send_deadline(cmd, now + Duration::from_millis(1)) {
                println!("Failed to send button state: {}", e);
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
            println!("Failed to send encoder event: {}", e);
        }
    })?;

    timer.every(Duration::from_millis(10)).unwrap();

    Ok(timer)
}

fn crate_sensors<I2C, SDA, SCL>(
    i2c0: I2C,
    pins: i2c::MasterPins<SDA, SCL>,
    sensor_channel: Sender<controller::SensorResult>,
    timer_svc: &mut esp_idf_svc::timer::EspTaskTimerService,
) -> anyhow::Result<esp_idf_svc::timer::EspTimer>
where
    I2C: i2c::I2c + Send + 'static,
    SDA: esp_idf_hal::gpio::OutputPin + esp_idf_hal::gpio::InputPin + Send + 'static,
    SCL: esp_idf_hal::gpio::OutputPin + esp_idf_hal::gpio::InputPin + Send + 'static,
{
    extern "C" {
        fn i2c_set_timeout(
            i2c_num: esp_idf_sys::i2c_port_t,
            timeout: esp_idf_sys::c_types::c_int,
        ) -> esp_idf_sys::esp_err_t;

        #[allow(unused)]
        fn i2c_get_timeout(
            i2c_num: esp_idf_sys::i2c_port_t,
            timeout: *mut esp_idf_sys::c_types::c_int,
        ) -> esp_idf_sys::esp_err_t;
    }

    fn print_read_failed(addr: u8, e: I2cError) {
        println!("Failed to read I2C sensor at {}: {}", addr, e);
    }

    let config = i2c::config::MasterConfig::new().baudrate(100.kHz().into());
    let mut i2c = i2c::Master::new(i2c0, pins, config)?;

    unsafe {
        //let mut ct: esp_idf_sys::c_types::c_int = 0;
        //i2c_get_timeout(0, &mut ct);
        //println!("Current i2c strech timout: {}", ct);
        i2c_set_timeout(0, 50000);
    }

    let p_sensor = i2c_sensor::I2CSensor::new(11);
    let f_sensor = i2c_sensor::I2CSensor::new(12);

    let mut timer = timer_svc.timer(move || {
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
            controller::SensorResult { f, p },
            now + Duration::from_millis(1),
        ) {
            println!("Failed to send sensor result: {}", e);
        }
    })?;

    timer.every(Duration::from_millis(100)).unwrap();

    Ok(timer)
}

fn create_display<SCLK, SDO, CS, DC, RESET, E>(
    spi: spi::SPI2,
    pins: spi::Pins<SCLK, SDO, esp_idf_hal::gpio::Gpio0<esp_idf_hal::gpio::Input>, CS>,
    dc: DC,
    mut reset: RESET,
    disp_channel: crossbeam::channel::Receiver<controller::DisplayCommand>,
) -> anyhow::Result<()>
where
    SCLK: esp_idf_hal::gpio::OutputPin + Send + 'static,
    SDO: esp_idf_hal::gpio::OutputPin + Send + 'static,
    CS: esp_idf_hal::gpio::OutputPin + Send + 'static,
    DC: OutputPin + Send + 'static,
    RESET: OutputPin<Error = E>,
    E: std::error::Error + Send + Sync + 'static,
{
    let config = spi::config::Config::new()
        .write_only(true)
        // mode 0 - defailt
        .baudrate(1.MHz().into());

    let di = display_interface_spi::SPIInterfaceNoCS::new(
        spi::Master::<
            spi::SPI2,
            _,
            _,
            esp_idf_hal::gpio::Gpio0<Input>, // заглушка
            _,
        >::new(spi, pins, config)
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
        .stack_size(10 * 1024)
        .name("Display".to_string())
        .spawn(move || display::dispaly_thread(disp, disp_channel))?;

    Ok(())
}
