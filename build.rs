extern crate regex;

use regex::{Captures, Regex};

use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{self, SystemTime};

fn load_prefs_from_str(pref_str: &str) -> HashMap<&str, String> {
    let mut prefs = HashMap::new();
    for line in pref_str.lines() {
        if line.starts_with("===") || !line.contains("=") {
            continue;
        }
        let mut splits = line.splitn(2, '=');
        let key = splits.next().unwrap();
        let value = splits.next().unwrap();
        if key.starts_with("tools.avrdude.") {
            prefs.insert(&key[14..], value.to_string());
        } else if !key.starts_with("tools.") {
            prefs.insert(key, value.to_string());
        }
    }
    prefs
}

fn expand_prefs<'a>(prefs: &HashMap<&'a str, String>) -> HashMap<&'a str, String> {
    let mut prefs = prefs.clone();
    let substitution_regex = Regex::new(r#"\{(.+?)\}"#).unwrap();
    for _ in 0 .. 10 {
        let mut new_prefs = HashMap::new();
        for (&key, value) in &prefs {
            new_prefs.insert(key, substitution_regex.replace_all(value, |captures: &Captures| {
                prefs.get(captures.get(1).unwrap().as_str())
                     .map_or(captures.get(0).unwrap().as_str(), AsRef::as_ref)
                     .to_string()
            }).to_string());
        }
        if prefs == new_prefs {
            break;
        }
        prefs = new_prefs;
    }
    prefs
}

fn write_board_file(path: &Path, prefs: &HashMap<&str, String>, name: &str, timestamp: u64) {
    let tool = prefs.get("upload.tool").unwrap();
    if *tool != "avrdude" {
        panic!("Only AVR boards are supported.");
    }
    let protocol: String = prefs.get("upload.protocol").map(Clone::clone).unwrap_or_default();
    let speed: usize = prefs.get("upload.speed").and_then(|s| s.parse().ok()).unwrap_or_default();
    let use_1200bps_touch: bool = prefs.get("upload.use_1200bps_touch")
                                       .and_then(|s| s.parse().ok()).unwrap_or_default();
    let wait_for_upload_port: bool = prefs.get("upload.wait_for_upload_port")
                                          .and_then(|s| s.parse().ok()).unwrap_or_default();
    let mcu: String = prefs.get("build.mcu").map(Clone::clone).unwrap_or_default();
    let config_path: PathBuf = prefs.get("config.path").map_or(PathBuf::new(), PathBuf::from);
    let build_path: PathBuf = prefs.get("build.path").map_or(PathBuf::new(), PathBuf::from);
    let project_name: String = prefs.get("build.project_name").map(Clone::clone).unwrap_or_default();

    let mut writer = BufWriter::new(File::create(path).unwrap());
    writeln!(writer, r##"pub const PROTOCOL: &'static str = r#"{}"#;"##, protocol).unwrap();
    writeln!(writer, "pub const SPEED: usize = {};", speed).unwrap();
    writeln!(writer, "pub const USE_1200BPS_TOUCH: bool = {};", use_1200bps_touch).unwrap();
    writeln!(writer, "pub const WAIT_FOR_UPLOAD_PORT: bool = {};", wait_for_upload_port).unwrap();
    writeln!(writer, r##"pub const MCU: &'static str = r#"{}"#;"##, mcu).unwrap();
    writeln!(writer, r##"pub const PROGRAM: &'static [u8] = include_bytes!(r#"{}.hex"#);"##,
             build_path.join(project_name).display()).unwrap();
    writeln!(writer, r##"pub const AVRDUDE_CONFIG: &'static [u8] = include_bytes!(r#"{}"#);"##, config_path.display()).unwrap();
    writeln!(writer, r#"lazy_static! {{ pub static ref DEVICE_INFO: ::arduino::DeviceInfo = ::arduino::DeviceInfo::new("{}", "{}", {}); }}"#,
             name, env!("CARGO_PKG_VERSION"), timestamp).unwrap();
}

fn builder_command(compile: bool, src_path: &Path, out_path: &Path, prefs: Option<HashMap<&str, String>>) -> Command {
    let mut command = Command::new("arduino-builder");
    command.arg("-libraries").arg("libraries")
           .arg("-build-path").arg(out_path)
           .arg("-warnings").arg("all")
           .arg("-verbose")
           .arg(if compile { "-compile" } else { "-dump-prefs" });

    if Path::new("build.options.json").exists() {
        command.arg("-build-options-file").arg("build.options.json");
    } else {
        if let Some(home) = env::var_os("ARDUINO_HOME").map(PathBuf::from) {
            command.arg("-built-in-libraries").arg(home.join("libraries"))
                   .arg("-hardware").arg(home.join("hardware"))
                   .arg("-tools").arg(home.join("hardware/tools/avr"))
                   .arg("-tools").arg(home.join("tools-builder"));
        }

        if let Some(board) = env::var_os("ARDUINO_BOARD") {
            command.arg("-fqbn").arg(board);
        }

        if let Some(hardware) = env::var_os("ARDUINO_HARDWARE") {
            for path in env::split_paths(&hardware) {
                command.arg("-hardware").arg(path);
            }
        }

        if let Some(tools) = env::var_os("ARDUINO_TOOLS") {
            for path in env::split_paths(&tools) {
                command.arg("-tools").arg(path);
            }
        }

        if let Some(libraries) = env::var_os("ARDUINO_LIBRARIES") {
            for path in env::split_paths(&libraries) {
                command.arg("-libraries").arg(path);
            }
        }
    }

    if let Some(prefs) = prefs {
        for (key, value) in prefs {
            command.arg("-prefs").arg(format!("{}={}", key, value));
        }
    }

    command.arg(src_path);

    command
}

pub fn main() {
    let timestamp = SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_secs();
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let src = Path::new("src/arduino/program.ino");
    let arduino_out = out.join("arduino");
    fs::create_dir_all(&arduino_out).unwrap();

    // Dump preferences
    let output = builder_command(false, src, &arduino_out, None).output().unwrap();
    let _ = io::stdout().write_all(&output.stdout);
    let _ = io::stderr().write_all(&output.stderr);
    if !output.status.success() {
        panic!(r#""arduino-builder -dump-prefs" exited with error code {}."#,
               output.status.code().map_or("<none>".to_string(), |code| code.to_string()))
    }

    let prefs_str = String::from_utf8_lossy(&output.stdout);
    let prefs = load_prefs_from_str(&prefs_str);
    let expanded_prefs = expand_prefs(&prefs);
    let name = expanded_prefs.get("name").map(Clone::clone).unwrap_or_default();
    write_board_file(&out.join("board.rs"), &expanded_prefs, &name, timestamp);



    // Compile sketch
    let mut extra_prefs = HashMap::new();
    let extra_flags = format!(r#"{}'-DDEVICEINFO_NAME="{}"' '-DDEVICEINFO_VERSION="{}"' -DDEVICEINFO_TIMESTAMP={:#x}"#,
                              prefs.get("build.extra_flags").map_or_else(String::new, |flags| format!("{} ", flags)),
                              name,
                              env!("CARGO_PKG_VERSION"),
                              timestamp);
    extra_prefs.insert("build.extra_flags", extra_flags);

    let output = builder_command(true, src, &arduino_out, Some(extra_prefs)).output().unwrap();
    let _ = io::stdout().write_all(&output.stdout);
    let _ = io::stderr().write_all(&output.stderr);
    if !output.status.success() {
        panic!(r#""arduino-builder -compile" exited with error code {}."#,
               output.status.code().map_or("<none>".to_string(), |code| code.to_string()))
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stderr.lines() {
        if line.contains(": warning: ") {
            println!("cargo:warning={}", line);
        }
    }

    println!("cargo:rerun-if-changed=src/arduino/program.ino");
    println!("cargo:rerun-if-changed=build.options.json");
}
