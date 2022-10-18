pub enum KlapanState {
    Atmosphere,
    Vacuum,
}

pub struct Klapan<PIN> {
    pin: PIN,
}

impl<E, PIN: embedded_hal::digital::v2::OutputPin<Error = E>> Klapan<PIN> {
    pub fn new(pin: PIN) -> Self {
        Self { pin }
    }

    pub fn set_state(&mut self, state: KlapanState) -> Result<(), E> {
        match state {
            KlapanState::Atmosphere => self.pin.set_low(),
            KlapanState::Vacuum => self.pin.set_high(),
        }
    }
}
