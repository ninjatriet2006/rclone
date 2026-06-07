use std::fs::{File, OpenOptions};
use std::sync::Mutex;
use crate::functions::app_config::AppConfig;

lazy_static::lazy_static! {
    pub static ref LOG_FILE: Mutex<Option<File>> = Mutex::new(None);
}

pub fn initialize_logger() {
    let log_path = AppConfig::config_dir().join("user_activity.log");
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(file) = OpenOptions::new().create(true).write(true).append(true).open(log_path) {
        if let Ok(mut guard) = LOG_FILE.lock() {
            *guard = Some(file);
        }
    }
}
