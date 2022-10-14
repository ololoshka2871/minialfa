// Принимает текущие показания
// Принимает команды с энкодера
// стейт-машина состояния
// Отдает команды монитору на отрисовку, если что-то изметилось

use std::time::Duration;

use crossbeam::channel::{self, Receiver, Sender};

#[derive(Clone, Copy, Debug)]
pub enum EncoderCommand {
    Increment,
    Decrement,
    Push,
    Pull,
}

#[derive(Clone, Copy, Debug)]
pub struct SensorResult {
    pub f: f32,
    pub p: f32,
}

pub enum DisplayCommand {}

pub struct Controller {
    encoder: (Sender<EncoderCommand>, Receiver<EncoderCommand>),
    sensors: (Sender<SensorResult>, Receiver<SensorResult>),
    display: (Sender<DisplayCommand>, Receiver<DisplayCommand>),
}

impl Controller {
    pub fn new() -> Self {
        Self {
            encoder: channel::bounded(3),
            sensors: channel::bounded(3),
            display: channel::bounded(3),
        }
    }

    pub fn command_chanel(&self) -> Sender<EncoderCommand> {
        self.encoder.0.clone()
    }

    pub fn sensor_chanel(&self) -> Sender<SensorResult> {
        self.sensors.0.clone()
    }

    pub fn display_chanel(&self) -> Receiver<DisplayCommand> {
        self.display.1.clone()
    }

    pub fn poll(&mut self) {
        if let Ok(res) = self.encoder.1.try_recv() {
            //
            println!("Encoder result: {:?}", res);
        } else if let Ok(res) = self.sensors.1.try_recv() {
            //
            println!("Sensor result: {:?}", res);
        } else {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
