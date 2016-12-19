#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![cfg_attr(not(debug_assertions), feature(windows_subsystem))]

extern crate commcomm;
extern crate env_logger;
extern crate log;

use commcomm::ui;

use env_logger::LogBuilder;
use log::LogLevelFilter;

use std::process;



fn main() {
    let mut log_builder = LogBuilder::new();
    log_builder.format(|record| format!("[{}][{}] {}", record.level(), record.target(), record.args()));
    log_builder.filter(None, LogLevelFilter::Info);
    log_builder.init().unwrap();

    if ui::run().is_err() {
        process::exit(1);
    }
}
