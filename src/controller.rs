// Принимает текущие показания
// Принимает команды с энкодера
// стейт-машина состояния
// Отдает команды монитору на отрисовку, если что-то изметилось

use std::time::Duration;

use crossbeam::channel::{self, Receiver, Sender};
use esp_idf_svc::timer::EspTimer;
use num_derive::FromPrimitive;

use crate::klapan::KlapanState;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EncoderCommand {
    Increment,
    Decrement,
    Push,
    Pull,
}

#[derive(Clone, Copy, Debug)]
pub enum SensorResult {
    SctbSensorResult { f: f32, p: f32 },
    TyracontSensorResult { p: f32 },
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
    Measure {
        f: Option<f32>,
        p: Option<f32>,
        threashold: f32,
    },
    Result {
        f: f32,
        p: f32,
        threashold: f32,
    },
}

#[derive(Clone, Copy)]
pub struct Parameters {
    pub threshold: f32,
    pub update_period_ms: u32,
    pub try_use_alternative_sensor: bool,
}

#[derive(PartialEq, Clone, Copy, FromPrimitive, Default, Debug)]
pub enum SelectedParameter {
    #[default]
    Threshold,
    UpdatePeriodMs,
    PSensorSelect,
    SaveAndExit,
}

#[derive(PartialEq)]
enum State {
    Title,
    Setup,
    Measuring,
    Result,
}

#[derive(PartialEq, Clone, Copy, FromPrimitive)]
enum TitleOptions {
    Auto = 0,
    Manual = 1,
    Setup = 2,
    COUNT,
}

static TITLE_OPTIONS: [&'static str; 3] = ["Авто", "Ручной", "Настройки"];

const MIN_PREASURE: f32 = 1.0;
const MAX_PRESSURE: f32 = 800.0;
const PREASURE_STEP: f32 = 1.0;

const MIN_INTERVAL: u32 = 50;
const MAX_INTERVAL: u32 = 500;
const INTERVAL_STEP: u32 = 50;

pub struct Controller {
    encoder: (Sender<EncoderCommand>, Receiver<EncoderCommand>),
    sensors: (Sender<SensorResult>, Receiver<SensorResult>),
    display: (Sender<DisplayCommand>, Receiver<DisplayCommand>),

    parameters: Parameters,

    title_option: TitleOptions,
    current_state: State,
    current_setup_parameter: SelectedParameter,

    prev_p: f32,
    prev_f: f32,
}

impl Controller {
    pub fn new() -> Self {
        Self {
            encoder: channel::bounded(3),
            sensors: channel::bounded(3),
            display: channel::bounded(3),

            parameters: Default::default(), // TODO: load

            title_option: TitleOptions::Auto,
            current_state: State::Title,
            current_setup_parameter: SelectedParameter::Threshold,

            prev_p: 0.0,
            prev_f: 0.0,
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
                option: TITLE_OPTIONS[self.title_option as usize],
                selected: false,
            })
            .unwrap();
        self.display.1.clone()
    }

    pub fn poll<E, PIN>(
        &mut self,
        sctb_sensors_timer: &mut EspTimer,
        thyracont_update_timer: &mut Option<EspTimer>,
        klapan: &mut crate::klapan::Klapan<PIN>,
    ) where
        PIN: embedded_hal::digital::v2::OutputPin<Error = E>,
        E: std::fmt::Debug,
    {
        match self.current_state {
            State::Measuring => klapan.set_state(KlapanState::Vacuum),
            _ => klapan.set_state(KlapanState::Atmosphere),
        }
        .unwrap();

        if let Ok(res) = self.encoder.1.try_recv() {
            //println!("Encoder result: {:?}", res);
            match self.current_state {
                State::Title => {
                    if self.process_title_cmd(res) {
                        sctb_sensors_timer
                            .every(Duration::from_millis(
                                self.parameters.update_period_ms as u64,
                            ))
                            .expect("Failed to starts sensors");

                        thyracont_update_timer.as_mut().map(|t| {
                            t.every(Duration::from_millis(
                                self.parameters.update_period_ms as u64,
                            ))
                            .expect("Failed to starts sensors")
                        });
                    }
                }
                State::Setup => self.process_setup(res),
                State::Measuring | State::Result => match res {
                    EncoderCommand::Pull => {
                        // отмена измерения, возврат на главный экран
                        self.current_state = State::Title;
                        self.title_option = TitleOptions::Auto;

                        sctb_sensors_timer.cancel().unwrap();
                        thyracont_update_timer.as_mut().map(|t| t.cancel().unwrap());

                        self.display
                            .0
                            .send(DisplayCommand::TitleScreen {
                                option: TITLE_OPTIONS[self.title_option as usize],
                                selected: false,
                            })
                            .unwrap()
                    }
                    _ => {}
                },
            }
        } else if let Ok(res) = self.sensors.1.try_recv() {
            //println!("Sensor result: {:?}", res);
            if self.current_state == State::Measuring {
                let p = match res {
                    SensorResult::SctbSensorResult { f, p } => {
                        self.prev_f = f;
                        if self.parameters.try_use_alternative_sensor
                            && thyracont_update_timer.is_some()
                        {
                            self.prev_p
                        } else {
                            p
                        }
                    }
                    SensorResult::TyracontSensorResult { p } => {
                        // ignore this sesor result if disabled
                        if self.parameters.try_use_alternative_sensor {
                            Self::mbar2mm_hg(p)
                        } else {
                            return;
                        }
                    }
                };

                if self.title_option == TitleOptions::Auto // Only in auto mode
                    && self.prev_p > p
                    && p <= self.parameters.threshold
                {
                    // end -> result screen
                    self.current_state = State::Result;

                    sctb_sensors_timer.cancel().unwrap();
                    thyracont_update_timer.as_mut().map(|t| t.cancel().unwrap());

                    self.display
                        .0
                        .send(DisplayCommand::Result {
                            p: p,
                            f: self.prev_f,
                            threashold: self.parameters.threshold,
                        })
                        .unwrap()
                } else {
                    // update screen
                    self.display
                        .0
                        .send(DisplayCommand::Measure {
                            f: Some(self.prev_f),
                            p: Some(p),
                            threashold: self.parameters.threshold,
                        })
                        .unwrap();
                }
                self.prev_p = p;
            }
        } else {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn process_title_cmd(&mut self, cmd: EncoderCommand) -> bool {
        match cmd {
            EncoderCommand::Increment | EncoderCommand::Decrement => {
                self.title_option = num::FromPrimitive::from_u32(
                    (self.title_option as u32).wrapping_add_signed(
                        if cmd == EncoderCommand::Increment {
                            1i32
                        } else {
                            -1
                        },
                    ) % (TitleOptions::COUNT as u32),
                )
                .unwrap();

                self.display
                    .0
                    .send(DisplayCommand::TitleScreen {
                        option: TITLE_OPTIONS[self.title_option as usize],
                        selected: false,
                    })
                    .unwrap();
                false
            }
            EncoderCommand::Push => {
                self.display
                    .0
                    .send(DisplayCommand::TitleScreen {
                        option: TITLE_OPTIONS[self.title_option as usize],
                        selected: true,
                    })
                    .unwrap();
                false
            }
            EncoderCommand::Pull => {
                match self.title_option {
                    TitleOptions::Auto | TitleOptions::Manual => {
                        // enter working cycle
                        self.prev_p = 0.0;
                        self.current_state = State::Measuring;
                        self.display
                            .0
                            .send(DisplayCommand::Measure {
                                f: None,
                                p: None,
                                threashold: self.parameters.threshold,
                            })
                            .unwrap();

                        true
                    }
                    TitleOptions::Setup => {
                        // enter setup
                        self.current_state = State::Setup;
                        self.current_setup_parameter = SelectedParameter::Threshold;

                        self.display
                            .0
                            .send(DisplayCommand::SetupMenu {
                                values: self.parameters,
                                selected: self.current_setup_parameter,
                            })
                            .unwrap();
                        false
                    }
                    TitleOptions::COUNT => unreachable!(),
                }
            }
        }
    }

    fn process_setup(&mut self, cmd: EncoderCommand) {
        if cmd == EncoderCommand::Pull {
            match self.current_setup_parameter {
                SelectedParameter::SaveAndExit => {
                    self.current_state = State::Title;
                    self.current_setup_parameter = SelectedParameter::Threshold;
                    self.title_option = TitleOptions::Setup;

                    self.display.0.send(DisplayCommand::TitleScreen {
                        option: TITLE_OPTIONS[self.title_option as usize],
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
                SelectedParameter::Threshold => match cmd {
                    EncoderCommand::Increment => {
                        if self.parameters.threshold < MAX_PRESSURE {
                            self.parameters.threshold += PREASURE_STEP;
                        }
                    }
                    EncoderCommand::Decrement => {
                        if self.parameters.threshold > MIN_PREASURE {
                            self.parameters.threshold -= PREASURE_STEP;
                        }
                    }
                    _ => {}
                },
                SelectedParameter::UpdatePeriodMs => match cmd {
                    EncoderCommand::Increment => {
                        if self.parameters.update_period_ms < MAX_INTERVAL {
                            self.parameters.update_period_ms += INTERVAL_STEP;
                        }
                    }
                    EncoderCommand::Decrement => {
                        if self.parameters.update_period_ms > MIN_INTERVAL {
                            self.parameters.update_period_ms -= INTERVAL_STEP;
                        }
                    }
                    _ => {}
                },
                SelectedParameter::PSensorSelect => {
                    self.parameters.try_use_alternative_sensor ^= true
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

    fn mbar2mm_hg(p: f32) -> f32 {
        p * 0.7500616
    }
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            threshold: 1.0,
            update_period_ms: 100,
            try_use_alternative_sensor: false,
        }
    }
}
