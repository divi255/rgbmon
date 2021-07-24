use colored::Colorize;
use log::{debug, error};
use std::convert::TryInto;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

const PROTOCOL_VERSION: u32 = 2;

const REQ_REQUEST_PROTOCOL_VERSION: u32 = 40;
const REQ_SET_CLIENT_NAME: u32 = 50;
const REQ_REQUEST_CONTROLLER_COUNT: u32 = 0;
const REQ_REQUEST_CONTROLLER_DATA: u32 = 1;
const REQ_RGBCONTROLLER_UPDATELEDS: u32 = 1050;
//const REQ_RGBCONTROLLER_UPDATEMODE:u32 = 1101;

const HEADER: [u8; 4] = [b'O', b'R', b'G', b'B'];

const ERR_CONTROLLER_NOT_FOUND: &str = "controller not found";

const CLIENT_NAME: &str = "rgbmon";
pub const VERSION: &str = "0.0.1";

#[derive(PartialEq, Copy, Clone)]
pub struct RGBColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl fmt::Display for RGBColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut color = String::new();
        for c in vec![self.red, self.green, self.blue] {
            let z = format!("{:#04X}", c as u32);
            color += &z.as_str()[2..];
        }
        write!(f, "{}", color)
    }
}

impl RGBColor {
    pub fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    pub fn black() -> Self {
        Self {
            red: 0,
            green: 0,
            blue: 0,
        }
    }

    pub fn colorize<T: fmt::Display>(&self, src: T) -> colored::ColoredString {
        format!("{}", src).truecolor(self.red, self.green, self.blue)
    }

    pub fn colorize_self(&self) -> colored::ColoredString {
        self.colorize(&self)
    }

    pub fn from_str(s: &str) -> Self {
        Self {
            red: u8::from_str_radix(&s[0..2], 16).unwrap(),
            green: u8::from_str_radix(&s[2..4], 16).unwrap(),
            blue: u8::from_str_radix(&s[4..6], 16).unwrap(),
        }
    }

    pub fn rainbow(step: u32, total: u32, start: u32, end: u32) -> Self {
        let coef: f32 = (total - start) as f32 / total as f32 - (total - end) as f32 / total as f32;
        let sstep: f32 = (step as f32 * coef * total as f32 / 100.) + start as f32;
        let r: f32;
        let g: f32;
        let b: f32;
        let h: f32 = 1. - (sstep as f32 / total as f32);
        let i: u32 = !!((h * 6.) as u32);
        let f: f32 = h * 6. - i as f32;
        let q: f32 = 1. - f;
        match i % 6 {
            0 => {
                r = 1.;
                g = f;
                b = 0.;
            }
            1 => {
                r = q;
                g = 1.;
                b = 0.;
            }
            2 => {
                r = 0.;
                g = 1.;
                b = f;
            }
            3 => {
                r = 0.;
                g = q;
                b = 1.;
            }
            4 => {
                r = f;
                g = 0.;
                b = 1.;
            }
            5 => {
                r = 1.;
                g = 0.;
                b = q;
            }
            _ => panic!(),
        }
        Self {
            red: !!((r * 235.) as u8),
            green: !!((g * 235.) as u8),
            blue: !!((b * 235.) as u8),
        }
    }
}

#[derive(Debug)]
pub struct ControllerMetaData {
    pub vendor: String,
    pub description: String,
    pub version: String,
    pub serial: String,
    pub location: String,
}

#[derive(Debug)]
pub struct LedData {
    pub name: String,
    pub value: u32,
}

#[derive(Debug)]
pub struct ControllerData {
    pub id: u32,
    pub name: String,
    pub metadata: ControllerMetaData,
    pub device_type: u32,
    pub leds: Vec<LedData>,
}

macro_rules! unwrap_data {
    ( $e:expr ) => {
        match $e {
            Ok(x) => x,
            Err(e) => return Err(io::Error::new(io::ErrorKind::InvalidData, e)),
        }
    };
}

macro_rules! try_data {
    ( $e:expr ) => {
        match $e.try_into() {
            Ok(x) => x,
            Err(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid data received",
                ))
            }
        }
    };
}

macro_rules! check_batch {
    ( $e:expr ) => {
        if $e.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                ERR_CONTROLLER_NOT_FOUND,
            ));
        }
    };
}

fn parse_string(pos: usize, data: &[u8]) -> Result<(usize, String), io::Error> {
    let string_len = u16::from_le_bytes(try_data!(data[pos..pos + 2])) as usize;
    let result = unwrap_data!(String::from_utf8(try_data!(
        data[pos + 2..pos + 1 + string_len]
    )));
    Ok((pos + string_len + 2, result))
}

impl ControllerData {
    fn unpack(id: u32, data: &[u8]) -> Result<Self, io::Error> {
        let device_type = u32::from_le_bytes(try_data!(data[4..8]));
        let (pos, name) = parse_string(8, data)?;
        let (pos, vendor) = parse_string(pos, data)?;
        let (pos, description) = parse_string(pos, data)?;
        let (pos, version) = parse_string(pos, data)?;
        let (pos, serial) = parse_string(pos, data)?;
        let (mut pos, location) = parse_string(pos, data)?;
        let num_modes = u16::from_le_bytes(try_data!(data[pos..pos + 2]));
        let _active_mode = i32::from_le_bytes(try_data!(data[pos + 2..pos + 6]));
        pos += 6;
        for _ in 0..num_modes {
            let (p, _mode_name) = parse_string(pos, data)?;
            pos = p;
            let num_colors = u16::from_le_bytes(try_data!(data[pos + 36..pos + 38]));
            pos = pos + 38 + num_colors as usize * 4;
            if device_type == 1 {
                //println!("{}", num_colors);
            }
        }
        let num_zones = u16::from_le_bytes(try_data!(data[pos..pos + 2]));
        pos += 2;
        for _ in 0..num_zones {
            let (p, _zone_name) = parse_string(pos, data)?;
            pos = p + 18;
            if data[pos] == 2 {
                // ZoneType Matrix, untested
                let height = u32::from_le_bytes(try_data!(data[pos..pos + 4]));
                let width = u32::from_le_bytes(try_data!(data[pos + 4..pos + 8]));
                pos += height as usize * width as usize * 4;
            }
        }
        let num_leds = u16::from_le_bytes(try_data!(data[pos..pos + 2]));
        pos += 2;
        let mut leds: Vec<LedData> = Vec::new();
        for _ in 0..num_leds {
            let (p, led_name) = parse_string(pos, data)?;
            pos = p;
            let value = u32::from_le_bytes(try_data!(data[pos..pos + 4]));
            pos += 4;
            leds.push(LedData {
                name: led_name,
                value,
            })
        }
        Ok(Self {
            id,
            name,
            device_type,
            metadata: ControllerMetaData {
                vendor,
                description,
                version,
                serial,
                location,
            },
            leds,
        })
    }
}

pub struct OpenRGBClient {
    stream: Option<TcpStream>,
    path: String,
    pub retries: u8,
    pub timeout: Duration,
    pub controllers: Vec<ControllerData>,
    pub server_protocol: Option<u32>,
}

struct ControllerLedSetCommand {
    controller_id: u32,
    end: u16,
}

impl OpenRGBClient {
    pub fn new() -> Self {
        Self {
            stream: None,
            path: String::new(),
            retries: 3,
            timeout: Duration::from_secs(2),
            controllers: Vec::new(),
            server_protocol: None,
        }
    }

    pub fn set_path(&mut self, path: &str) {
        self.path = path.to_owned();
        self.stream = None;
        debug!("ORGB server path set: {}", self.path);
    }

    fn get_stream(&mut self) -> Result<&mut TcpStream, io::Error> {
        match self.stream {
            Some(ref mut v) => Ok(v),
            None => {
                let stream = match TcpStream::connect(&self.path) {
                    Ok(v) => v,
                    Err(e) => {
                        error!("ORGB server {} error: {}", &self.path, e);
                        return Err(e);
                    }
                };
                stream.set_read_timeout(Some(self.timeout))?;
                stream.set_write_timeout(Some(self.timeout))?;
                self.stream = Some(stream);
                debug!("ORGB server connected: {}", self.path);
                Ok(self.stream.as_mut().unwrap())
            }
        }
    }

    pub fn call(
        &mut self,
        device_id: u32,
        packet_type: u32,
        data: &[u8],
    ) -> Result<Option<Vec<u8>>, std::io::Error> {
        let mut attempt = 0;
        loop {
            match self._call(device_id, packet_type, data) {
                Ok(v) => return Ok(v),
                Err(e) => {
                    attempt += 1;
                    if attempt > self.retries {
                        return Err(e);
                    } else {
                        self.stream = None;
                    }
                }
            }
        }
    }

    pub fn _call(
        &mut self,
        device_id: u32,
        packet_type: u32,
        data: &[u8],
    ) -> Result<Option<Vec<u8>>, std::io::Error> {
        let stream = self.get_stream()?;
        let mut request = Vec::new();
        request.extend_from_slice(&HEADER);
        request.extend_from_slice(&device_id.to_le_bytes());
        request.extend_from_slice(&packet_type.to_le_bytes());
        request.extend_from_slice(&(data.len() as u32).to_le_bytes());
        request.extend_from_slice(data);
        stream.write(&request)?;
        if packet_type == REQ_SET_CLIENT_NAME || packet_type == REQ_RGBCONTROLLER_UPDATELEDS {
            return Ok(None);
        }
        let mut buf = [0u8; 16];
        stream.read_exact(&mut buf)?;
        let r_device_id = u32::from_le_bytes(try_data!(buf[4..8]));
        let r_packet_type = u32::from_le_bytes(try_data!(buf[8..12]));
        if buf[..4] != HEADER || r_device_id != device_id || r_packet_type != packet_type {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid server response",
            ));
        }
        let r_len = u32::from_le_bytes(try_data!(buf[12..16]));
        let mut response = vec![0u8; r_len as usize];
        stream.read_exact(&mut response)?;
        Ok(Some(response))
    }

    pub fn load(&mut self) -> Result<(), io::Error> {
        self.controllers.clear();
        let data = self.call(
            0,
            REQ_REQUEST_PROTOCOL_VERSION,
            &PROTOCOL_VERSION.to_le_bytes(),
        )?;
        let server_protocol_version = u32::from_le_bytes(try_data!(data.unwrap()));
        if server_protocol_version != PROTOCOL_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Server protocol unsupported",
            ));
        }
        self.server_protocol = Some(server_protocol_version);
        let mut buf = Vec::new();
        buf.extend_from_slice(CLIENT_NAME.as_bytes());
        buf.push(32);
        buf.extend_from_slice(VERSION.as_bytes());
        buf.push(0);
        self.call(0, REQ_SET_CLIENT_NAME, &buf)?;
        let data = self.call(0, REQ_REQUEST_CONTROLLER_COUNT, &buf)?;
        let controller_count = u32::from_le_bytes(try_data!(data.unwrap()));
        debug!("{} controller(s) found", controller_count);
        for i in 0..controller_count {
            let data = self
                .call(
                    i,
                    REQ_REQUEST_CONTROLLER_DATA,
                    &PROTOCOL_VERSION.to_le_bytes(),
                )
                .unwrap();
            let c = ControllerData::unpack(i, &data.unwrap())?;
            debug!("controller loaded: {:?}", c);
            self.controllers.push(c);
        }
        Ok(())
    }

    pub fn reload(&mut self) -> Result<(), io::Error> {
        self.stream = None;
        debug!("reloading");
        self.load()
    }

    pub fn set_color_by_id(
        &mut self,
        controller_id: u32,
        color: &RGBColor,
    ) -> Result<(), io::Error> {
        let mut to_set = Vec::new();
        for c in &self.controllers {
            if c.id == controller_id {
                to_set.push(ControllerLedSetCommand {
                    controller_id: c.id,
                    end: c.leds.len() as u16,
                });
                break;
            }
        }
        check_batch!(to_set);
        self.set_color_for_controllers(&to_set, color)
    }

    pub fn set_color_by_name(
        &mut self,
        controller_name: &str,
        color: &RGBColor,
    ) -> Result<(), io::Error> {
        let mut to_set = Vec::new();
        for c in &self.controllers {
            if c.name == controller_name {
                to_set.push(ControllerLedSetCommand {
                    controller_id: c.id,
                    end: c.leds.len() as u16,
                });
            }
        }
        check_batch!(to_set);
        self.set_color_for_controllers(&to_set, color)
    }

    pub fn set_color_by_device_types(
        &mut self,
        device_types: &Vec<u32>,
        color: &RGBColor,
    ) -> Result<(), io::Error> {
        let mut found = false;
        for d in device_types {
            match self.set_color_by_device_type(*d, color) {
                Ok(_) => found = true,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        match found {
            true => Ok(()),
            false => Err(io::Error::new(
                io::ErrorKind::NotFound,
                ERR_CONTROLLER_NOT_FOUND,
            )),
        }
    }

    pub fn set_color_by_device_type(
        &mut self,
        device_type: u32,
        color: &RGBColor,
    ) -> Result<(), io::Error> {
        let mut to_set = Vec::new();
        for c in &self.controllers {
            if c.device_type == device_type {
                to_set.push(ControllerLedSetCommand {
                    controller_id: c.id,
                    end: c.leds.len() as u16,
                });
            }
        }
        check_batch!(to_set);
        self.set_color_for_controllers(&to_set, color)
    }

    pub fn set_color(&mut self, color: &RGBColor) -> Result<(), io::Error> {
        let mut to_set = Vec::new();
        for c in &self.controllers {
            to_set.push(ControllerLedSetCommand {
                controller_id: c.id,
                end: c.leds.len() as u16,
            });
        }
        self.set_color_for_controllers(&to_set, color)
    }

    fn set_color_for_controllers(
        &mut self,
        cmd: &Vec<ControllerLedSetCommand>,
        color: &RGBColor,
    ) -> Result<(), io::Error> {
        for c in cmd {
            let mut data: Vec<u8> = Vec::new();
            data.extend_from_slice(&((4 * c.end + 6) as u32).to_le_bytes());
            data.extend_from_slice(&c.end.to_le_bytes());
            for _ in 0..c.end {
                data.push(color.red);
                data.push(color.green);
                data.push(color.blue);
                data.push(0x00); // X
            }
            self.call(c.controller_id, REQ_RGBCONTROLLER_UPDATELEDS, &data)?;
        }
        Ok(())
    }
}
