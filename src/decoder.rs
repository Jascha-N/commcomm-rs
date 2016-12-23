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

#[derive(Clone, Debug)]
pub enum Input {
    Append(String),
    Delete,
    Question
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Dictionary(BTreeMap<String, Vec<DictEntry>>);

#[derive(Debug, Deserialize, Serialize)]
struct DictEntry(u64, String);

impl Dictionary {
    pub fn new() -> Dictionary {
        Dictionary::default()
    }

    fn from_config(config: &Configuration) -> Result<Option<Dictionary>> {
        config.decoder.prediction.dictionary.as_ref().map(|dict| {
            File::open(dict).chain_err(|| t!("Could not open the dictionary file")).map(|file| {
                BufReader::new(file).zlib_decode()
            }).and_then(|reader| {
                serde_json::from_reader(reader).chain_err(|| t!("Could not parse the dictionary"))
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
        File::create(path).chain_err(|| t!("Could not write the dictionary file")).and_then(|file| {
            let mut writer = BufWriter::new(file).zlib_encode(Compression::Best);
            serde_json::to_writer(&mut writer, self).chain_err(|| t!("Could not write the dictionary file"))
        })
    }
}

type InputScheme = BTreeMap<Vec<usize>, Input>;

#[derive(Debug)]
pub enum InputEvent {
    Illegal,
    DeleteLetter,
    DeleteWord,
    DeleteLine,
    Letters(String),
    Word(String),
    Line(String)
}

#[derive(Debug)]
pub struct Decoder {
    scheme: InputScheme,
    dictionary: Option<Dictionary>,
    confirm: usize,
    confirm_count: usize,
    //last_command: Option<Command>,
    //question: bool,
    input: Vec<usize>,
    word: Vec<String>,
    line: Vec<Vec<String>>
}

impl Decoder {
    pub fn new(config: &Configuration) -> Result<Decoder> {
        let mut scheme = InputScheme::new();
        let confirm = config.decoder.confirm;
        for (command, input) in &config.decoder.scheme {
            let command = match command.as_str() {
                "question" => Input::Question,
                "delete" => Input::Delete,
                append if append.starts_with("append:") => Input::Append(append[7..].to_string()),
                command => bail!(t!("Unknown command in 'decoder.scheme': {}"), command)
            };

            for &id in input {
                if id >= config.arduino.sensors.len() {
                    bail!(t!("Sensor index out of range: {}") , id);
                }
                if id == confirm {
                    bail!(t!("Sensor index in 'decoder.scheme' can not be equal to 'decoder.confirm'"));
                }
            }

            if scheme.insert(input.clone(), command).is_some() {
                let input = input.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
                bail!(t!("Input defined more than once: [{}]"), input);
            }
        }

        Ok(Decoder {
            scheme: scheme,
            dictionary: Dictionary::from_config(&config)?,
            confirm: confirm,
            confirm_count: 0,
            input: Vec::new(),
            word: Vec::new(),
            line: Vec::new()
        })
    }

    pub fn line(&self) -> String {
        self.line.iter()
                 .chain(iter::once(&self.word))
                 .map(|word| word.concat())
                 .collect::<Vec<_>>()
                 .join(" ")
    }

    pub fn process_input(&mut self, input: usize) -> Option<InputEvent> {
        if input == self.confirm {
            self.confirm_count += 1;
            match self.confirm_count {
                1 => {
                    let input = self.scheme.get(&self.input).cloned();
                    self.input.clear();
                    Some(input.map(|input| {
                        match input {
                            Input::Append(letters) => {
                                let letters = if self.line.is_empty() && self.word.is_empty() {
                                    let mut chars = letters.chars();
                                    if let Some(c) = chars.next() {
                                        c.to_uppercase().chain(chars).collect::<String>()
                                    } else {
                                        String::new()
                                    }
                                } else {
                                    letters
                                };
                                self.word.push(letters.clone());
                                InputEvent::Letters(letters)
                            }
                            Input::Delete => {
                                self.word.pop();
                                InputEvent::DeleteLetter
                            }
                            Input::Question => {
                                unimplemented!()
                                // self.question = !self.question;
                                // InputEvent::
                            }
                        }
                    }).unwrap_or_else(|| {
                        self.input.clear();
                        InputEvent::Illegal
                    }))
                }
                _ => None
            }
        } else {
            self.confirm_count = 0;
            self.input.push(input);
            None
        }
    }

    pub fn predict_input(&self) -> Vec<&Input> {
        self.input.split_last().map(|(last, rest)| {
            let lower = Bound::Included(&self.input);
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
