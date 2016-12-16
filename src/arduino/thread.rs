use super::{Arduino, Event, SensorConfig, Port};
use error::*;

use std::fmt::Write;
use std::mem;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

enum Command {
    SetSensor(u8, SensorConfig)
}

pub struct PollEvents<'a>(&'a Receiver<Event>);

impl<'a> Iterator for PollEvents<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        self.0.try_recv().ok()
    }
}

pub struct ArduinoController {
    handle: Option<JoinHandle<()>>,
    connected: Arc<AtomicBool>,
    command_sender: Option<SyncSender<Command>>,
    event_receiver: Option<Receiver<Event>>,
    sensor_config: Arc<Mutex<Vec<Option<SensorConfig>>>>
}

impl ArduinoController {
    pub fn new(port: Port, sensor_config: Vec<Option<SensorConfig>>) -> ArduinoController {
        let (event_sender, event_receiver) = mpsc::sync_channel(10);
        let (command_sender, command_receiver) = mpsc::sync_channel(10);

        let connected = Arc::new(AtomicBool::new(false));
        let sensor_config = Arc::new(Mutex::new(sensor_config));
        let controller = ArduinoThread {
            arduino: None,
            upload_tried: false,
            port: port,
            connected: connected.clone(),
            event_sender: event_sender,
            command_receiver: command_receiver,
            sensor_config: sensor_config.clone()
        };

        ArduinoController {
            handle: Some(controller.run()),
            connected: connected,
            command_sender: Some(command_sender),
            event_receiver: Some(event_receiver),
            sensor_config: sensor_config
        }
    }

    pub fn connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    pub fn poll_events(&self) -> PollEvents {
        PollEvents(self.event_receiver.as_ref().unwrap())
    }

    pub fn set_sensor(&self, id: u8, config: SensorConfig) -> Result<()> {
        self.command_sender.as_ref().unwrap().send(Command::SetSensor(id, config))
            .chain_err(|| "Kon sensorinstelling niet wijzigen")
    }

    pub fn sensor(&self, id: u8) -> Option<SensorConfig> {
        self.sensor_config.lock().unwrap().get(id as usize).and_then(|s| *s)
    }
}

impl Drop for ArduinoController {
    fn drop(&mut self) {
        if !thread::panicking() {
            mem::drop(self.command_sender.take());
            mem::drop(self.event_receiver.take());
            info!("Bezig met wachten op Arduino-thread.");
            let _ = self.handle.take().unwrap().join();
        }
    }
}

struct ArduinoThread {
    arduino: Option<Arduino>,
    upload_tried: bool,
    port: Port,
    connected: Arc<AtomicBool>,
    event_sender: SyncSender<Event>,
    command_receiver: Receiver<Command>,
    sensor_config: Arc<Mutex<Vec<Option<SensorConfig>>>>
}

impl ArduinoThread {
    fn run(mut self) -> JoinHandle<()> {
        thread::spawn(move || {
            while !self.stopping() {
                self.arduino = match self.restart() {
                    Ok(arduino) => Some(arduino),
                    Err(error) => {
                        log_full_error(&error);
                        info!("Opnieuw proberen over 5 seconden.");
                        thread::sleep(Duration::from_secs(5));
                        continue;
                    }
                };
                self.connected.store(true, Ordering::Relaxed);
                loop {
                    match self.process_commands() {
                        Ok(_) => {
                            break;
                        }
                        Err(error) => {
                            log_full_error(&error);
                            if let ErrorKind::Io(_) = *error.kind() {
                                self.connected.store(false, Ordering::Relaxed);
                                info!("Opnieuw proberen over 5 seconden.");
                                thread::sleep(Duration::from_secs(5));
                                break;
                            } else {
                                info!("Opnieuw proberen over 1 seconde.");
                                thread::sleep(Duration::from_secs(1));
                            }
                        }
                    }
                }
            }

            info!("Arduino-thread is gestopt.");
        })
    }

    fn stopping(&mut self) -> bool {
        loop {
            match self.command_receiver.try_recv() {
                Ok(_) => {}
                Err(TryRecvError::Empty) => {
                    return false;
                }
                Err(TryRecvError::Disconnected) => {
                    return true;
                }
            }
        }
    }

    fn restart(&mut self) -> Result<Arduino> {
        Arduino::open(&self.port, true).or_else(|error| match error {
            Error(ErrorKind::ArduinoVerification(_), _) if !self.upload_tried => {
                self.upload_tried = true;
                log_full_error(&error);
                info!("Eenmalig proberen om de schets opnieuw te uploaden.");
                self.port = Arduino::upload(&self.port)?.into_owned();
                Arduino::open(&self.port, true)
            }
            error => Err(error)
        }).and_then(|mut arduino| {
            for (id, sensor) in self.sensor_config.lock().unwrap().iter().enumerate() {
                if let Some(ref sensor) = *sensor {
                    arduino.set_sensor(id as u8, sensor)?;
                }
            }
            Ok(arduino)
        })
    }

    fn process_commands(&mut self) -> Result<()> {
        let arduino = self.arduino.as_mut().unwrap();

        'outer: loop {
            match self.command_receiver.try_recv() {
                Ok(Command::SetSensor(id, config)) => {
                    arduino.set_sensor(id, &config)?;
                }
                Err(TryRecvError::Empty) => {
                    let events = arduino.poll_events()?;
                    for event in events {
                        match self.event_sender.try_send(event) {
                            Ok(()) => {}
                            error @ Err(TrySendError::Full(_)) => {
                                return error.chain_err(|| "Gebeurtenisbuffer is vol");
                            }
                            Err(TrySendError::Disconnected(_)) => {
                                break 'outer;
                            }
                        }
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    break;
                }
            }
        }

        Ok(())
    }
}

fn log_full_error(error: &Error) {
    let mut chain = error.iter();
    let mut message = String::new();

    let first = chain.next().unwrap();
    let _ = write!(message, "{}", first);
    for cause in chain {
        let _ = write!(message, ": {}", cause);
    }

    error!("{}.", message);
}