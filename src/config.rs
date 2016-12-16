use error::*;

use serde::Deserialize;

use toml::{Decoder as TomlDecoder, Parser, Value};

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct Configuration {
    pub speech: Speech,
    pub arduino: Arduino,
    pub decoder: Decoder
}

#[derive(Deserialize)]
pub struct Speech {
    pub engine: String,
    pub sapi: Option<SpeechEngine>,
    pub espeak: Option<SpeechEngine>
}

#[derive(Deserialize)]
pub struct SpeechEngine {
    pub voice: String,
    pub volume: u8
}

#[derive(Deserialize)]
pub struct Arduino {
    pub board: String,
    pub port: String,
    pub sensors: Vec<ArduinoSensor>
}

#[derive(Deserialize)]
pub struct ArduinoSensor {
    pub pin: String,
    pub label: String,
    pub limits: (u16, u16),
    pub thresholds: (u8, u8)
}

#[derive(Deserialize)]
pub struct Decoder {
    pub confirm: usize,
    pub scheme: HashMap<String, Vec<usize>>,
    pub prediction: DecoderPrediction
}

#[derive(Deserialize)]
pub struct DecoderPrediction {
    pub dictionary: Option<PathBuf>,
    pub suggestions: usize
}

impl Configuration {
    pub fn new(path: &Path) -> Result<Configuration> {
        File::open(path).and_then(|file| {
            let mut reader = BufReader::new(file);
            let mut toml = String::new();
            reader.read_to_string(&mut toml).map(move |_| toml)
        }).chain_err(|| text!("Could not read the configuration file")).and_then(|toml| {
            let mut parser = Parser::new(&toml);
            let result = parser.parse();
            for error in &parser.errors {
                error!(text!("Syntax error: {}."), error);
            }
            result.map_or_else(|| Err(parser.errors[0].clone()), Ok)
                  .chain_err(|| text!("The configuration file contains syntax errors"))
        }).and_then(|table| {
            Deserialize::deserialize(&mut TomlDecoder::new(Value::Table(table)))
                        .chain_err(|| text!("The configuration file is invalid"))
        })
    }
}