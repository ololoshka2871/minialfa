// Принимает текущие показания
// Принимает команды с энкодера
// стейт-машина состояния
// Отдает команды монитору на отрисовку, если что-то изметилось

use std::time::Duration;

use crossbeam::channel::{self, Receiver, Sender};

#[derive(Clone, Copy, Debug, PartialEq)]
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

pub enum DisplayCommand {
    TitleScreen {
        option: &'static str,
        selected: bool,
    },
}

enum State {
    Title,
}

static TITLE_OPTIONS: [&'static str; 2] = ["Начать", "Настройки"];

pub struct Controller {
    encoder: (Sender<EncoderCommand>, Receiver<EncoderCommand>),
    sensors: (Sender<SensorResult>, Receiver<SensorResult>),
    display: (Sender<DisplayCommand>, Receiver<DisplayCommand>),

    title_option: usize,
    current_state: State,
}

impl Controller {
    pub fn new() -> Self {
        Self {
            encoder: channel::bounded(3),
            sensors: channel::bounded(3),
            display: channel::bounded(3),

            title_option: 0,

            current_state: State::Title,
        }
    }

    pub fn command_chanel(&self) -> Sender<EncoderCommand> {
        self.encoder.0.clone()
    }

    pub fn sensor_chanel(&self) -> Sender<SensorResult> {
        self.sensors.0.clone()
    }

    pub fn display_chanel(&self) -> Receiver<DisplayCommand> {
        self.display
            .0
            .send(DisplayCommand::TitleScreen {
                option: TITLE_OPTIONS[self.title_option],
                selected: false,
            })
            .unwrap();
        self.display.1.clone()
    }

    pub fn poll(&mut self) {
        if let Ok(res) = self.encoder.1.try_recv() {
            //
            println!("Encoder result: {:?}", res);

            match self.current_state {
                State::Title => self.process_title_cmd(res),
            }
        } else if let Ok(res) = self.sensors.1.try_recv() {
            //
            println!("Sensor result: {:?}", res);
        } else {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn process_title_cmd(&mut self, cmd: EncoderCommand) {
        match cmd {
            EncoderCommand::Increment | EncoderCommand::Decrement => {
                self.title_option =
                    self.title_option
                        .wrapping_add_signed(if cmd == EncoderCommand::Increment {
                            1isize
                        } else {
                            -1
                        })
                        % TITLE_OPTIONS.len();

                self.display.0.send(DisplayCommand::TitleScreen {
                    option: TITLE_OPTIONS[self.title_option],
                    selected: false,
                })
            }
            EncoderCommand::Push | EncoderCommand::Pull => {
                self.display.0.send(DisplayCommand::TitleScreen {
                    option: TITLE_OPTIONS[self.title_option],
                    selected: cmd == EncoderCommand::Push,
                })
            }
        }
        .unwrap()
    }
}
