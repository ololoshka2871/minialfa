use std::num::ParseIntError;
use std::time::Duration;
use std::{io::Write, thread};

//use regex::Regex;

/*
lazy_static::lazy_static! {
    static ref ID_RE: Regex = Regex::new(r"(\d{1,3})(TVSP206).\r").unwrap();
    static ref RESULT_RE: Regex = Regex::new(r"\d{1,3}M(\d{4})(\d{2}).\r").unwrap();
}
*/

#[derive(Debug)]
pub enum Response {
    Id { addr: u8, id: String },
    Pressure(f32),
}

#[derive(Debug)]
enum ParseError {
    Incomplead,
    CrcError { msg: u8, actual: u8 },
    ValueError(String),
    Unknown,
}

pub struct TyracontSensor {
    addr: u8,
}

impl TyracontSensor {
    pub fn new(addr: u8) -> Self {
        Self { addr }
    }

    pub fn get_id<P, E, PIN, PINE>(
        &self,
        port: &mut P,
        re_de: &mut PIN,
    ) -> Result<Option<String>, E>
    where
        P: embedded_hal::serial::Read<u8, Error = E> + embedded_hal::serial::Write<u8, Error = E>,
        PIN: embedded_hal::digital::v2::OutputPin<Error = PINE>,
    {
        let mut id_req = vec![];
        let addr = self.addr;
        write!(&mut id_req, "{addr:03}T").unwrap();

        Self::write_request(port, &id_req, re_de)?;
        Self::flush_rx(port)?;

        thread::sleep(Duration::from_millis(35));

        let resp = Self::read_response(port)?;

        //println!("-> rx: \"{}\"", String::from_utf8_lossy(&resp));

        match Self::decode_resp(&resp) {
            Ok(Response::Id { addr: _, id }) => Ok(Some(id)),
            Ok(r) => panic!("Unknown response {r:?}"),
            Err(ParseError::CrcError { msg, actual }) => {
                println!("Got invalig message: Crc error (msg: {msg} != actual: {actual})");
                Ok(None)
            }
            Err(ParseError::Incomplead) => Ok(None),
            Err(e) => panic!("Response parse error {e:?}"),
        }
    }

    pub fn read<P, E, PIN, PINE>(&self, port: &mut P, re_de: &mut PIN) -> Result<Option<f32>, E>
    where
        P: embedded_hal::serial::Read<u8, Error = E> + embedded_hal::serial::Write<u8, Error = E>,
        PIN: embedded_hal::digital::v2::OutputPin<Error = PINE>,
    {
        let mut output_req = vec![];
        let addr = self.addr;
        write!(&mut output_req, "{addr:03}M").unwrap();

        Self::flush_rx(port)?;
        Self::write_request(port, &output_req, re_de)?;

        thread::sleep(Duration::from_millis(35));

        let resp = Self::read_response(port)?;

        match Self::decode_resp(&resp) {
            Ok(Response::Pressure(p)) => Ok(Some(p)),
            Ok(r) => {
                println!("Unknown response {r:?}");
                Ok(None)
            }
            Err(ParseError::CrcError { msg, actual }) => {
                println!("Got invalig message: Crc error (msg: {msg} != actual: {actual})");
                Ok(None)
            }
            Err(ParseError::Incomplead) => Ok(None),
            Err(e) => {
                println!("Parse error {e:?}");
                Ok(None)
            }
        }
    }

    /// Checksum (hex), defined as sum over bytes from fields “Address”, “Code” and “Data”, modulo 64 plus 64.
    fn calc_crc(data: &[u8]) -> u8 {
        (data.iter().fold(0u32, |a, b| a + *b as u32) % 64 + 64) as u8
    }

    fn write_request<P, E, PIN, PINE>(port: &mut P, req: &[u8], re_de: &mut PIN) -> Result<(), E>
    where
        P: embedded_hal::serial::Read<u8, Error = E> + embedded_hal::serial::Write<u8, Error = E>,
        PIN: embedded_hal::digital::v2::OutputPin<Error = PINE>,
    {
        let crc = Self::calc_crc(req);

        let _ = re_de.set_high();
        for b in req.iter() {
            nb::block!(port.write(*b))?;
        }
        nb::block!(port.write(crc))?;
        nb::block!(port.write(b'\r'))?;

        nb::block!(port.flush())?;
        let _ = re_de.set_low();

        Ok(())
    }

    fn flush_rx<P, E>(port: &mut P) -> Result<(), E>
    where
        P: embedded_hal::serial::Read<u8, Error = E> + embedded_hal::serial::Write<u8, Error = E>,
    {
        loop {
            match port.read() {
                Ok(_) => {}
                Err(nb::Error::WouldBlock) => break,
                Err(nb::Error::Other(e)) => Err(e)?,
            }
        }
        Ok(())
    }

    fn read_response<P, E>(port: &mut P) -> Result<Vec<u8>, E>
    where
        P: embedded_hal::serial::Read<u8, Error = E> + embedded_hal::serial::Write<u8, Error = E>,
    {
        let mut result = vec![];
        loop {
            match port.read() {
                Ok(b) => result.push(b),
                Err(nb::Error::WouldBlock) => break,
                Err(nb::Error::Other(e)) => Err(e)?,
            }
        }
        Ok(result)
    }

    fn decode_resp(data: &[u8]) -> Result<Response, ParseError> {
        let last_ch = data.last();
        if last_ch.is_none() {
            return Err(ParseError::Incomplead);
        }

        let last_ch = last_ch.unwrap();

        if *last_ch != b'\r' {
            return Err(ParseError::Incomplead);
        }

        let crc = Self::calc_crc(&data[..data.len() - 2]);
        let msg_crc = data[data.len() - 2];
        if crc != msg_crc {
            return Err(ParseError::CrcError {
                msg: msg_crc,
                actual: crc,
            });
        }

        let resp_addr = String::from_utf8_lossy(&data[..3]);
        let resp_addr = resp_addr
            .parse::<u8>()
            .map_err(|_| ParseError::ValueError(format!("{resp_addr} not a number")))?;

        let command = data[3];

        match command {
            b'T' => Ok(Response::Id {
                addr: resp_addr,
                id: std::str::from_utf8(&data[4..data.len() - 2])
                    .map_err(|_| ParseError::ValueError("Failed to parse id".to_string()))?
                    .to_string(),
            }),
            b'M' => {
                let data = &data[4..4 + 6];
                let m: u32 = String::from_utf8_lossy(&data[..4])
                    .parse()
                    .map_err(|e: ParseIntError| ParseError::ValueError(e.to_string()))?;
                let exp: i32 = String::from_utf8_lossy(&data[4..])
                    .parse()
                    .map_err(|e: ParseIntError| ParseError::ValueError(e.to_string()))?;

                let res = m as f32 / 1_000.0 * 10.0f32.powi(exp - 20);

                Ok(Response::Pressure(res))
            }
            _ => Err(ParseError::Unknown),
        }
    }
}
