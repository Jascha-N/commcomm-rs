use super::{Arduino, Event, Port};
use error::*;

use std::fmt::Write;
use std::mem;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

enum Command {
    SetThresholds {
        id: u8,
        trigger: u8,
        release: u8
    }
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
    sensor_thresholds: Arc<Mutex<Vec<(u8, u8)>>>
}

impl ArduinoController {
    pub fn new(port: Port, sensor_thresholds: Vec<(u8, u8)>) -> ArduinoController {
        let (event_sender, event_receiver) = mpsc::sync_channel(10);
        let (command_sender, command_receiver) = mpsc::sync_channel(10);

        let connected = Arc::new(AtomicBool::new(false));
        let sensor_thresholds = Arc::new(Mutex::new(sensor_thresholds));
        let controller = ArduinoThread {
            arduino: None,
            upload_tried: false,
            port: port,
            connected: connected.clone(),
            event_sender: event_sender,
            command_receiver: command_receiver,
            sensor_thresholds: sensor_thresholds.clone()
        };

        ArduinoController {
            handle: Some(controller.run()),
            connected: connected,
            command_sender: Some(command_sender),
            event_receiver: Some(event_receiver),
            sensor_thresholds: sensor_thresholds
        }
    }

    pub fn connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    pub fn poll_events(&self) -> PollEvents {
        PollEvents(self.event_receiver.as_ref().unwrap())
    }

    pub fn set_sensor_thresholds(&self, id: u8, trigger: u8, release: u8) -> Result<()> {
        let command = Command::SetThresholds {
            id: id,
            trigger: trigger,
            release: release
        };
        self.command_sender.as_ref().unwrap().send(command)
            .chain_err(|| t!("Could not change the sensor thresholds"))
    }

    pub fn sensor_thresholds(&self, id: u8) -> Option<(u8, u8)> {
        self.sensor_thresholds.lock().unwrap().get(id as usize).cloned()
    }
}

impl Drop for ArduinoController {
    fn drop(&mut self) {
        if !thread::panicking() {
            mem::drop(self.command_sender.take());
            mem::drop(self.event_receiver.take());
            info!(t!("Waiting for Arduino thread to finish."));
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
    sensor_thresholds: Arc<Mutex<Vec<(u8, u8)>>>
}

impl ArduinoThread {
    fn run(mut self) -> JoinHandle<()> {
        thread::spawn(move || {
            while !self.stopping() {
                self.arduino = match self.restart() {
                    Ok(arduino) => Some(arduino),
                    Err(error) => {
                        log_full_error(&error);
                        info!(t!("Retrying in {} seconds."), 5);
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
                                info!(t!("Retrying in {} seconds."), 5);
                                thread::sleep(Duration::from_secs(5));
                                break;
                            } else {
                                info!(t!("Retrying in {} seconds."), 2);
                                thread::sleep(Duration::from_secs(2));
                            }
                        }
                    }
                }
            }

            info!(t!("The Arduino thread finished."));
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
                info!(t!("Trying to reupload the sketch once."));
                self.port = Arduino::upload(&self.port)?.into_owned();
                Arduino::open(&self.port, true)
            }
            error => Err(error)
        }).and_then(|mut arduino| {
            for (id, &(trigger, release)) in self.sensor_thresholds.lock().unwrap().iter().enumerate() {
                arduino.set_thresholds(id as u8, trigger, release)?;
            }
            Ok(arduino)
        })
    }

    fn process_commands(&mut self) -> Result<()> {
        let arduino = self.arduino.as_mut().unwrap();

        'outer: loop {
            match self.command_receiver.try_recv() {
                Ok(Command::SetThresholds { id, trigger, release }) => {
                    arduino.set_thresholds(id, trigger, release)?;
                }
                Err(TryRecvError::Empty) => {
                    let event = arduino.poll_event()?;
                    if let Some(event) = event {
                        match self.event_sender.try_send(event) {
                            Ok(()) => {}
                            error @ Err(TrySendError::Full(_)) => {
                                return error.chain_err(|| t!("Event buffer is full"));
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
