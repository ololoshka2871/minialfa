// Принимает текущие показания
// Принимает команды с энкодера
// стейт-машина состояния
// Отдает команды монитору на отрисовку, если что-то изметилось

use std::time::Duration;

use crossbeam::channel::{self, Receiver, Sender};
use num_derive::FromPrimitive;

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
    SetupMenu {
        values: Parameters,
        selected: SelectedParameter,
    },
}

#[derive(Clone, Copy)]
pub struct Parameters {
    pub threshold: f32,
    pub update_period_ms: u32,
}

#[derive(PartialEq, Clone, Copy, FromPrimitive, Default, Debug)]
pub enum SelectedParameter {
    #[default]
    Threshold,
    UpdatePeriodMs,
    SaveAndExit,
}

enum State {
    Title,
    Setup,
}

const MIN_PREASURE: f32 = 1.0;
const MAX_PRESSURE: f32 = 800.0;
const PREASURE_STEP: f32 = 1.0;

const MIN_INTERVAL: u32 = 50;
const MAX_INTERVAL: u32 = 500;
const INTERVAL_STEP: u32 = 50;

static TITLE_OPTIONS: [&'static str; 2] = ["Начать", "Настройки"];

pub struct Controller {
    encoder: (Sender<EncoderCommand>, Receiver<EncoderCommand>),
    sensors: (Sender<SensorResult>, Receiver<SensorResult>),
    display: (Sender<DisplayCommand>, Receiver<DisplayCommand>),

    parameters: Parameters,

    title_option: usize,
    current_state: State,
    current_setup_parameter: SelectedParameter,
}

impl Controller {
    pub fn new() -> Self {
        Self {
            encoder: channel::bounded(3),
            sensors: channel::bounded(3),
            display: channel::bounded(3),

            parameters: Default::default(), // TODO: load

            title_option: 0,
            current_state: State::Title,
            current_setup_parameter: SelectedParameter::Threshold,
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
            println!("Encoder result: {:?}", res);

            match self.current_state {
                State::Title => self.process_title_cmd(res),
                State::Setup => self.process_setup(res),
            }
        } else if let Ok(res) = self.sensors.1.try_recv() {
            //println!("Sensor result: {:?}", res);
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
            EncoderCommand::Push => self.display.0.send(DisplayCommand::TitleScreen {
                option: TITLE_OPTIONS[self.title_option],
                selected: true,
            }),
            EncoderCommand::Pull => {
                // enter setup
                self.current_state = State::Setup;
                self.current_setup_parameter = SelectedParameter::Threshold;

                self.display.0.send(DisplayCommand::SetupMenu {
                    values: self.parameters,
                    selected: self.current_setup_parameter,
                })
            }
        }
        .unwrap()
    }

    fn process_setup(&mut self, cmd: EncoderCommand) {
        if cmd == EncoderCommand::Pull {
            match self.current_setup_parameter {
                SelectedParameter::SaveAndExit => {
                    self.current_state = State::Title;
                    self.current_setup_parameter = SelectedParameter::Threshold;
                    self.title_option = 0;

                    self.display.0.send(DisplayCommand::TitleScreen {
                        option: TITLE_OPTIONS[self.title_option],
                        selected: false,
                    })
                }
                _ => {
                    self.current_setup_parameter =
                        num::FromPrimitive::from_u32(self.current_setup_parameter as u32 + 1)
                            .unwrap_or_default();

                    self.display.0.send(DisplayCommand::SetupMenu {
                        values: self.parameters,
                        selected: self.current_setup_parameter,
                    })
                }
            }
            .unwrap()
        } else if cmd != EncoderCommand::Push {
            match self.current_setup_parameter {
                SelectedParameter::Threshold => {
                    match cmd {
                        EncoderCommand::Increment => {
                            if self.parameters.threshold < MAX_PRESSURE {
                                self.parameters.threshold += PREASURE_STEP;
                            }
                        }
                        EncoderCommand::Decrement => 
                        if self.parameters.threshold > MIN_PREASURE {
                            self.parameters.threshold -= PREASURE_STEP;
                        }
                        _ => {}
                    }
                }
                SelectedParameter::UpdatePeriodMs => {
                    match cmd {
                        EncoderCommand::Increment => if self.parameters.update_period_ms < MAX_INTERVAL{
                            self.parameters.update_period_ms += INTERVAL_STEP;
                        },
                        EncoderCommand::Decrement => if self.parameters.update_period_ms > MIN_INTERVAL{
                            self.parameters.update_period_ms -= INTERVAL_STEP;
                        },
                        _ => {},
                    }
                }
                SelectedParameter::SaveAndExit => { /* nothing */ }
            }

            self.display
                .0
                .send(DisplayCommand::SetupMenu {
                    values: self.parameters,
                    selected: self.current_setup_parameter,
                })
                .unwrap();
        }
    }
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            threshold: 1.0,
            update_period_ms: 100,
        }
    }
}
