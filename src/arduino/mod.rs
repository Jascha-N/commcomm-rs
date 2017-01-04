use error::*;

use chrono::{DateTime, Local, NaiveDateTime, TimeZone};

use serde::{Deserialize, Deserializer};
use serde::de::{Error as DeserializeError, Visitor};

use serde_json::{self, Value};
use serde_json::builder::ObjectBuilder;

use serial::{self, SerialPort, SystemPort};
use serial_enumerate;

use tempdir::TempDir;

use wait_timeout::ChildExt;

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fmt::{self, Display, Formatter};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::iter::FromIterator;
use std::process::{Command, Stdio};
use std::result::Result as StdResult;
use std::time::{Duration, Instant};
use std::thread as stdthread;
use std::u8;

pub mod thread;

mod board {
    include!(concat!(env!("OUT_DIR"), "/board.rs"));
}



#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Port(OsString);

impl Port {
    pub fn new<N: Into<OsString>>(name: N) -> Port {
        Port(name.into())
    }

    pub fn enumerate() -> Result<Vec<Port>> {
        let ports = serial_enumerate::enumerate_serial_ports()
            .chain_err(|| t!("Could not enumerate serial ports"))?;
        Ok(ports.iter().map(|port| Port(OsString::from(port))).collect())
    }

    pub fn name(&self) -> &OsStr {
        &self.0
    }

    pub fn open(&self) -> Result<SystemPort> {
        serial::open(&self.name()).chain_err(|| t!("Serial port could not be opened"))
    }
}

impl Display for Port {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        write!(fmt, "{}", self.name().to_string_lossy())
    }
}

#[derive(Debug, Deserialize)]
pub enum Event {
    #[serde(rename = "flexed")]
    SensorFlexed(u8),
    #[serde(rename = "extended")]
    SensorExtended(u8)
}

#[derive(Clone, Copy, Debug)]
pub enum ResponseCode {
    JsonParse,
    JsonAlloc,
    RequestTooLong,
    UnknownCommand,
    BufferTooSmall,
    InvalidParam
}

impl ResponseCode {
    fn from_code(code: u64) -> Result<ResponseCode> {
        const CODES: &'static [ResponseCode] = &[
            ResponseCode::JsonParse,
            ResponseCode::JsonAlloc,
            ResponseCode::RequestTooLong,
            ResponseCode::UnknownCommand,
            ResponseCode::BufferTooSmall,
            ResponseCode::InvalidParam
        ];

        CODES.get(code as usize)
             .cloned()
             .map_or_else(|| bail!(t!("Unknown error code: {}"), code), Ok)
    }
}

impl Display for ResponseCode {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        let message = match *self {
            ResponseCode::JsonParse => t!("Could not parse the request"),
            ResponseCode::JsonAlloc => t!("JSON buffer is full"),
            ResponseCode::RequestTooLong => t!("Request too long"),
            ResponseCode::UnknownCommand => t!("Unknown command"),
            ResponseCode::BufferTooSmall => t!("Response buffer too small"),
            ResponseCode::InvalidParam => t!("Illegal parameter")
        };
        write!(fmt, "{}", message)
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
pub struct DeviceInfo {
    name: Cow<'static, str>,
    version: Cow<'static, str>,
    #[serde(deserialize_with = "DeviceInfo::deserialize_timestamp")]
    timestamp: DateTime<Local>
}

impl DeviceInfo {
    fn new(name: &'static str, version: &'static str, timestamp: i64) -> DeviceInfo {
        DeviceInfo {
            name: Cow::Borrowed(name),
            version: Cow::Borrowed(version),
            timestamp: DeviceInfo::make_timestamp(timestamp)
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn timestamp(&self) -> &DateTime<Local> {
        &self.timestamp
    }

    fn make_timestamp(timestamp: i64) -> DateTime<Local> {
        Local.from_utc_datetime(&NaiveDateTime::from_timestamp(timestamp, 0))
    }

    fn deserialize_timestamp<D: Deserializer>(deserializer: &mut D) -> StdResult<DateTime<Local>, D::Error> {
        struct TimestampVisitor;

        impl Visitor for TimestampVisitor {
            type Value = i64;

            fn visit_i64<E: DeserializeError>(&mut self, v: i64) -> StdResult<i64, E> {
                Ok(v)
            }

            fn visit_u64<E: DeserializeError>(&mut self, v: u64) -> StdResult<i64, E> {
                Ok(v as i64)
            }
        }
        deserializer.deserialize(TimestampVisitor).map(DeviceInfo::make_timestamp)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SensorConfig {
    range: (u16, u16),
    thresholds: (u8, u8)
}

impl SensorConfig {
    pub fn new(min: u16, max: u16) -> SensorConfig {
        debug_assert!(max < 1024);
        debug_assert!(min < 1024);
        SensorConfig {
            range: (min, max),
            thresholds: (u8::MIN, u8::MAX)
        }
    }

    pub fn range(&self) -> (u16, u16) {
        self.range
    }

    pub fn thresholds(&self) -> (u8, u8) {
        self.thresholds
    }

    pub fn set_thresholds(&mut self, low: u8, high: u8) {
        debug_assert!(low < high);
        self.thresholds = (low, high);
    }
}



pub struct Arduino(SystemPort);

impl Arduino {
    fn cdc_reset(port: &Port) -> Result<()> {
        info!(t!("Arduino is being reset."));
        let mut port = port.open()?;
        port.reconfigure(&|settings| {
            settings.set_baud_rate(serial::Baud1200)?;
            Ok(())
        }).chain_err(|| t!("Serial port could not be configured"))?;
        port.set_dtr(false).chain_err(|| t!("Serial error"))?;

        Ok(())
    }

    fn wait_for_bootloader(mut prev_ports: BTreeSet<Port>) -> Result<Option<Port>> {
        info!(t!("Waiting for the bootloader port."));
        let start = Instant::now();
        while Instant::now().duration_since(start).as_secs() < 10 {
            let ports = BTreeSet::from_iter(Port::enumerate()?);
            {
                let new_port = ports.difference(&prev_ports).next();
                if let Some(new_port) = new_port {
                    info!(t!("Bootloader port found: {}."), new_port);
                    return Ok(Some(new_port.clone()));
                }
            }

            prev_ports = ports;

            stdthread::sleep(Duration::from_millis(100));
        }
        warn!(t!("Waiting for bootloader port timed out."));
        Ok(None)
    }

    fn run_avrdude(port: &Port) -> Result<()> {
        let temp_dir = TempDir::new(env!("CARGO_PKG_NAME"))
            .chain_err(|| t!("Temporary folder could not be created"))?;

        let avrdude_conf_path = temp_dir.path().join("avrdude.conf");
        File::create(&avrdude_conf_path).and_then(|mut file| {
            file.write_all(board::AVRDUDE_CONFIG)
        }).chain_err(|| t!("Could not write to the AVRDUDE configuration file"))?;

        let program_path = temp_dir.path().join("program.hex");
        File::create(&program_path).and_then(|mut file| {
            file.write_all(board::PROGRAM)
        }).chain_err(|| t!("Could not write to the sketch file"))?;

        info!(t!("The AVRDUDE process is being started."));
        let mut command = Command::new("avrdude");

        #[cfg(windows)]
        ::std::os::windows::process::CommandExt::creation_flags(&mut command, ::winapi::CREATE_NO_WINDOW);

        let mut process = command.arg("-v").arg("-v")
                                 .arg("-C").arg(avrdude_conf_path)
                                 .arg("-p").arg(board::MCU)
                                 .arg("-c").arg(board::PROTOCOL)
                                 .arg("-P").arg(port.name())
                                 .arg("-b").arg(format!("{}", board::SPEED))
                                 .arg("-D")
                                 .arg(format!("-Uflash:w:{}:i", program_path.display()))
                                 .stderr(Stdio::piped())
                                 .stdout(Stdio::null())
                                 .stdin(Stdio::null())
                                 .spawn()
                                 .chain_err(|| t!("Could not start the AVRDUDE process"))?;

        let stderr = process.stderr.take().unwrap();
        stdthread::spawn(move || {
            let reader = BufReader::new(stderr);

            for line in reader.split(b'\n').filter_map(|line| line.ok()) {
                let line = String::from_utf8_lossy(&line);
                if line.starts_with("avrdude: ") {
                    info!(target: "<avrdude>", "{}", &line[9..]);
                }
            }
        });

        let status = process.wait_timeout(Duration::from_secs(60))
                            .chain_err(|| t!("Error while waiting for the AVRDUDE process"))?;
        if let Some(status) = status {
            if status.success() {
                Ok(())
            } else {
                Err(format!(t!("Uploading with AVRDUDE failed with error code: {}"),
                            status.code().map_or("<none>".to_string(), |code| code.to_string())).into())
            }
        } else {
            process.kill().chain_err(|| t!("Could not kill the AVRDUDE process"))?;
            bail!(t!("Waiting for the AVRDUDE process to finish timed out"));
        }
    }

    fn wait_for_sketch(port: &Port) -> Result<bool> {
        info!(t!("Waiting for the sketch port."));
        let start = Instant::now();
        while Instant::now().duration_since(start).as_secs() < 2 {
            if Port::enumerate()?.contains(port) {
                info!(t!("Found sketch port: {}."), port);
                return Ok(true);
            }
            stdthread::sleep(Duration::from_millis(100));
        }

        warn!(t!("Waiting for sketch port timed out."));
        Ok(false)
    }

    pub fn upload(port: &Port) -> Result<Cow<Port>> {
        info!(t!("Preparing to upload the sketch."));
        let bootloader_port = if board::USE_1200BPS_TOUCH {
            let ports = BTreeSet::from_iter(Port::enumerate()?);
            Arduino::cdc_reset(port)?;
            if board::WAIT_FOR_UPLOAD_PORT {
                Arduino::wait_for_bootloader(ports)?
                        .map_or(Cow::Borrowed(port), Cow::Owned)
            } else {
                Cow::Borrowed(port)
            }
        } else {
            Cow::Borrowed(port)
        };

        Arduino::run_avrdude(&bootloader_port)?;
        info!(t!("Upload successful."));

        let sketch_port = if board::WAIT_FOR_UPLOAD_PORT && Arduino::wait_for_sketch(port)? {
            Cow::Borrowed(port)
        } else {
            bootloader_port
        };

        Ok(sketch_port)
    }

    fn verify(&mut self) -> Result<()> {
        info!(t!("Verifying sketch."));
        self.device_info().chain_err(|| ErrorKind::ArduinoVerification(None)).and_then(|info| {
            if let Some(info) = info {
                info!(t!("Device information received."));
                info!(t!("Device name: {}."), info.name());
                info!(t!("Sketch version: {}."), info.version());
                info!(t!("Timestamp: {}."), info.timestamp().format(t!("%Y-%m-%d %H:%M:%S")));

                if info != *board::DEVICE_INFO {
                    bail!(ErrorKind::ArduinoVerification(Some(t!("Device information does not match").to_string())));
                }

                info!(t!("Verification successful."));
            } else {
                info!(t!("No device information available; skipping verification."));
            }

            Ok(())
        })
    }

    pub fn open(port: &Port, verify: bool) -> Result<Arduino> {
        info!(t!("Opening sketch port on {}."), port);
        let mut serial = port.open()?;
        serial.reconfigure(&|settings| {
            settings.set_baud_rate(serial::Baud115200)?;
            settings.set_char_size(serial::Bits8);
            settings.set_parity(serial::ParityNone);
            settings.set_stop_bits(serial::Stop1);
            Ok(())
        }).and_then(|_| {
            serial.set_timeout(Duration::from_millis(100))
        }).chain_err(|| t!("Serial port could not be configured"))?;

        let mut buffer = Vec::new();
        let _ = serial.read_to_end(&mut buffer);

        let mut arduino = Arduino(serial);
        if verify {
            arduino.verify()?;
        }
        Ok(arduino)
    }

    fn send_request<D: Deserialize>(&mut self, command: &str, parameters: &[(&str, Value)]) -> Result<D> {
        let &mut Arduino(ref mut serial) = self;

        let mut builder = ObjectBuilder::new();
        for &(key, ref value) in parameters {
            builder = builder.insert(key, value);
        }
        let request = builder.insert("command", command).build();

        let json = serde_json::to_string(&request).unwrap();
        writeln!(serial, "{}", json)
            .chain_err(|| ErrorKind::Io(t!("Request could not be sent to the Arduino").to_string()))?;
        let _ = serial.flush();
        debug!(t!("Request sent: {}."), json);

        let mut buffer = String::with_capacity(20);
        for byte in serial.bytes() {
            let byte = byte.chain_err(|| ErrorKind::Io(t!("Arduino response could not be received").to_string()))?;
            match byte {
                b'\r' => {}
                b'\n' => {
                    break;
                }
                b => {
                    buffer.push(b.into());
                }
            }
        }
        debug!(t!("Response received: {}."), buffer);
        let result = serde_json::from_str::<Value>(&buffer).chain_err(|| t!("Could not parse response"))?;
        if result.is_i64() || result.is_u64() {
            let code = ResponseCode::from_code(result.as_u64().unwrap())?;

            Err(ErrorKind::ArduinoResponse(command.to_string(), code).into())
        } else {
            serde_json::from_value(result).chain_err(|| t!("Could not deserialize the response"))
        }
    }

    pub fn device_info(&mut self) -> Result<Option<DeviceInfo>> {
        self.send_request("device_info", &[])
    }

    pub fn poll_event(&mut self) -> Result<Option<Event>> {
        self.send_request("poll_event", &[])
    }

    pub fn sensor_values(&mut self, raw: bool) -> Result<Vec<Option<u16>>> {
        self.send_request("sensor_values", &[("raw", serde_json::to_value(raw))])
    }

    pub fn set_sensor(&mut self, id: u8, config: &SensorConfig) -> Result<()> {
        let (min, max) = config.range();
        let (low, high) = config.thresholds();

        self.send_request("set_sensor", &[
            ("id", serde_json::to_value(id)),
            ("min", serde_json::to_value(min)),
            ("max", serde_json::to_value(max)),
            ("low", serde_json::to_value(low)),
            ("high", serde_json::to_value(high))
        ])
    }

    pub fn unset_sensor(&mut self, id: u8) -> Result<()> {
        self.send_request("unset_sensor", &[
            ("id", serde_json::to_value(id))
        ])
    }
}

impl Drop for Arduino {
    fn drop(&mut self) {
        let _ = self.0.set_dtr(false);
    }
}
