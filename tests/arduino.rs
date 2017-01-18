extern crate commcomm;

use commcomm::arduino::{Arduino, Port};

use std::time::Duration;
use std::thread;

#[test]
fn main() {
    let port = Port::new("COM3");
    let port = Arduino::upload(&port).unwrap();
    let mut arduino = Arduino::open(&port, false).unwrap();

    arduino.set_thresholds(0, 100, 150).unwrap();
    arduino.set_thresholds(1, 100, 150).unwrap();

    loop {
        if let Some(event) = arduino.poll_event().unwrap() {
            println!("Event: {:?}", event);
        }
        println!("Mapped: {:?}", arduino.read_values(false).unwrap());
        println!("Raw: {:?}", arduino.read_values(true).unwrap());
        thread::sleep(Duration::from_millis(100));
    }
}
