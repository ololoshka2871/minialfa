// Принимает текущие показания
// Принимает команды с энкодера
// стейт-машина состояния
// Отдает команды монитору на отрисовку, если что-то изметилось

use std::time::Duration;

use crossbeam::channel::{self, Receiver, Sender};
use esp_idf_svc::{
    nvs::{EspNvs, NvsPartitionId},
    timer::EspTimer,
};
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
    SctbSensorResult { f: f32, p: f32, t: f32 },
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
        precision: Precission,
    },
    Measure {
        f: Option<f32>,
        p: Option<f32>,
        threashold: f32,
        wait_time: Option<Duration>,
    },
    Result {
        f: f32,
        p: f32,
        t: Option<f32>,
        threashold: f32,
        sensivity: f32,
    },
}

#[derive(Clone, Copy, Default, FromPrimitive)]
pub enum Precission {
    #[default]
    C1,
    C01,
    C001,
}

impl Precission {
    pub fn value(&self) -> usize {
        match self {
            Precission::C1 => 0,
            Precission::C01 => 1,
            Precission::C001 => 2,
        }
    }

    fn increment_value(current: f32) -> f32 {
        match current.into() {
            Precission::C1 => 1.0,
            Precission::C01 => 0.1,
            Precission::C001 => 0.01,
        }
    }

    fn decrement_value(current: f32) -> f32 {
        let inc = Self::increment_value(current);
        if current - inc < inc {
            inc / 10.0
        } else {
            inc
        }
    }
}

impl From<f32> for Precission {
    fn from(v: f32) -> Self {
        if v >= 0.0 && v <= 0.09 {
            Precission::C001
        } else if v <= 0.9 {
            Precission::C01
        } else {
            Precission::C1
        }
    }
}

#[derive(Clone, Copy)]
pub struct Parameters {
    pub threshold: f32,
    pub wait_time_s: u32,
    pub update_period_ms: u32,
    pub try_use_alternative_sensor: bool,
}

#[derive(PartialEq, Clone, Copy, FromPrimitive, Default, Debug)]
pub enum SelectedParameter {
    #[default]
    Threshold,
    UpdatePeriodMs,
    PSensorSelect,
    WaitTimeS,
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

const MIN_PREASURE: f32 = 0.01;
const MAX_PRESSURE: f32 = 800.0;

const MAX_WAIT_TIME_S: u32 = 5 * 60; //5 min

const MIN_INTERVAL: u32 = 50;
const MAX_INTERVAL: u32 = 200;
const INTERVAL_STEP: u32 = 10;

pub struct Controller<T: NvsPartitionId> {
    encoder: (Sender<EncoderCommand>, Receiver<EncoderCommand>),
    sensors: (Sender<SensorResult>, Receiver<SensorResult>),
    display: (Sender<DisplayCommand>, Receiver<DisplayCommand>),

    parameters: Parameters,

    title_option: TitleOptions,
    current_mode: TitleOptions,
    current_state: State,
    current_setup_parameter: SelectedParameter,

    prev_p: f32,
    prev_f: f32,
    prev_t: f32,

    initial_point: Option<(f32, f32)>,

    start_waiting_time: Option<Duration>,

    nvs: EspNvs<T>,
}

impl<T: NvsPartitionId> Controller<T> {
    pub fn new(nvs: EspNvs<T>) -> Self {
        Self {
            encoder: channel::bounded(3),
            sensors: channel::bounded(3),
            display: channel::bounded(3),

            parameters: Parameters::load(&nvs),

            title_option: TitleOptions::Auto,
            current_mode: TitleOptions::Auto,
            current_state: State::Title,
            current_setup_parameter: SelectedParameter::Threshold,

            prev_p: 0.0,
            prev_f: 0.0,
            prev_t: 0.0,

            initial_point: None,

            start_waiting_time: None,

            nvs,
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
        use esp_idf_svc::systime::EspSystemTime;

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
                        self.title_option = self.current_mode;

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
                let (p, t) = match res {
                    SensorResult::SctbSensorResult { f, p, t } => {
                        self.prev_f = f;
                        self.prev_t = t;
                        if self.parameters.try_use_alternative_sensor
                            && thyracont_update_timer.is_some()
                        {
                            (self.prev_p, t)
                        } else {
                            (p, t)
                        }
                    }
                    SensorResult::TyracontSensorResult { p } => {
                        // ignore this sesor result if disabled
                        if self.parameters.try_use_alternative_sensor {
                            (Self::mbar2mm_hg(p), self.prev_t)
                        } else {
                            return;
                        }
                    }
                };

                // capture initial point
                if self.initial_point.is_none() {
                    self.initial_point.replace((p, self.prev_f));
                }

                if let Some(start_waiting_time) = self.start_waiting_time {
                    let wait_time_s = Duration::from_secs(self.parameters.wait_time_s as u64);
                    let now = EspSystemTime {}.now();

                    // Идет удержание
                    if (now - start_waiting_time) >= wait_time_s {
                        println!("Waiting time expired");
                        self.start_waiting_time.take(); // clear waiting time

                        // end -> result screen
                        self.current_state = State::Result;

                        sctb_sensors_timer.cancel().unwrap();
                        thyracont_update_timer.as_mut().map(|t| t.cancel().unwrap());

                        let sensivity = if let Some(initial_point) = self.initial_point.take() {
                            let delta_p = initial_point.0 - p;
                            let delta_f = initial_point.1 - self.prev_f;
                            delta_f / delta_p
                        } else {
                            f32::NAN
                        };

                        self.display
                            .0
                            .send(DisplayCommand::Result {
                                p: p,
                                f: self.prev_f,
                                t: Some(t),
                                threashold: self.parameters.threshold,
                                sensivity,
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
                                wait_time: Some(start_waiting_time + wait_time_s - now),
                            })
                            .unwrap();
                    }
                } else {
                    // Only in auto mode
                    if self.title_option == TitleOptions::Auto
                        && self.prev_p > p
                        && p <= self.parameters.threshold
                    {
                        self.start_waiting_time.replace(EspSystemTime {}.now());
                    }

                    // update screen
                    self.display
                        .0
                        .send(DisplayCommand::Measure {
                            f: Some(self.prev_f),
                            p: Some(p),
                            threashold: self.parameters.threshold,
                            wait_time: None,
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
                self.title_option = match (self.title_option, cmd) {
                    (TitleOptions::Auto, EncoderCommand::Decrement) => TitleOptions::Setup,

                    _ => num::FromPrimitive::from_u32(
                        (self.title_option as u32).wrapping_add_signed(
                            if cmd == EncoderCommand::Increment {
                                1i32
                            } else {
                                -1
                            },
                        ) % (TitleOptions::COUNT as u32),
                    )
                    .unwrap(),
                };

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
                self.initial_point.take(); // clear initial point
                self.start_waiting_time.take(); // clear waiting time
                match self.title_option {
                    TitleOptions::Auto | TitleOptions::Manual => {
                        // enter working cycle
                        self.prev_p = 0.0;
                        self.current_mode = self.title_option; // save current mode for return
                        self.current_state = State::Measuring;
                        self.display
                            .0
                            .send(DisplayCommand::Measure {
                                f: None,
                                p: None,
                                threashold: self.parameters.threshold,
                                wait_time: None,
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
                                precision: Precission::from(self.parameters.threshold),
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

                    self.parameters.store(&mut self.nvs);

                    self.display.0.send(DisplayCommand::TitleScreen {
                        option: TITLE_OPTIONS[self.title_option as usize],
                        selected: false,
                    })
                }
                _ => {
                    self.current_setup_parameter =
                        // skip SelectedParameter::PSensorSelect
                        if self.current_setup_parameter == SelectedParameter::UpdatePeriodMs {
                            SelectedParameter::WaitTimeS
                        } else {
                            num::FromPrimitive::from_u32(self.current_setup_parameter as u32 + 1)
                                .unwrap_or_default()
                        };

                    self.display.0.send(DisplayCommand::SetupMenu {
                        values: self.parameters,
                        selected: self.current_setup_parameter,
                        precision: Precission::from(self.parameters.threshold),
                    })
                }
            }
            .unwrap()
        } else if cmd != EncoderCommand::Push {
            match self.current_setup_parameter {
                SelectedParameter::Threshold => match cmd {
                    EncoderCommand::Increment => {
                        if self.parameters.threshold < MAX_PRESSURE {
                            let step = Precission::increment_value(self.parameters.threshold);
                            self.parameters.threshold =
                                ((self.parameters.threshold / step).round() + 1.0) * step;
                        }
                    }
                    EncoderCommand::Decrement => {
                        if self.parameters.threshold > MIN_PREASURE {
                            let step = Precission::decrement_value(self.parameters.threshold);
                            self.parameters.threshold =
                                ((self.parameters.threshold / step).round() - 1.0) * step;
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
                SelectedParameter::WaitTimeS => match cmd {
                    EncoderCommand::Increment => {
                        if self.parameters.wait_time_s < MAX_WAIT_TIME_S {
                            self.parameters.wait_time_s += 1;
                        }
                    }
                    EncoderCommand::Decrement => {
                        if self.parameters.wait_time_s > 0 {
                            self.parameters.wait_time_s -= 1;
                        }
                    }
                    _ => {}
                },

                SelectedParameter::SaveAndExit => { /* nothing */ }
            }

            self.display
                .0
                .send(DisplayCommand::SetupMenu {
                    values: self.parameters,
                    selected: self.current_setup_parameter,
                    precision: Precission::from(self.parameters.threshold),
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
            wait_time_s: 10,
        }
    }
}

// parameters storage names
const THRESHOLD: &str = "threshold";
const UPDATE_PERIOD_MS: &str = "upd_per_ms";
const TRY_USE_ALTERNATIVE_SENSOR: &str = "alt_sens";
const WAIT_TIME_S: &str = "wait_time_s";

impl Parameters {
    pub fn load(nvs: &EspNvs<impl NvsPartitionId>) -> Self {
        let threshold = nvs
            .get_u32(THRESHOLD)
            .map(|v| {
                v.map(|v| unsafe { std::mem::transmute::<_, f32>(v) })
                    .unwrap_or(1.0)
            })
            .unwrap();
        let update_period_ms = nvs
            .get_u32(UPDATE_PERIOD_MS)
            .map(|v| v.unwrap_or(100))
            .unwrap();
        let try_use_alternative_sensor = nvs
            .get_u8(TRY_USE_ALTERNATIVE_SENSOR)
            .map(|v| v.unwrap_or(0) != 0)
            .unwrap();
        let wait_time_s = nvs.get_u32(WAIT_TIME_S).map(|v| v.unwrap_or(5)).unwrap();

        Self {
            threshold,
            update_period_ms,
            try_use_alternative_sensor,
            wait_time_s,
        }
    }

    pub fn store(&self, nvs: &mut EspNvs<impl NvsPartitionId>) {
        nvs.set_u32(THRESHOLD, unsafe { std::mem::transmute(self.threshold) })
            .unwrap();
        nvs.set_u32(UPDATE_PERIOD_MS, self.update_period_ms)
            .unwrap();
        nvs.set_u8(
            TRY_USE_ALTERNATIVE_SENSOR,
            self.try_use_alternative_sensor as u8,
        )
        .unwrap();
        nvs.set_u32(WAIT_TIME_S, self.wait_time_s).unwrap();
    }
}
