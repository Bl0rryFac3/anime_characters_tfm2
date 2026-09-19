use std::fs::OpenOptions;
use std::io::Write;

fn log_path() -> std::path::PathBuf {
    std::env::temp_dir().join("my_mod.log")
}

pub fn log(msg: &str) {
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path()) else {
        return;
    };
    let _ = writeln!(file, "{msg}");
}

#[cfg(debug_assertions)]
#[macro_export]
macro_rules! diagnostic_log {
    ($message:expr) => {
        crate::logger::log($message);
    };
}

#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! diagnostic_log {
    ($message:expr) => {};
}
