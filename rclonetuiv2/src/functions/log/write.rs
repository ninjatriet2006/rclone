use std::io::Write;
use crate::functions::log::initialize::LOG_FILE;
use crate::functions::log::scope::INDENT_LEVEL;

fn get_indent_prefix() -> String {
    let level = INDENT_LEVEL.with(|l| *l.borrow());
    if level == 0 {
        String::new()
    } else {
        let mut prefix = String::new();
        for _ in 0..(level - 1) {
            prefix.push_str("│   ");
        }
        prefix.push_str("├── ");
        prefix
    }
}

fn get_indent_end_prefix() -> String {
    let level = INDENT_LEVEL.with(|l| *l.borrow());
    if level <= 1 {
        String::new()
    } else {
        let mut prefix = String::new();
        for _ in 0..(level - 2) {
            prefix.push_str("│   ");
        }
        prefix.push_str("└── ");
        prefix
    }
}

fn get_timestamp() -> String {
    if let Ok(duration) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        let secs = duration.as_secs();
        // Định dạng thời gian tương đối thô sơ nhưng không cần crate phụ thuộc
        let hour = (secs / 3600) % 24;
        let minute = (secs / 60) % 60;
        let second = secs % 60;
        return format!("{:02}:{:02}:{:02}", hour, minute, second);
    }
    "00:00:00".to_string()
}

pub fn log_info_start(module: &str, action: &str, desc: &str) {
    if let Ok(mut guard) = LOG_FILE.lock() {
        if let Some(ref mut file) = *guard {
            let prefix = get_indent_prefix();
            let _ = writeln!(
                file,
                "[{}] [START] {}{}[{}::{}] {}",
                get_timestamp(),
                if prefix.is_empty() { "" } else { " " },
                prefix,
                module,
                action,
                desc
            );
            let _ = file.flush();
        }
    }
}

pub fn log_info_end(module: &str, action: &str) {
    if let Ok(mut guard) = LOG_FILE.lock() {
        if let Some(ref mut file) = *guard {
            let prefix = get_indent_end_prefix();
            let _ = writeln!(
                file,
                "[{}] [END]   {}{}[{}::{}] Hoàn thành",
                get_timestamp(),
                if prefix.is_empty() { "" } else { " " },
                prefix,
                module,
                action
            );
            let _ = file.flush();
        }
    }
}

pub fn log_info(msg: &str) {
    if let Ok(mut guard) = LOG_FILE.lock() {
        if let Some(ref mut file) = *guard {
            let prefix = get_indent_prefix();
            let _ = writeln!(
                file,
                "[{}] [INFO]  {}{}{}",
                get_timestamp(),
                if prefix.is_empty() { "" } else { " " },
                prefix,
                msg
            );
            let _ = file.flush();
        }
    }
}

pub fn log_rpc(rpc_command: &str, params_json: &str, response_status: Option<u16>, elapsed_ms: u128) {
    if let Ok(mut guard) = LOG_FILE.lock() {
        if let Some(ref mut file) = *guard {
            let prefix = get_indent_prefix();
            let _ = writeln!(
                file,
                "[{}] [RPC]   {}{}[RCLONE::RPC] Call \"{}\" with params: {} -> Status: {:?} ({}ms)",
                get_timestamp(),
                if prefix.is_empty() { "" } else { " " },
                prefix,
                rpc_command,
                params_json,
                response_status,
                elapsed_ms
            );
            let _ = file.flush();
        }
    }
}
