pub struct I2CSensor {
    addr: u8,
}

pub struct SensorResult {
    pub pressure: f32,
    pub temperature: f32,
    pub f_p: f32,
    pub f_t: f32,
}

impl I2CSensor {
    pub fn new(i2c_addr: u8) -> Self {
        Self { addr: i2c_addr }
    }

    pub fn read<I2C: embedded_hal::blocking::i2c::Read>(
        &self,
        i2c_bus: &mut I2C,
    ) -> Result<SensorResult, I2C::Error> {
        let mut dest = [0u8; std::mem::size_of::<SensorResult>()];
        i2c_bus.read(self.addr, &mut dest)?;
        unsafe { Ok(std::mem::transmute(dest)) }
    }

    pub fn address(&self) -> u8 {
        self.addr
    }
}
