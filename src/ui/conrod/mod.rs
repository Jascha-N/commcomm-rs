use self::apps::*;
use self::window::Window;
use arduino::thread::ArduinoController;
use config::Configuration;
use decoder::Decoder;
use error::*;

use std::thread;
use std::time::Duration;

mod apps;
mod window;

pub fn run(_: Configuration, _: ArduinoController, mut decoder: Decoder) -> Result<()> {
    let mut window = Window::new(&[&Speech::new_app, &Editor::new_app])?;
    while window.update(&mut decoder)? {
        thread::sleep(Duration::from_millis(1));
    }
    info!(t!("The window was closed."));

    Ok(())
}
