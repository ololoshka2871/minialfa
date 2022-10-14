mod display;
mod i2c_sensor;

use embedded_hal::digital::v2::InputPin;
use embedded_svc::timer::PeriodicTimer;
use esp_idf_hal::peripherals::Peripherals;

use std::time::Duration;

use esp_idf_hal::delay;
use esp_idf_hal::gpio::Input;
use esp_idf_hal::gpio::Output;
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

    {
        println!("Initialising rotary encoder");
        let mut v1 = dp.pins.gpio2.into_input().unwrap();
        let mut v2 = dp.pins.gpio4.into_input().unwrap();
        let mut btn = dp.pins.gpio19.into_input().unwrap();

        v1.set_pull_up().unwrap();
        v2.set_pull_up().unwrap();
        btn.set_pull_up().unwrap();

        let mut enc = rotary_encoder_embedded::RotaryEncoder::new(v1, v2).into_standard_mode();

        let mut timer_service = esp_idf_svc::timer::EspTimerService::new().unwrap();

        let mut prev_btn_state = false;
        let mut pos = 0i32;

        let mut t = Box::new(
            timer_service
                .timer(move || {
                    use rotary_encoder_embedded::Direction;

                    let new_btn_state = btn.is_low().unwrap();
                    if new_btn_state != prev_btn_state {
                        println!("btn: {}", new_btn_state);
                        prev_btn_state = new_btn_state;
                    }
                    enc.update();
                    match enc.direction() {
                        Direction::Clockwise => {
                            pos += 1;
                            println!("encoder + 1 : {}", pos);
                        }
                        Direction::Anticlockwise => {
                            pos -= 1;
                            println!("encoder - 1  : {}", pos);
                        }
                        Direction::None => {}
                    }
                })
                .unwrap(),
        );

        t.every(Duration::from_millis(10)).unwrap();

        std::mem::forget(t);
    }

    /*
    {
        println!("Initialising rotary encoder");
        let mut v1 = dp.pins.gpio27.into_input().unwrap();
        let mut v2 = dp.pins.gpio26.into_input().unwrap();
        let mut btn = dp.pins.gpio25.into_input().unwrap();

        v1.set_pull_up().unwrap();
        v2.set_pull_up().unwrap();
        btn.set_pull_up().unwrap();

        let mut enc = rotary_encoder_embedded::RotaryEncoder::new(v1, v2).into_standard_mode();

        std::thread::spawn(move || {
            use rotary_encoder_embedded::Direction;

            let mut prev_btn_state = false;
            let mut pos = 0i32;
            loop {
                let new_btn_state = btn.is_low().unwrap();
                if new_btn_state != prev_btn_state {
                    println!("btn: {}", new_btn_state);
                    prev_btn_state = new_btn_state;
                }
                enc.update();
                match enc.direction() {
                    Direction::Clockwise => {
                        pos += 1;
                        println!("encoder + 1 : {}", pos);
                    }
                    Direction::Anticlockwise => {
                        pos -= 1;
                        println!("encoder - 1  : {}", pos);
                    }
                    Direction::None => {}
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });
    }
    */

    {
        println!("Initialising display...");
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
            >::new(
                dp.spi2,
                spi::Pins {
                    sclk: dp.pins.gpio18,
                    sdo: dp.pins.gpio23,
                    sdi: None,
                    cs: Some(dp.pins.gpio5.into_output().unwrap()),
                },
                config,
            )
            .expect("Failed to create spi device"),
            dp.pins.gpio21.into_output().unwrap(), // DC
        );

        let mut disp: ssd1309::prelude::GraphicsMode<_> =
            ssd1309::Builder::new().connect(di).into();
        {
            let mut display_reset_pin = dp.pins.gpio22.into_output().unwrap();
            let mut delay_provider = delay::FreeRtos {};
            disp.reset(&mut display_reset_pin, &mut delay_provider)
                .unwrap();
        }
        disp.init().unwrap();

        std::thread::Builder::new()
            .stack_size(10 * 1024)
            .name("Display".to_string())
            .spawn(move || display::dispaly_thread(disp))
            .unwrap();
    }

    {
        extern "C" {
            fn i2c_set_timeout(
                i2c_num: esp_idf_sys::i2c_port_t,
                timeout: esp_idf_sys::c_types::c_int,
            ) -> esp_idf_sys::esp_err_t;
            fn i2c_get_timeout(
                i2c_num: esp_idf_sys::i2c_port_t,
                timeout: *mut esp_idf_sys::c_types::c_int,
            ) -> esp_idf_sys::esp_err_t;
        }

        println!("Initialising sensors...");
        fn print_read_failed(addr: u8, e: I2cError) {
            println!("Failed to read I2C sensor at {}: {}", addr, e);
        }

        let config = <i2c::config::MasterConfig as Default>::default().baudrate(100.kHz().into());
        let mut i2c = i2c::Master::new(
            dp.i2c0,
            i2c::MasterPins {
                sda: dp.pins.gpio26,
                scl: dp.pins.gpio25,
            },
            config,
        )
        .expect("Failed to init i2c");

        unsafe {
            let mut ct: esp_idf_sys::c_types::c_int = 0;
            i2c_get_timeout(0, &mut ct);
            println!("Current i2c strech timout: {}", ct);
            i2c_set_timeout(0, 50000);
        }

        let p_sensor = i2c_sensor::I2CSensor::new(11);
        let f_sensor = i2c_sensor::I2CSensor::new(12);

        std::thread::Builder::new()
            .stack_size(10 * 1024)
            .name("Sensors".to_string())
            .spawn(move || loop {
                let p = match p_sensor.read(&mut i2c) {
                    Ok(v) => v.pressure,
                    Err(e) => {
                        print_read_failed(p_sensor.address(), e);
                        continue;
                    }
                };

                let f = match f_sensor.read(&mut i2c) {
                    Ok(v) => v.f_p,
                    Err(e) => {
                        print_read_failed(f_sensor.address(), e);
                        continue;
                    }
                };

                println!("P = {}, F = {}", p, f);
                std::thread::sleep(Duration::from_millis(100));
            })
            .unwrap();
    }

    println!("Ready!");
}
