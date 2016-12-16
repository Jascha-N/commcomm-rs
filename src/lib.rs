//#![cfg_attr(debug_assertions, allow(dead_code))]
#![recursion_limit = "1024"]
#![feature(btree_range, collections_bound, proc_macro)]

extern crate chrono;
#[macro_use] extern crate conrod;
#[macro_use] extern crate error_chain;
extern crate flate2;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_json;
extern crate serial;
extern crate serial_enumerate;
extern crate tempdir;
extern crate toml;
extern crate wait_timeout;

#[cfg(windows)] extern crate kernel32;
#[cfg(windows)] extern crate ole32;
#[cfg(windows)] extern crate user32;
#[cfg(windows)] extern crate winapi;

pub mod arduino;
pub mod config;
pub mod decoder;
pub mod error;
pub mod intern;
pub mod platform;
pub mod speech;
pub mod ui;

mod glium {
    pub use conrod::glium::*;
}

mod glutin {
    pub use conrod::glium::glutin::*;
}
