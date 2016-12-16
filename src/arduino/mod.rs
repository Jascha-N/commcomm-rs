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
            .chain_err(|| "Kon seriële poorten niet opvragen")?;
        Ok(ports.iter().map(|port| Port(OsString::from(port))).collect())
    }

    pub fn name(&self) -> &OsStr {
        &self.0
    }

    pub fn open(&self) -> Result<SystemPort> {
        serial::open(&self.name()).chain_err(|| "Seriële poort kon niet worden geopend")
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

        CODES.get(code as usize).map_or_else(|| bail!("Onbekende foutcode: {}", code), |code| Ok(*code))
    }
}

impl Display for ResponseCode {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        let message = match *self {
            ResponseCode::JsonParse => "Kon de aanvraag niet parseren",
            ResponseCode::JsonAlloc => "De JSON-buffer is vol",
            ResponseCode::RequestTooLong => "Aanvraagtekst te lang",
            ResponseCode::UnknownCommand => "Onbekend commando",
            ResponseCode::BufferTooSmall => "Antwoordbuffer te klein",
            ResponseCode::InvalidParam => "Ongeldige invoer"
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
    limits: (u16, u16),
    thresholds: (u8, u8)
}

impl SensorConfig {
    pub fn new(min: u16, max: u16) -> SensorConfig {
        debug_assert!(min < max);
        debug_assert!(max < 1024);
        SensorConfig {
            limits: (min, max),
            thresholds: (u8::MIN, u8::MAX)
        }
    }

    pub fn limits(&self) -> (u16, u16) {
        self.limits
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
        info!("Arduino wordt gereset.");
        let mut port = port.open()?;
        port.reconfigure(&|settings| {
            settings.set_baud_rate(serial::Baud1200)?;
            Ok(())
        }).chain_err(|| "Seriële poort kon niet worden ingesteld")?;
        port.set_dtr(false).chain_err(|| "Seriële fout")?;

        Ok(())
    }

    fn wait_for_bootloader(mut prev_ports: BTreeSet<Port>) -> Result<Option<Port>> {
        info!("Bezig met het wachten op de bootloader-poort.");
        let start = Instant::now();
        while Instant::now().duration_since(start).as_secs() < 10 {
            let ports = BTreeSet::from_iter(Port::enumerate()?);
            {
                let new_port = ports.difference(&prev_ports).next();
                if let Some(new_port) = new_port {
                    info!("Bootloader-poort gevonden: {}.", new_port);
                    return Ok(Some(new_port.clone()));
                }
            }

            prev_ports = ports;

            stdthread::sleep(Duration::from_millis(100));
        }
        warn!("Time-out bij het wachten op de bootloader-poort.");
        Ok(None)
    }

    fn run_avrdude(port: &Port) -> Result<()> {
        let temp_dir = TempDir::new(env!("CARGO_PKG_NAME"))
            .chain_err(|| "Tijdelijke map kon niet worden gemaakt")?;

        let avrdude_conf_path = temp_dir.path().join("avrdude.conf");
        File::create(&avrdude_conf_path).and_then(|mut file| {
            file.write_all(board::AVRDUDE_CONFIG)
        }).chain_err(|| "Kon het AVRDude-configuratiebestand niet wegschrijven")?;

        let program_path = temp_dir.path().join("program.hex");
        File::create(&program_path).and_then(|mut file| {
            file.write_all(board::PROGRAM)
        }).chain_err(|| "Kon de schets niet wegschrijven")?;

        info!("Het AVRDude-proces wordt gestart.");
        let mut process = Command::new("avrdude")
                                  .arg("-v").arg("-v")
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
                                  .chain_err(|| "Kon het AVRDude-proces niet starten")?;

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
                            .chain_err(|| "Fout bij het wachten op het AVRDude-proces")?;
        if let Some(status) = status {
            if status.success() {
                Ok(())
            } else {
                Err(format!("Het uploaden met AVRDude is mislukt met de foutcode: {}",
                            status.code().map_or("<none>".to_string(), |code| code.to_string())).into())
            }
        } else {
            process.kill().chain_err(|| "Kon het AVRDude-proces niet beëindigen")?;
            bail!("Time-out bij het wachten op het AVRDude-proces");
        }
    }

    fn wait_for_sketch(port: &Port) -> Result<bool> {
        info!("Bezig met het wachten op de schetspoort.");
        let start = Instant::now();
        while Instant::now().duration_since(start).as_secs() < 2 {
            if Port::enumerate()?.contains(port) {
                info!("Schetspoort gevonden: {}.", port);
                return Ok(true);
            }
            stdthread::sleep(Duration::from_millis(100));
        }

        warn!("Time-out bij het wachten op de schetspoort.");
        Ok(false)
    }

    pub fn upload(port: &Port) -> Result<Cow<Port>> {
        info!("Bezig met het voorbereiden om de schets te uploaden.");
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
        info!("Upload succesvol.");

        let sketch_port = if board::WAIT_FOR_UPLOAD_PORT && Arduino::wait_for_sketch(port)? {
            Cow::Borrowed(port)
        } else {
            bootloader_port
        };

        Ok(sketch_port)
    }

    fn verify(&mut self) -> Result<()> {
        info!("Bezig met het verifiëren van de schets.");
        self.device_info().chain_err(|| ErrorKind::ArduinoVerification(None)).and_then(|info| {
            if let Some(info) = info {
                info!("Apparaatinformatie ontvangen.");
                info!("Apparaatnaam: {}.", info.name());
                info!("Schetsversie: {}.", info.version());
                info!("Timestamp: {}.", info.timestamp().format("%Y-%m-%d %H:%M:%S"));

                if info != *board::DEVICE_INFO {
                    bail!(ErrorKind::ArduinoVerification(Some("Apparaatinformatie komt niet overeen".to_string())));
                }

                info!("Verificatie succesvol.");
            } else {
                info!("Geen apparaatinformatie beschikbaar; verificatie wordt overgeslagen.");
            }

            Ok(())
        })
    }

    pub fn open(port: &Port, verify: bool) -> Result<Arduino> {
        info!("Bezig met het openen van de schetspoort op {}.", port);
        let mut serial = port.open()?;
        serial.reconfigure(&|settings| {
            settings.set_baud_rate(serial::Baud115200)?;
            settings.set_char_size(serial::Bits8);
            settings.set_parity(serial::ParityNone);
            settings.set_stop_bits(serial::Stop1);
            Ok(())
        }).chain_err(|| "Seriële poort kon niet worden ingesteld")?;

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
            .chain_err(|| ErrorKind::Io("Aanvraag kon niet worden verzonden naar de Arduino".to_string()))?;
        let _ = serial.flush();
        debug!("Aanvraag verstuurd: {}.", json);

        let mut buffer = String::with_capacity(20);
        for byte in serial.bytes() {
            let byte = byte.chain_err(|| ErrorKind::Io("Antwoord van de Arduino kon niet worden ontvangen".to_string()))?;
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
        debug!("Antwoord ontvangen: {}.", buffer);
        let result = serde_json::from_str::<Value>(&buffer).chain_err(|| "Kon het antwoord niet parseren")?;
        if result.is_i64() || result.is_u64() {
            let code = ResponseCode::from_code(result.as_u64().unwrap())?;

            Err(ErrorKind::ArduinoResponse(command.to_string(), code).into())
        } else {
            serde_json::from_value(result).chain_err(|| "Kon het antwoord niet deserialiseren")
        }
    }

    pub fn device_info(&mut self) -> Result<Option<DeviceInfo>> {
        self.send_request("device_info", &[])
    }

    pub fn poll_events(&mut self) -> Result<Vec<Event>> {
        self.send_request("poll_events", &[])
    }

    pub fn raw_values(&mut self) -> Result<Vec<Option<u8>>> {
        self.send_request("raw_values", &[])
    }

    pub fn set_sensor(&mut self, id: u8, config: &SensorConfig) -> Result<()> {
        let (min, max) = config.limits();
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