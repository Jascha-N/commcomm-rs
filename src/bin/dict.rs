#[macro_use] extern crate commcomm;
#[macro_use] extern crate clap;

use commcomm::decoder::Dictionary;

use clap::{App, Arg};

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{PathBuf, Path};

fn main() {
    let matches = App::new(text!("commcomm-rs dictionary tool"))
                      .version(crate_version!())
                      .author(crate_authors!())
                      .about(text!("Builds a dictionary file from word-frequency file."))
                      .arg(Arg::with_name("OUTPUT")
                               .short("o")
                               .long("output")
                               .value_name("FILE")
                               .help(text!("Sets a custom output file"))
                               .takes_value(true))
                      .arg(Arg::with_name("INPUT")
                               .help(text!("The input file to use"))
                               .required(true))
                      .get_matches();

    let source = Path::new(matches.value_of("INPUT").unwrap());

    let reader = BufReader::new(File::open(source).unwrap());
    let mut dictionary = Dictionary::new();
    for line in reader.lines() {
        let line = line.unwrap();
        let parts = line.splitn(2, '=').collect::<Vec<_>>();
        dictionary.insert(parts[0], parts[1].parse().unwrap());
    }

    let dest = matches.value_of("OUTPUT").map_or_else(|| source.with_extension("dict"), PathBuf::from);
    dictionary.write_to_file(&dest).unwrap();
}
