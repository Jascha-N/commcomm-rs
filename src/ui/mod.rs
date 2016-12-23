use arduino::Port;
use arduino::thread::ArduinoController;
use config::Configuration;
use decoder::Decoder;
use error::*;

use std::fmt::Write;
use std::path::Path;
use std::result;

#[cfg(feature = "conrod")]
mod conrod;

pub fn run() -> result::Result<(), ()> {
    fn run() -> Result<()> {
        let config = Configuration::new(Path::new("config.toml"))?;
        let arduino = ArduinoController::new(Port::new("COM3"), Vec::new());
        let decoder = Decoder::new(&config)?;

        #[cfg(feature = "conrod")]
        conrod::run(config, arduino, decoder)
    }

    info!(t!("Application started. Version: {}. Debug mode: {}."),
          env!("CARGO_PKG_VERSION"),
          if cfg!(debug_assertions) { t!("Yes") } else { t!("No") });

    let result = if let Err(error) = run() {
        let mut chain = error.iter();
        let mut message = String::new();
        let _ = write!(message, t!("An error has occurred: {}."), chain.next().unwrap());
        for cause in chain {
            let _ = write!(message, t!("\nCaused by:\n    {}."), cause);
        }

        println!("{}", message);

        #[cfg(windows)]
        ::platform::windows::error_message_box(&message);

        Err(())
    } else {
        Ok(())
    };

    info!(t!("The application is shutting down."));

    result
}
