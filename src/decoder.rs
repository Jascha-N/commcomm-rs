use config::Configuration;
use error::*;

//use fst::{IntoStreamer, Map};

use flate2::{Compression, FlateReadExt, FlateWriteExt};
use serde_json;

use std::collections::{Bound, BTreeMap};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::iter;
use std::path::Path;

pub enum Event {

}

pub struct Translator {
    sentence: Vec<String>,
    word: Vec<u8>
    //decoder:
}

#[derive(Debug)]
enum Input {
    Append(String),
    Delete,
    Question
}

#[derive(Default, Deserialize, Serialize)]
pub struct Dictionary(BTreeMap<String, Vec<DictEntry>>);

#[derive(Deserialize, Serialize)]
struct DictEntry(u64, String);

impl Dictionary {
    pub fn new() -> Dictionary {
        Dictionary::default()
    }

    fn from_config(config: &Configuration) -> Result<Option<Dictionary>> {
        config.decoder.prediction.dictionary.as_ref().map(|dict| {
            File::open(dict).chain_err(|| "Kon het woordenboek niet openen").map(|file| {
                BufReader::new(file).zlib_decode()
            }).and_then(|reader| {
                serde_json::from_reader(reader).chain_err(|| "Kon het woordenboek niet inlezen")
            }).map(Some)
        }).unwrap_or_else(|| Ok(None))
    }

    pub fn insert<S: Into<String>>(&mut self, word: S, frequency: u64) {
        fn word_to_key(word: &str) -> String {
            word.to_lowercase()
                .replace(&['\'', '+', '-', '.', '/', '_', ' '][..], "")
                .replace(&['à', 'á', 'â', 'ä', 'å'][..], "a")
                .replace('ç', "c")
                .replace(&['è', 'é', 'ê', 'ë'][..], "e")
                .replace(&['í', 'î', 'ï'][..], "i")
                .replace('ñ', "n")
                .replace(&['ó', 'ô', 'ö'][..], "o")
                .replace(&['ú', 'û', 'ü'][..], "u")
                .replace('₂', "2")
        }

        let word = word.into();
        let key = word_to_key(&word);
        self.0.entry(key).or_insert_with(Vec::new).push(DictEntry(frequency, word));
    }

    pub fn write_to_file(&self, path: &Path) -> Result<()> {
        File::create(path).chain_err(|| "Kon het woordenboek niet wegschrijven").and_then(|file| {
            let mut writer = BufWriter::new(file).zlib_encode(Compression::Best);
            serde_json::to_writer(&mut writer, self).chain_err(|| "Kon het woordenboek niet wegschrijven")
        })
    }
}

type InputScheme = BTreeMap<Vec<usize>, Input>;

struct Decoder {
    scheme: InputScheme,
    dictionary: Option<Dictionary>
}

impl Decoder {
    pub fn new(config: &Configuration) -> Result<Decoder> {
        let mut scheme = InputScheme::new();
        for (command, input) in &config.decoder.scheme {
            let command = match command.as_str() {
                "question" => Input::Question,
                "delete" => Input::Delete,
                append if append.starts_with("append:") => Input::Append(append[7..].to_string()),
                command => bail!("Onbekend commando in 'decoder.scheme': {}", command)
            };

            for &id in input {
                if id >= config.arduino.sensors.len() {
                    bail!("Sensorindex buiten bereik: {}", id);
                }
                if id == config.decoder.confirm {
                    bail!("Sensorindex in 'decoder.scheme' mag niet gelijk zijn aan 'decoder.confirm'");
                }
            }

            if scheme.insert(input.clone(), command).is_some() {
                let input = input.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
                bail!("Invoer meerdere keren gedefinïeerd: [{}]", input);
            }
        }

        Ok(Decoder {
            scheme: scheme,
            dictionary: Dictionary::from_config(&config)?
        })
    }

    fn lookup(&self, input: &[usize]) -> Vec<&Input> {
        input.split_last().map(|(last, rest)| {
            let lower = Bound::Included(input);
            last.checked_add(1).map(|last| {
                let upper = rest.iter().cloned().chain(iter::once(last)).collect::<Vec<_>>();
                self.scheme.range(lower, Bound::Excluded(&upper)).map(|(_, v)| v).collect()
            }).unwrap_or_else(|| {
                self.scheme.range(lower, Bound::Unbounded::<&[_]>).map(|(_, v)| v).collect()
            })
        }).unwrap_or_else(|| {
            self.scheme.values().collect()
        })
    }
}