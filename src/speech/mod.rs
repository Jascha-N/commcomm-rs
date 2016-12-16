//use config::Configuration;

#[cfg(windows)]
pub use platform::windows::sapi::{SpeechEngine, SpeechEngineImpl, Voice};

// #[cfg(windows)]
// mod sapi;

// pub trait SpeechEngine {
//     type Voice: Voice;

//     fn voice(&self) -> Self::Voice;
// }