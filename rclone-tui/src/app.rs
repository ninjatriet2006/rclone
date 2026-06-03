use crate::app_config::{AppConfig, ExportResult};
use crate::rclone;
use crate::ui;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

lazy_static::lazy_static! {
    static ref RUNNING_SIZE_CHECKS: std::sync::Mutex<std::collections::HashSet<String>> = std::sync::Mutex::new(std::collections::HashSet::new());
    static ref JOB_DESCRIPTIONS: std::sync::Mutex<std::collections::HashMap<i64, String>> = std::sync::Mutex::new(std::collections::HashMap::new());
}

pub fn register_job_description(job_id: i64, description: String) {
    if let Ok(mut map) = JOB_DESCRIPTIONS.lock() {
        map.insert(job_id, description);
    }
}

pub fn get_job_description(job_id: i64) -> Option<String> {
    if let Ok(map) = JOB_DESCRIPTIONS.lock() {
        map.get(&job_id).cloned()
    } else {
        None
    }
}

fn handle_input_key(
    key: &crossterm::event::KeyEvent,
    input_buffer: &mut String,
    edit_cursor_idx: &mut usize,
) -> bool {
    use crossterm::event::KeyCode;
    
    let chars: Vec<char> = input_buffer.chars().collect();
    let char_count = chars.len();
    
    if *edit_cursor_idx > char_count {
        *edit_cursor_idx = char_count;
    }

    match key.code {
        KeyCode::Left => {
            if *edit_cursor_idx > 0 {
                *edit_cursor_idx -= 1;
            }
            true
        }
        KeyCode::Right => {
            if *edit_cursor_idx < char_count {
                *edit_cursor_idx += 1;
            }
            true
        }
        KeyCode::Home => {
            *edit_cursor_idx = 0;
            true
        }
        KeyCode::End => {
            *edit_cursor_idx = char_count;
            true
        }
        KeyCode::Backspace => {
            if *edit_cursor_idx > 0 {
                let mut new_chars = Vec::with_capacity(char_count.saturating_sub(1));
                new_chars.extend_from_slice(&chars[0..*edit_cursor_idx - 1]);
                new_chars.extend_from_slice(&chars[*edit_cursor_idx..]);
                *input_buffer = new_chars.into_iter().collect();
                *edit_cursor_idx -= 1;
            }
            true
        }
        KeyCode::Delete => {
            if *edit_cursor_idx < char_count {
                let mut new_chars = Vec::with_capacity(char_count.saturating_sub(1));
                new_chars.extend_from_slice(&chars[0..*edit_cursor_idx]);
                new_chars.extend_from_slice(&chars[*edit_cursor_idx + 1..]);
                *input_buffer = new_chars.into_iter().collect();
            }
            true
        }
        KeyCode::Char(c) => {
            let has_modifiers = key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) 
                || key.modifiers.contains(crossterm::event::KeyModifiers::ALT);
            if !has_modifiers {
                let mut new_chars = Vec::with_capacity(char_count + 1);
                new_chars.extend_from_slice(&chars[0..*edit_cursor_idx]);
                new_chars.push(c);
                new_chars.extend_from_slice(&chars[*edit_cursor_idx..]);
                *input_buffer = new_chars.into_iter().collect();
                *edit_cursor_idx += 1;
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    MainMenu,
    ConnectionManager,
    FileExplorer,
    JobMonitor,
    ConfigProfileManager,
    ServicesAndMounts,
    LanguageSelect,
}

#[allow(dead_code)]
pub enum AppEvent {
    Input(KeyEvent),
    Tick,
    ExplorerListResult {
        pane: ui::explorer::ActivePane,
        result: Result<Vec<ui::explorer::FileItem>, String>,
    },
    WizardGuiListResult {
        result: Result<Vec<ui::explorer::FileItem>, String>,
    },
    WizardGuiRefresh,
    CopyProgress {
        src: String,
        dest: String,
        pct: f64,
        job_id: Option<i64>,
    },
    MoveProgress {
        src: String,
        dest: String,
        pct: f64,
        job_id: Option<i64>,
    },
    JobStatsUpdate {
        speed: f64,
        transferred: u64,
        total: u64,
        active: Vec<ui::monitor::TransferJob>,
    },
    OAuthFinished {
        result: Result<(), String>,
    },
    OAuthUrlReceived {
        url: String,
    },
    ActiveServicesLoaded(Vec<ui::services::ActiveService>),
    RemoteStatusUpdate {
        remote: String,
        status: String,
    },
    ExplorerOperationFinished {
        pane: ui::explorer::ActivePane,
        op_name: String,
        result: Result<(), String>,
    },
    FeaturesChecked {
        action_type: String,
        src: String,
        dest: String,
        src_features: Option<serde_json::Value>,
        dst_features: Option<serde_json::Value>,
        is_dir: bool,
    },
    FileViewLoaded {
        file_name: String,
        result: Result<Vec<String>, String>,
    },
    TuiSelectorListResult {
        result: Result<Vec<ui::explorer::FileItem>, String>,
    },
    CryptdecodeFinished {
        result: Result<String, String>,
    },
    CryptdecodeResult {
        result: Result<String, String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeleteTarget {
    Connection(String),
    FileExplorer(String),
    FileExplorerMultiple(Vec<String>),
    Service(usize),
    SystemdService(usize),
}

pub struct App {
    pub screen: Screen,
    pub config: AppConfig,
    pub should_exit: bool,
    pub delete_confirm: Option<DeleteTarget>,

    // States
    pub menu_state: ui::menu::MenuState,
    pub connection_state: ui::connection::ConnectionState,
    pub explorer_state: ui::explorer::ExplorerState,
    pub monitor_state: ui::monitor::MonitorState,
    pub profile_state: ui::profile::ProfileState,
    pub services_state: ui::services::ServicesState,

    // Language States
    pub available_languages: Vec<String>,
    pub selected_lang_idx: usize,

    // Status checker trigger
    pub status_trigger_tx: Option<tokio::sync::mpsc::UnboundedSender<()>>,
    pub remote_dependencies: std::collections::HashMap<String, String>,
    pub features_cache: std::collections::HashMap<String, serde_json::Value>,
    pub last_services_scan: std::time::Instant,
    pub last_stats_scan: std::time::Instant,
    pub stats_scan_in_progress: bool,
}

impl App {
    pub fn new() -> Self {
        // Khởi tạo và nạp ngôn ngữ
        crate::lang::init_languages();
        let config = AppConfig::load();
        crate::lang::load_translation(&config.active_language);
        let available_languages = crate::lang::get_available_languages();
        let selected_lang_idx = available_languages
            .iter()
            .position(|l| l == &config.active_language)
            .unwrap_or(0);

        let mut features_cache = std::collections::HashMap::new();
        let cache_path = crate::app_config::AppConfig::config_dir().join("features_cache.json");
        if let Ok(content) = std::fs::read_to_string(&cache_path) {
            if let Ok(parsed) = serde_json::from_str::<std::collections::HashMap<String, serde_json::Value>>(&content) {
                features_cache = parsed;
            }
        }

        App {
            screen: Screen::MainMenu,
            config,
            should_exit: false,
            delete_confirm: None,
            menu_state: ui::menu::MenuState::new(),
            connection_state: ui::connection::ConnectionState::new(),
            explorer_state: ui::explorer::ExplorerState::new(),
            monitor_state: ui::monitor::MonitorState::new(),
            profile_state: ui::profile::ProfileState::new(),
            services_state: ui::services::ServicesState::new(),
            available_languages,
            selected_lang_idx,
            status_trigger_tx: None,
            remote_dependencies: std::collections::HashMap::new(),
            features_cache,
            last_services_scan: std::time::Instant::now(),
            last_stats_scan: std::time::Instant::now(),
            stats_scan_in_progress: false,
        }
    }

fn get_underlying_remote(config_path: &str, remote: &str) -> Option<String> {
    let target_section = match remote.find(':') {
        Some(pos) => &remote[..pos],
        None => remote,
    }.trim();
    
    if let Ok(content) = std::fs::read_to_string(config_path) {
        let mut in_section = false;
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                let section_name = &line[1..line.len()-1];
                in_section = section_name == target_section;
            } else if in_section {
                if line.starts_with("remote") {
                    let parts: Vec<&str> = line.split('=').collect();
                    if parts.len() >= 2 {
                        return Some(parts[1].trim().to_string());
                    }
                }
            }
        }
    }
    None
}

    fn detect_systemd_service(_pid: u32) -> Option<(String, bool)> {
        #[cfg(unix)]
        {
            if let Ok(content) = std::fs::read_to_string(format!("/proc/{}/cgroup", _pid)) {
                for line in content.lines() {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() >= 3 {
                        let cgroup_path = parts[2];
                        if cgroup_path.contains(".service") {
                            let segments: Vec<&str> = cgroup_path.split('/').collect();
                            let mut service_unit = None;
                            for seg in segments.iter().rev() {
                                if seg.ends_with(".service") && !seg.starts_with("user@") && !seg.starts_with("user-") && *seg != "init.service" {
                                    service_unit = Some(seg.to_string());
                                    break;
                                }
                            }
                            if let Some(unit) = service_unit {
                                let is_user = cgroup_path.contains("/user.slice/") || cgroup_path.contains("user@");
                                return Some((unit, is_user));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn parse_rclone_args(&self, pid: u32, args: &[String]) -> Option<ui::services::ActiveService> {
        if args.is_empty() {
            return None;
        }
        let exe = &args[0];
        let is_rclone = exe == "rclone" 
            || exe.ends_with("/rclone") 
            || exe == "rclone.exe" 
            || exe.to_lowercase().ends_with("\\rclone.exe");

        if !is_rclone {
            return None;
        }

        let systemd_info = Self::detect_systemd_service(pid);

        if args.contains(&"mount".to_string()) || args.contains(&"nfsmount".to_string()) {
            let is_nfs = args.contains(&"nfsmount".to_string());
            let cmd_name = if is_nfs { "nfsmount" } else { "mount" };
            let mut non_flags = Vec::new();
            for arg in args.iter().skip(1) {
                if !arg.starts_with('-') && arg != cmd_name {
                    non_flags.push(arg.clone());
                }
            }
            let (remote, local_mnt) = if non_flags.len() >= 2 {
                (non_flags[0].clone(), non_flags[1].clone())
            } else if non_flags.len() == 1 {
                (String::new(), non_flags[0].clone())
            } else {
                (String::new(), String::new())
            };
            let mut config_path = String::new();
            for arg in args {
                if arg.starts_with("--config=") {
                    config_path = arg.trim_start_matches("--config=").to_string();
                }
            }
            if config_path.is_empty() {
                if let Some(pos) = args.iter().position(|r| r == "--config") {
                    if pos + 1 < args.len() {
                        config_path = args[pos + 1].clone();
                    }
                }
            }
            let mut profile_name = "default".to_string();
            if !config_path.is_empty() {
                for (name, path) in &self.config.profiles {
                    if path == &config_path {
                        profile_name = name.clone();
                        break;
                    }
                }
            } else {
                profile_name = self.config.active_profile.clone();
            }
            let profile_prefix = if profile_name == "default" {
                String::new()
            } else {
                format!("{}: -> ", profile_name)
            };
            let details = if remote.is_empty() {
                format!("{}{}", profile_prefix, local_mnt)
            } else {
                let display_remote = if let Some(und) = Self::get_underlying_remote(&config_path, &remote) {
                    let base = und.split(':').next().unwrap_or(&und);
                    format!("{}: -> {}", base, remote)
                } else {
                    remote.clone()
                };
                format!("{}{}{} -> {}", profile_prefix, if display_remote.ends_with(':') || display_remote.contains("->") { "" } else { "" }, display_remote, local_mnt)
            };
            let details = if is_nfs { format!("NfsMount: {}", details) } else { details };

            let (service_type_str, final_details) = if let Some((unit_name, is_user)) = &systemd_info {
                let lvl = if *is_user { "Cá nhân" } else { "Hệ thống" };
                (format!("Service ({})", lvl), format!("Dịch vụ: {} | {}", unit_name, details))
            } else {
                ("Mount (Tạm thời)".to_string(), details)
            };

            Some(ui::services::ActiveService {
                service_type_str,
                remote,
                path: local_mnt,
                pid,
                details: final_details,
            })
        } else if args.contains(&"serve".to_string()) {
            let mut proto = "http".to_string();
            let mut addr = ":8080".to_string();
            let mut remote_path = String::new();
            
            if let Some(pos) = args.iter().position(|r| r == "serve") {
                if pos + 1 < args.len() {
                    proto = args[pos + 1].clone();
                }
            }

            for i in 0..args.len() {
                if args[i].starts_with("--addr=") {
                    addr = args[i]["--addr=".len()..].to_string();
                } else if args[i] == "--addr" && i + 1 < args.len() {
                    addr = args[i+1].clone();
                } else if !args[i].starts_with('-') && args[i] != "serve" && args[i] != proto && i > 0 && args[i-1] != "--addr" && args[i-1] != "--user" && args[i-1] != "--pass" {
                    remote_path = args[i].clone();
                }
            }
            let mut config_path = String::new();
            for arg in args {
                if arg.starts_with("--config=") {
                    config_path = arg.trim_start_matches("--config=").to_string();
                }
            }
            if config_path.is_empty() {
                if let Some(pos) = args.iter().position(|r| r == "--config") {
                    if pos + 1 < args.len() {
                        config_path = args[pos + 1].clone();
                    }
                }
            }
            let mut profile_name = "default".to_string();
            if !config_path.is_empty() {
                for (name, path) in &self.config.profiles {
                    if path == &config_path {
                        profile_name = name.clone();
                        break;
                    }
                }
            } else {
                profile_name = self.config.active_profile.clone();
            }
            let profile_prefix = if profile_name == "default" {
                String::new()
            } else {
                format!("{}: -> ", profile_name)
            };
            let details = if remote_path.is_empty() {
                format!("{}{}{}", profile_prefix, proto, addr)
            } else {
                let display_remote = if let Some(und) = Self::get_underlying_remote(&config_path, &remote_path) {
                    let base = und.split(':').next().unwrap_or(&und);
                    format!("{}: -> {}", base, remote_path)
                } else {
                    remote_path.clone()
                };
                format!("{}{}{} -> {}://{}", profile_prefix, display_remote, if display_remote.is_empty() { "" } else { " -> " }, proto, addr)
            };

            let (service_type_str, final_details) = if let Some((unit_name, is_user)) = &systemd_info {
                let lvl = if *is_user { "Cá nhân" } else { "Hệ thống" };
                (format!("Service ({})", lvl), format!("Dịch vụ: {} | {}", unit_name, details))
            } else {
                ("Serve (Tạm thời)".to_string(), details)
            };

            Some(ui::services::ActiveService {
                service_type_str,
                remote: remote_path,
                path: addr,
                pid,
                details: final_details,
            })
        } else if args.contains(&"rcd".to_string()) {
            let mut rc_addr = "localhost:5572".to_string();
            for i in 0..args.len() {
                if args[i].starts_with("--rc-addr=") {
                    rc_addr = args[i]["--rc-addr=".len()..].to_string();
                } else if args[i] == "--rc-addr" && i + 1 < args.len() {
                    rc_addr = args[i+1].clone();
                }
            }
            let mut config_path = String::new();
            for arg in args {
                if arg.starts_with("--config=") {
                    config_path = arg.trim_start_matches("--config=").to_string();
                }
            }
            if config_path.is_empty() {
                if let Some(pos) = args.iter().position(|r| r == "--config") {
                    if pos + 1 < args.len() {
                        config_path = args[pos + 1].clone();
                    }
                }
            }
            let mut profile_name = "default".to_string();
            if !config_path.is_empty() {
                for (name, path) in &self.config.profiles {
                    if path == &config_path {
                        profile_name = name.clone();
                        break;
                    }
                }
            } else {
                profile_name = self.config.active_profile.clone();
            }
            let profile_prefix = if profile_name == "default" {
                String::new()
            } else {
                format!("{}: -> ", profile_name)
            };
            let details = format!("{}Cổng Web: {}", profile_prefix, rc_addr);

            let (service_type_str, final_details) = if let Some((unit_name, is_user)) = &systemd_info {
                let lvl = if *is_user { "Cá nhân" } else { "Hệ thống" };
                (format!("Service ({})", lvl), format!("Dịch vụ: {} | {}", unit_name, details))
            } else {
                ("WebGui (Tạm thời)".to_string(), details)
            };

            Some(ui::services::ActiveService {
                service_type_str,
                remote: String::new(),
                path: rc_addr,
                pid,
                details: final_details,
            })
        } else {
            if let Some((unit_name, is_user)) = systemd_info {
                let lvl = if is_user { "Cá nhân" } else { "Hệ thống" };
                let service_type_str = format!("Service ({})", lvl);
                let details = format!("Dịch vụ: {} | Lệnh: {}", unit_name, args.join(" "));
                Some(ui::services::ActiveService {
                    service_type_str,
                    remote: String::new(),
                    path: String::new(),
                    pid,
                    details,
                })
            } else {
                None
            }
        }
    }

    /// Quét các tiến trình rclone đang chạy thực tế trên hệ thống (Linux /proc, Windows wmic)
    pub fn scan_running_services(&mut self) {
        let mut scanned_services = Vec::new();

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            if let Ok(entries) = fs::read_dir("/proc") {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        if path.is_dir() {
                            if let Some(pid_str) = path.file_name().and_then(|s| s.to_str()) {
                                if let Ok(pid) = pid_str.parse::<u32>() {
                                    let cmdline_path = path.join("cmdline");
                                    if let Ok(mut file) = fs::File::open(cmdline_path) {
                                        use std::io::Read;
                                        let mut buffer = Vec::new();
                                        if file.read_to_end(&mut buffer).is_ok() {
                                            let args: Vec<String> = buffer
                                                .split(|&b| b == 0)
                                                .filter_map(|slice| {
                                                    if slice.is_empty() {
                                                        None
                                                    } else {
                                                        Some(String::from_utf8_lossy(slice).trim().to_string())
                                                    }
                                                })
                                                .collect();
                                            if let Some(service) = self.parse_rclone_args(pid, &args) {
                                                scanned_services.push(service);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = std::process::Command::new("ps")
                .args(["-ax", "-o", "pid,command"])
                .output()
            {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    for line in stdout.lines().skip(1) { // Skip header line
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        if let Some(pos) = line.find(' ') {
                            let pid_str = &line[..pos];
                            let cmdline = &line[pos..].trim();
                            if let Ok(pid) = pid_str.parse::<u32>() {
                                let args = parse_cmdline(cmdline);
                                if let Some(service) = self.parse_rclone_args(pid, &args) {
                                    scanned_services.push(service);
                                }
                            }
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            let mut wmic_success = false;
            if let Ok(output) = std::process::Command::new("wmic")
                .args(["process", "where", "name='rclone.exe'", "get", "CommandLine,ProcessId", "/FORMAT:list"])
                .output()
            {
                if output.status.success() {
                    wmic_success = true;
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let mut current_cmdline = String::new();
                    let mut current_pid: Option<u32> = None;

                    for line in stdout.lines() {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        if line.starts_with("CommandLine=") {
                            current_cmdline = line["CommandLine=".len()..].trim().to_string();
                        } else if line.starts_with("ProcessId=") {
                            if let Ok(pid) = line["ProcessId=".len()..].trim().parse::<u32>() {
                                current_pid = Some(pid);
                            }
                        }

                        if !current_cmdline.is_empty() && current_pid.is_some() {
                            let pid = current_pid.unwrap();
                            let args = parse_cmdline(&current_cmdline);
                            if let Some(service) = self.parse_rclone_args(pid, &args) {
                                scanned_services.push(service);
                            }
                            current_cmdline.clear();
                            current_pid = None;
                        }
                    }
                }
            }

            // Fallback sang PowerShell nếu WMIC thất bại hoặc không có sẵn
            if !wmic_success {
                if let Ok(output) = std::process::Command::new("powershell")
                    .args([
                        "-NoProfile",
                        "-Command",
                        "Get-CimInstance Win32_Process -Filter \"name = 'rclone.exe'\" | Select-Object CommandLine, ProcessId | Format-List"
                    ])
                    .output()
                {
                    if output.status.success() {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let mut current_cmdline = String::new();
                        let mut current_pid: Option<u32> = None;

                        for line in stdout.lines() {
                            let line = line.trim();
                            if line.is_empty() {
                                continue;
                            }
                            if line.starts_with("CommandLine") {
                                if let Some(pos) = line.find(':') {
                                    current_cmdline = line[pos + 1..].trim().to_string();
                                }
                            } else if line.starts_with("ProcessId") {
                                if let Some(pos) = line.find(':') {
                                    if let Ok(pid) = line[pos + 1..].trim().parse::<u32>() {
                                        current_pid = Some(pid);
                                    }
                                }
                            }

                            if !current_cmdline.is_empty() && current_pid.is_some() {
                                let pid = current_pid.unwrap();
                                let args = parse_cmdline(&current_cmdline);
                                if let Some(service) = self.parse_rclone_args(pid, &args) {
                                    scanned_services.push(service);
                                }
                                current_cmdline.clear();
                                current_pid = None;
                            }
                        }
                    }
                }
            }
        }

        self.services_state.active_services = scanned_services;
    }

    /// Quét các dịch vụ systemd (rclone) cấp hệ thống và cấp cá nhân
    #[cfg(all(unix, not(target_os = "macos")))]
    pub fn scan_systemd_services(&mut self) {
        let mut services_map = std::collections::HashMap::new();

        // 1. Quét tệp trên đĩa cứng
        let system_dir = "/etc/systemd/system";
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/home/bimatkeo".to_string());
        let user_dir = format!("{}/.config/systemd/user", home_dir);

        let mut scan_dir = |dir_path: &str, is_user: bool| {
            if let Ok(entries) = std::fs::read_dir(dir_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        if filename.ends_with(".service") {
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                if content.to_lowercase().contains("rclone") || filename.to_lowercase().contains("rclone") {
                                    let mut desc = String::new();
                                    for line in content.lines() {
                                        let trimmed = line.trim();
                                        if trimmed.starts_with("Description=") {
                                            desc = trimmed["Description=".len()..].trim().to_string();
                                            break;
                                        }
                                    }
                                    let name = filename.clone();
                                    services_map.insert(
                                        (name.clone(), is_user),
                                        ui::services::SystemdServiceInfo {
                                            name,
                                            file_path: path.to_string_lossy().to_string(),
                                            is_user,
                                            active_status: "inactive".to_string(),
                                            sub_state: "dead".to_string(),
                                            description: desc,
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
        };

        scan_dir(system_dir, false);
        scan_dir(&user_dir, true);

        // 2. Chạy systemctl để lấy thông tin trạng thái hoạt động cấp hệ thống
        if let Ok(output) = std::process::Command::new("systemctl")
            .args(["list-units", "--type=service", "--all", "--no-legend"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let unit_name = parts[0];
                    if unit_name.ends_with(".service") {
                        let name = unit_name.to_string();
                        let key = (name.clone(), false);
                        if services_map.contains_key(&key) || name.to_lowercase().contains("rclone") {
                            let _load_state = parts[1];
                            let active_status = parts[2].to_string();
                            let sub_state = parts[3].to_string();
                            let description = parts[4..].join(" ");

                            if let Some(service) = services_map.get_mut(&key) {
                                  service.active_status = active_status;
                                  service.sub_state = sub_state;
                                  if !description.is_empty() {
                                      service.description = description;
                                  }
                              } else {
                                  services_map.insert(
                                      key,
                                      ui::services::SystemdServiceInfo {
                                          name,
                                          file_path: format!("/etc/systemd/system/{}", unit_name),
                                          is_user: false,
                                          active_status,
                                          sub_state,
                                          description,
                                      },
                                  );
                              }
                          }
                      }
                  }
              }
          }

          // 3. Chạy systemctl --user để lấy thông tin trạng thái cấp cá nhân
          if let Ok(output) = std::process::Command::new("systemctl")
              .args(["--user", "list-units", "--type=service", "--all", "--no-legend"])
              .output()
          {
              let stdout = String::from_utf8_lossy(&output.stdout);
              for line in stdout.lines() {
                  let parts: Vec<&str> = line.split_whitespace().collect();
                  if parts.len() >= 4 {
                      let unit_name = parts[0];
                      if unit_name.ends_with(".service") {
                          let name = unit_name.to_string();
                          let key = (name.clone(), true);
                          if services_map.contains_key(&key) || name.to_lowercase().contains("rclone") {
                              let _load_state = parts[1];
                              let active_status = parts[2].to_string();
                              let sub_state = parts[3].to_string();
                              let description = parts[4..].join(" ");

                              if let Some(service) = services_map.get_mut(&key) {
                                  service.active_status = active_status;
                                  service.sub_state = sub_state;
                                  if !description.is_empty() {
                                      service.description = description;
                                  }
                              } else {
                                  services_map.insert(
                                      key,
                                      ui::services::SystemdServiceInfo {
                                          name,
                                          file_path: format!("{}/{}", user_dir, unit_name),
                                          is_user: true,
                                          active_status,
                                          sub_state,
                                          description,
                                      },
                                  );
                              }
                          }
                      }
                  }
              }
          }

          let mut services: Vec<ui::services::SystemdServiceInfo> = services_map.into_values().collect();
          services.sort_by(|a, b| a.name.cmp(&b.name));
          self.services_state.systemd_services = services;
      }

    /// Quét các dịch vụ systemd (rclone) cấp hệ thống và cấp cá nhân (không hoạt động trên Windows/macOS)
    #[cfg(any(not(unix), target_os = "macos"))]
    pub fn scan_systemd_services(&mut self) {
        self.services_state.systemd_services.clear();
    }

    fn parse_exec_start_full(
        &self,
        exec_start: &str,
    ) -> (String, String, std::collections::HashMap<String, String>) {
        let words: Vec<&str> = exec_start.split_whitespace().collect();
        let mut remote = String::new();
        let mut mount_path = String::new();
        let mut flags = std::collections::HashMap::new();
        
        if let Some(mount_pos) = words.iter().position(|&w| w == "mount") {
            let mut non_flags = Vec::new();
            let mut i = mount_pos + 1;
            while i < words.len() {
                let w = words[i];
                if w.starts_with("--") {
                    if let Some(eq_pos) = w.find('=') {
                        let key = w[..eq_pos].to_string();
                        let val = w[eq_pos + 1..].to_string();
                        flags.insert(key, val);
                    } else {
                        if i + 1 < words.len() && !words[i + 1].starts_with("--") {
                            flags.insert(w.to_string(), words[i + 1].to_string());
                            i += 1;
                        } else {
                            flags.insert(w.to_string(), "true".to_string());
                        }
                    }
                } else {
                    non_flags.push(w.to_string());
                }
                i += 1;
            }
            if non_flags.len() >= 2 {
                remote = non_flags[0].clone();
                mount_path = non_flags[1].clone();
            } else if non_flags.len() == 1 {
                mount_path = non_flags[0].clone();
            }
        }
        (remote, mount_path, flags)
    }

    fn parse_systemd_file(&self, file_path: &str) -> std::io::Result<Vec<(String, String)>> {
        let content = std::fs::read_to_string(file_path)?;
        let mut fields = Vec::new();
        let mut current_section = String::new();

        let mut lines = content.lines().peekable();
        while let Some(line) = lines.next() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
                continue;
            }

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                current_section = trimmed[1..trimmed.len() - 1].to_string();
                continue;
            }

            if let Some(pos) = trimmed.find('=') {
                let key = trimmed[..pos].trim();
                let mut val = trimmed[pos + 1..].trim().to_string();

                while val.ends_with('\\') {
                    val.pop();
                    if let Some(next_line) = lines.next() {
                        val.push_str(" ");
                        val.push_str(next_line.trim());
                    } else {
                        break;
                    }
                }

                let full_key = if current_section.is_empty() {
                    key.to_string()
                } else {
                    format!("[{}]{}", current_section, key)
                };
                fields.push((full_key, val));
            }
        }

        Ok(fields)
    }

    fn load_systemd_service_fields(
        &self,
        file_path: &str,
        is_user: bool,
    ) -> std::io::Result<Vec<(String, String, String, Vec<String>)>> {
        let raw_fields = self.parse_systemd_file(file_path)?;
        let mut fields = Vec::new();

        let filename = std::path::Path::new(file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let service_name = filename.strip_suffix(".service").unwrap_or(&filename).to_string();

        let desc = raw_fields
            .iter()
            .find(|(k, _)| k == "[Unit]Description")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();

        let user = raw_fields
            .iter()
            .find(|(k, _)| k == "[Service]User")
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| std::env::var("USER").unwrap_or_else(|_| "bimatkeo".to_string()));

        let exec_start = raw_fields
            .iter()
            .find(|(k, _)| k == "[Service]ExecStart")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();

        let (remote, mount_path, parsed_flags) = self.parse_exec_start_full(&exec_start);

        fields.push((
            "_service_name".to_string(),
            "Tên dịch vụ".to_string(),
            service_name,
            Vec::new(),
        ));
        fields.push((
            "_service_level".to_string(),
            "Cấp chạy".to_string(),
            if is_user { "User (Cá nhân)".to_string() } else { "System (Hệ thống)".to_string() },
            vec!["User (Cá nhân)".to_string(), "System (Hệ thống)".to_string()],
        ));
        fields.push((
            "_remote".to_string(),
            "Cloud Remote".to_string(),
            remote,
            self.connection_state.remotes.clone(),
        ));
        fields.push((
            "_mount_path".to_string(),
            "Thư mục Mount".to_string(),
            mount_path,
            Vec::new(),
        ));
        fields.push((
            "_description".to_string(),
            "Mô tả".to_string(),
            desc,
            Vec::new(),
        ));
        fields.push((
            "_user".to_string(),
            "Tài khoản chạy".to_string(),
            user,
            Vec::new(),
        ));

        let get_flag = |key: &str| parsed_flags.get(key).cloned().unwrap_or_default();
        fields.push((
            "_flag_vfs_cache_mode".to_string(),
            "Chế độ Cache VFS".to_string(),
            get_flag("--vfs-cache-mode"),
            vec!["".to_string(), "off".to_string(), "minimal".to_string(), "writes".to_string(), "full".to_string()],
        ));
        fields.push((
            "_flag_vfs_cache_max_size".to_string(),
            "Dung lượng Cache tối đa".to_string(),
            get_flag("--vfs-cache-max-size"),
            Vec::new(),
        ));
        fields.push((
            "_flag_vfs_cache_max_age".to_string(),
            "Thời gian Cache tối đa".to_string(),
            get_flag("--vfs-cache-max-age"),
            Vec::new(),
        ));
        fields.push((
            "_flag_vfs_read_chunk_size".to_string(),
            "Kích thước đoạn đọc".to_string(),
            get_flag("--vfs-read-chunk-size"),
            Vec::new(),
        ));
        fields.push((
            "_flag_vfs_read_chunk_size_limit".to_string(),
            "Giới hạn đoạn đọc".to_string(),
            get_flag("--vfs-read-chunk-size-limit"),
            Vec::new(),
        ));
        fields.push((
            "_flag_dir_cache_time".to_string(),
            "Thời gian Cache thư mục".to_string(),
            get_flag("--dir-cache-time"),
            Vec::new(),
        ));
        fields.push((
            "_flag_attr_timeout".to_string(),
            "Timeout thuộc tính".to_string(),
            get_flag("--attr-timeout"),
            Vec::new(),
        ));
        fields.push((
            "_flag_buffer_size".to_string(),
            "Kích thước buffer RAM".to_string(),
            get_flag("--buffer-size"),
            Vec::new(),
        ));
        
        let allow_other_val = if parsed_flags.contains_key("--allow-other") { "Có (yes)".to_string() } else { "".to_string() };
        fields.push((
            "_flag_allow_other".to_string(),
            "Cho phép User khác".to_string(),
            allow_other_val,
            vec!["".to_string(), "Có (yes)".to_string()],
        ));

        let read_only_val = if parsed_flags.contains_key("--read-only") { "Có (yes)".to_string() } else { "".to_string() };
        fields.push((
            "_flag_read_only".to_string(),
            "Chế độ chỉ đọc".to_string(),
            read_only_val,
            vec!["".to_string(), "Có (yes)".to_string()],
        ));

        let allow_non_empty_val = if parsed_flags.contains_key("--allow-non-empty") { "Có (yes)".to_string() } else { "".to_string() };
        fields.push((
            "_flag_allow_non_empty".to_string(),
            "Cho phép thư mục chứa file".to_string(),
            allow_non_empty_val,
            vec!["".to_string(), "Có (yes)".to_string()],
        ));

        for (k, v) in raw_fields {
            let choices = if k == "[Service]Type" {
                vec!["simple".to_string(), "forking".to_string(), "oneshot".to_string(), "dbus".to_string(), "notify".to_string(), "idle".to_string()]
            } else if k == "[Service]Restart" {
                vec!["no".to_string(), "on-success".to_string(), "on-failure".to_string(), "on-abnormal".to_string(), "on-watchdog".to_string(), "on-abort".to_string(), "always".to_string()]
            } else {
                Vec::new()
            };
            fields.push((k, String::new(), v, choices));
        }

        Ok(fields)
    }

    fn init_create_systemd_service_fields(&self) -> Vec<(String, String, String, Vec<String>)> {
        let mut fields = Vec::new();
        let user = std::env::var("USER").unwrap_or_else(|_| "bimatkeo".to_string());
        let default_remote = self.connection_state.remotes.first().cloned().unwrap_or_default();

        fields.push((
            "_service_name".to_string(),
            "Tên dịch vụ".to_string(),
            "rclone-torrent".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_service_level".to_string(),
            "Cấp chạy".to_string(),
            "User (Cá nhân)".to_string(),
            vec!["User (Cá nhân)".to_string(), "System (Hệ thống)".to_string()],
        ));
        fields.push((
            "_remote".to_string(),
            "Cloud Remote".to_string(),
            default_remote,
            self.connection_state.remotes.clone(),
        ));
        fields.push((
            "_mount_path".to_string(),
            "Thư mục Mount".to_string(),
            "/media/bimatkeo/DATA/Torrents".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_description".to_string(),
            "Mô tả".to_string(),
            "Rclone Mount Service".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_user".to_string(),
            "Tài khoản chạy".to_string(),
            user,
            Vec::new(),
        ));

        // Rclone cờ ảo mặc định khi tạo mới
        fields.push((
            "_flag_vfs_cache_mode".to_string(),
            "Chế độ Cache VFS".to_string(),
            "full".to_string(),
            vec!["".to_string(), "off".to_string(), "minimal".to_string(), "writes".to_string(), "full".to_string()],
        ));
        fields.push((
            "_flag_vfs_cache_max_size".to_string(),
            "Dung lượng Cache tối đa".to_string(),
            "10G".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_flag_vfs_cache_max_age".to_string(),
            "Thời gian Cache tối đa".to_string(),
            "72h".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_flag_vfs_read_chunk_size".to_string(),
            "Kích thước đoạn đọc".to_string(),
            "32M".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_flag_vfs_read_chunk_size_limit".to_string(),
            "Giới hạn đoạn đọc".to_string(),
            "off".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_flag_dir_cache_time".to_string(),
            "Thời gian Cache thư mục".to_string(),
            "72h".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_flag_attr_timeout".to_string(),
            "Timeout thuộc tính".to_string(),
            "72h".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_flag_buffer_size".to_string(),
            "Kích thước buffer RAM".to_string(),
            "64M".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_flag_allow_other".to_string(),
            "Cho phép User khác".to_string(),
            "".to_string(),
            vec!["".to_string(), "Có (yes)".to_string()],
        ));
        fields.push((
            "_flag_read_only".to_string(),
            "Chế độ chỉ đọc".to_string(),
            "".to_string(),
            vec!["".to_string(), "Có (yes)".to_string()],
        ));
        fields.push((
            "_flag_allow_non_empty".to_string(),
            "Cho phép thư mục chứa file".to_string(),
            "Có (yes)".to_string(),
            vec!["".to_string(), "Có (yes)".to_string()],
        ));

        fields.push(("[Unit]Description".to_string(), String::new(), "Rclone Mount Service".to_string(), Vec::new()));
        fields.push(("[Unit]After".to_string(), String::new(), "network-online.target".to_string(), Vec::new()));
        fields.push(("[Unit]Wants".to_string(), String::new(), "network-online.target".to_string(), Vec::new()));
        fields.push(("[Service]Type".to_string(), String::new(), "notify".to_string(), vec!["simple".to_string(), "forking".to_string(), "oneshot".to_string(), "dbus".to_string(), "notify".to_string(), "idle".to_string()]));
        fields.push(("[Service]ExecStart".to_string(), String::new(), String::new(), Vec::new()));
        fields.push(("[Service]ExecStop".to_string(), String::new(), String::new(), Vec::new()));
        fields.push(("[Service]Restart".to_string(), String::new(), "on-failure".to_string(), vec!["no".to_string(), "on-success".to_string(), "on-failure".to_string(), "on-abnormal".to_string(), "on-watchdog".to_string(), "on-abort".to_string(), "always".to_string()]));
        fields.push(("[Service]RestartSec".to_string(), String::new(), "10s".to_string(), Vec::new()));
        fields.push(("[Install]WantedBy".to_string(), String::new(), "default.target".to_string(), Vec::new()));

        fields
    }

    fn save_systemd_service_file(
        &mut self,
        _is_create: bool,
        _service_name: &str,
        file_path: &str,
        is_user: bool,
        fields: &[(String, String, String, Vec<String>)],
    ) -> std::io::Result<()> {
        let mut final_fields = fields.to_vec();

        let active_tab = match &self.services_state.wizard {
            ui::services::ServicesWizardState::EditSystemdService { active_tab, .. } => *active_tab,
            ui::services::ServicesWizardState::CreateSystemdService { active_tab, .. } => *active_tab,
            _ => 0,
        };

        if active_tab == 0 {
            let get_val = |key: &str| {
                final_fields
                    .iter()
                    .find(|(k, _, _, _)| k == key)
                    .map(|(_, _, v, _)| v.clone())
                    .unwrap_or_default()
            };

            let remote = get_val("_remote");
            let mount_path = get_val("_mount_path");
            let desc = get_val("_description");
            let user = get_val("_user");

            // Ensure mount point directory exists and is writable
            if !mount_path.is_empty() {
                let mut need_escalation = false;
                let is_drive = cfg!(windows) && (
                    (mount_path.len() == 2 && mount_path.ends_with(':'))
                    || (mount_path.len() == 3 && mount_path.ends_with(":\\"))
                    || (mount_path.len() == 3 && mount_path.ends_with(":/"))
                );

                if !is_drive {
                    if std::fs::create_dir_all(&mount_path).is_err() {
                        need_escalation = true;
                    } else {
                        let temp_file = std::path::Path::new(&mount_path).join(".rclone_tui_temp");
                        if std::fs::write(&temp_file, "").is_err() {
                            need_escalation = true;
                        } else {
                            let _ = std::fs::remove_file(temp_file);
                        }
                    }
                }

                #[cfg(unix)]
                if need_escalation {
                    self.services_state.info_message = Some("Yêu cầu xác thực quyền root để tạo và phân quyền thư mục mount...".to_string());
                    let _ = Command::new("pkexec")
                        .args(&["mkdir", "-p", &mount_path])
                        .status();

                    let username = std::process::Command::new("id")
                        .args(&["-u", "-n"])
                        .output()
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                        .unwrap_or_else(|_| "bimatkeo".to_string());

                    let groupname = std::process::Command::new("id")
                        .args(&["-g", "-n"])
                        .output()
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                        .unwrap_or_else(|_| "bimatkeo".to_string());

                    let owner_arg = format!("{}:{}", username, groupname);
                    let _ = Command::new("pkexec")
                        .args(&["chown", "-R", &owner_arg, &mount_path])
                        .status();
                }

                #[cfg(windows)]
                if need_escalation {
                    self.services_state.info_message = Some("Yêu cầu xác thực Administrator để tạo thư mục mount...".to_string());
                    let _ = Command::new("powershell")
                        .args(&[
                            "-NoProfile",
                            "-Command",
                            &format!("Start-Process cmd -ArgumentList '/c mkdir \"{}\"' -Verb RunAs -WindowStyle Hidden -Wait", mount_path)
                        ])
                        .status();
                }
            }

            let config_path = self.config.get_active_profile_path();

            let mut exec_start = format!(
                "/usr/bin/rclone mount {} {} --config {}",
                remote, mount_path, config_path
            );
            
            let vfs_cache_mode = get_val("_flag_vfs_cache_mode");
            if !vfs_cache_mode.is_empty() {
                exec_start.push_str(&format!(" --vfs-cache-mode {}", vfs_cache_mode));
            }
            let vfs_cache_max_size = get_val("_flag_vfs_cache_max_size");
            if !vfs_cache_max_size.is_empty() {
                exec_start.push_str(&format!(" --vfs-cache-max-size {}", vfs_cache_max_size));
            }
            let vfs_cache_max_age = get_val("_flag_vfs_cache_max_age");
            if !vfs_cache_max_age.is_empty() {
                exec_start.push_str(&format!(" --vfs-cache-max-age {}", vfs_cache_max_age));
            }
            let vfs_read_chunk_size = get_val("_flag_vfs_read_chunk_size");
            if !vfs_read_chunk_size.is_empty() {
                exec_start.push_str(&format!(" --vfs-read-chunk-size {}", vfs_read_chunk_size));
            }
            let vfs_read_chunk_size_limit = get_val("_flag_vfs_read_chunk_size_limit");
            if !vfs_read_chunk_size_limit.is_empty() {
                exec_start.push_str(&format!(" --vfs-read-chunk-size-limit {}", vfs_read_chunk_size_limit));
            }
            let dir_cache_time = get_val("_flag_dir_cache_time");
            if !dir_cache_time.is_empty() {
                exec_start.push_str(&format!(" --dir-cache-time {}", dir_cache_time));
            }
            let attr_timeout = get_val("_flag_attr_timeout");
            if !attr_timeout.is_empty() {
                exec_start.push_str(&format!(" --attr-timeout {}", attr_timeout));
            }
            let buffer_size = get_val("_flag_buffer_size");
            if !buffer_size.is_empty() {
                exec_start.push_str(&format!(" --buffer-size {}", buffer_size));
            }
            let allow_other = get_val("_flag_allow_other");
            if !allow_other.is_empty() {
                exec_start.push_str(" --allow-other");
            }
            let read_only = get_val("_flag_read_only");
            if !read_only.is_empty() {
                exec_start.push_str(" --read-only");
            }
            let allow_non_empty = get_val("_flag_allow_non_empty");
            if !allow_non_empty.is_empty() {
                exec_start.push_str(" --allow-non-empty");
            }
            let exec_stop = format!("/bin/fusermount -uz {}", mount_path);

            let mut update_raw = |key: &str, val: &str| {
                if let Some(item) = final_fields.iter_mut().find(|(k, _, _, _)| k == key) {
                    item.2 = val.to_string();
                } else {
                    final_fields.push((key.to_string(), String::new(), val.to_string(), Vec::new()));
                }
            };

            update_raw("[Unit]Description", &desc);
            update_raw("[Service]ExecStart", &exec_start);
            update_raw("[Service]ExecStop", &exec_stop);

            if is_user {
                update_raw("[Install]WantedBy", "default.target");
                final_fields.retain(|(k, _, _, _)| k != "[Service]User" && k != "[Service]Group");
            } else {
                update_raw("[Service]User", &user);
                update_raw("[Service]Group", &user);
                update_raw("[Install]WantedBy", "multi-user.target");
            }
        }

        let home_dir = crate::app_config::get_home_dir();
        let temp_file_path = format!("{}/.config/rclone-tui-temp.service", home_dir);

        let mut sections: std::collections::BTreeMap<String, Vec<(String, String)>> = std::collections::BTreeMap::new();
        for (name, _, value, _) in &final_fields {
            if name.starts_with('_') {
                continue;
            }
            if name.starts_with('[') {
                if let Some(pos) = name.find(']') {
                    let sec = name[1..pos].to_string();
                    let key = name[pos + 1..].to_string();
                    sections.entry(sec).or_default().push((key, value.clone()));
                }
            }
        }

        let mut content = String::new();
        let ordered_sections = vec!["Unit", "Service", "Install"];
        for sec in ordered_sections {
            if let Some(keys) = sections.remove(sec) {
                content.push_str(&format!("[{}]\n", sec));
                for (k, v) in keys {
                    content.push_str(&format!("{}={}\n", k, v));
                }
                content.push_str("\n");
            }
        }
        for (sec, keys) in sections {
            content.push_str(&format!("[{}]\n", sec));
            for (k, v) in keys {
                content.push_str(&format!("{}={}\n", k, v));
            }
            content.push_str("\n");
        }

        std::fs::create_dir_all(format!("{}/.config", home_dir))?;
        std::fs::write(&temp_file_path, content)?;

        if is_user {
            let parent_dir = std::path::Path::new(file_path).parent().unwrap();
            std::fs::create_dir_all(parent_dir)?;
            std::fs::copy(&temp_file_path, file_path)?;
            let _ = std::fs::remove_file(&temp_file_path);

            let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).status();
        } else {
            let parent_dir = std::path::Path::new(file_path).parent().unwrap().to_string_lossy().to_string();
            let _ = Command::new("pkexec").args(["mkdir", "-p", &parent_dir]).status();

            let status = Command::new("pkexec")
                .args(["mv", &temp_file_path, file_path])
                .status()?;

            if !status.success() {
                return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "pkexec mv failed"));
            }

            let _ = Command::new("pkexec").args(["chown", "root:root", file_path]).status();
            let _ = Command::new("pkexec").args(["chmod", "644", file_path]).status();
            let _ = Command::new("pkexec").args(["systemctl", "daemon-reload"]).status();
        }

        Ok(())
    }

    fn ensure_mount_point_exists_from_service_file(&self, file_path: &str) {
        if let Ok(content) = std::fs::read_to_string(file_path) {
            for line in content.lines() {
                let is_mount = line.contains(" mount ");
                let is_nfsmount = line.contains(" nfsmount ");
                if line.starts_with("ExecStart=") && (is_mount || is_nfsmount) {
                    let mount_pos = if is_mount {
                        line.find(" mount ").unwrap()
                    } else {
                        line.find(" nfsmount ").unwrap()
                    };
                    let mount_word_len = if is_mount { 7 } else { 10 };
                    let after_mount = &line[mount_pos + mount_word_len..];
                        let mut args = Vec::new();
                        let mut current = String::new();
                        let mut in_quotes = false;
                        let mut quote_char = ' ';
                        for c in after_mount.chars() {
                            if (c == '"' || c == '\'') && !in_quotes {
                                in_quotes = true;
                                quote_char = c;
                            } else if c == quote_char && in_quotes {
                                in_quotes = false;
                            } else if c == ' ' && !in_quotes {
                                if !current.is_empty() {
                                    args.push(current.clone());
                                    current.clear();
                                }
                            } else {
                                current.push(c);
                            }
                        }
                        if !current.is_empty() {
                            args.push(current);
                        }

                        if args.len() >= 2 {
                            let mount_path = &args[1];
                            if mount_path.starts_with('/') {
                                let mut need_sudo = false;
                                if std::fs::create_dir_all(mount_path).is_err() {
                                    need_sudo = true;
                                } else {
                                    let temp_file = std::path::Path::new(mount_path).join(".rclone_tui_temp");
                                    if std::fs::write(&temp_file, "").is_err() {
                                        need_sudo = true;
                                    } else {
                                        let _ = std::fs::remove_file(temp_file);
                                    }
                                }

                                if need_sudo {
                                    let _ = Command::new("pkexec")
                                        .args(&["mkdir", "-p", mount_path])
                                        .status();

                                    let username = std::process::Command::new("id")
                                        .args(&["-u", "-n"])
                                        .output()
                                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                                        .unwrap_or_else(|_| "bimatkeo".to_string());

                                    let groupname = std::process::Command::new("id")
                                        .args(&["-g", "-n"])
                                        .output()
                                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                                        .unwrap_or_else(|_| "bimatkeo".to_string());

                                    let owner_arg = format!("{}:{}", username, groupname);
                                    let _ = Command::new("pkexec")
                                        .args(&["chown", "-R", &owner_arg, mount_path])
                                        .status();
                                }
                            }
                        }
                    }
                }
            }
        }

    fn get_systemd_error_logs(&self, service_name: &str, is_user: bool) -> String {
        let mut cmd = Command::new("journalctl");
        if is_user {
            cmd.args(["--user", "-u", service_name, "-n", "10", "--no-pager"]);
        } else {
            cmd.args(["-u", service_name, "-n", "10", "--no-pager"]);
        }
        if let Ok(output) = cmd.output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut lines = Vec::new();
            for line in stdout.lines() {
                if line.contains("error") || line.contains("Error") || line.contains("Fatal") || line.contains("failed") {
                    lines.push(line.trim().to_string());
                }
            }
            if !lines.is_empty() {
                return lines.join("\n");
            }
            let all_lines: Vec<&str> = stdout.lines().collect();
            if all_lines.len() >= 3 {
                return all_lines[all_lines.len() - 3..].join("\n");
            } else {
                return stdout.into_owned();
            }
        }
        String::new()
    }

    /// Đọc danh sách tiến trình chạy ngầm bằng cách quét tiến trình thực tế
    pub fn load_active_services_from_file(&mut self) {
        self.scan_running_services();
        self.scan_systemd_services();
    }

    /// Lưu danh sách tiến trình chạy ngầm (không cần thiết nữa vì quét động)
    #[allow(dead_code)]
    pub fn save_active_services_to_file(&self) {
        // Trống
    }

    /// Nạp danh sách các Cloud Remotes và kích hoạt kiểm tra kết nối qua tác vụ ngầm
    pub async fn load_remotes(&mut self, _tx: tokio::sync::mpsc::UnboundedSender<AppEvent>) {
        self.scan_running_services();
        self.scan_systemd_services();
        let res = rclone::rpc_async("config/listremotes".to_string(), "{}".to_string()).await;
        if let Ok(rpc_res) = res {
            if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                if let Some(arr) = val.get("remotes").and_then(|r| r.as_array()) {
                    let remotes: Vec<String> = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                    self.connection_state.remotes = remotes.clone();
                    self.services_state.all_remotes = remotes.clone();

                    // Cập nhật remote_dependencies từ config/dump
                    self.remote_dependencies.clear();
                    let dump_res = rclone::rpc_async("config/dump".to_string(), "{}".to_string()).await;
                    if let Ok(rpc_dump) = dump_res {
                        if let Ok(dump_val) = serde_json::from_str::<Value>(&rpc_dump.output) {
                            if let Some(obj) = dump_val.as_object() {
                                for (name, details) in obj {
                                    if let Some(details_obj) = details.as_object() {
                                        if let Some(r_type) = details_obj.get("type").and_then(|t| t.as_str()) {
                                            if r_type == "crypt" || r_type == "alias" {
                                                if let Some(base_remote_path) = details_obj.get("remote").and_then(|r| r.as_str()) {
                                                    let base_name = if let Some(idx) = base_remote_path.find(':') {
                                                        &base_remote_path[..idx]
                                                    } else {
                                                        base_remote_path
                                                    };
                                                    self.remote_dependencies.insert(name.clone(), base_name.to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Khởi tạo trạng thái ban đầu cho các remote mới nếu chưa có
                    for remote in remotes {
                        self.connection_state
                            .remote_statuses
                            .entry(remote)
                            .or_insert_with(|| "Đang kiểm tra...".to_string());
                    }

                    // Kích hoạt tác vụ ngầm kiểm tra lại ngay lập tức
                    if let Some(ref trigger_tx) = self.status_trigger_tx {
                        let _ = trigger_tx.send(());
                    }
                }
            }
        }
    }

    /// Cập nhật danh sách file trong File Explorer
    pub async fn refresh_explorer_pane(
        &mut self,
        pane_type: ui::explorer::ActivePane,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        let pane = match pane_type {
            ui::explorer::ActivePane::Left => &mut self.explorer_state.left_pane,
            ui::explorer::ActivePane::Right => &mut self.explorer_state.right_pane,
        };
        pane.loading = true;

        let remote = pane.remote.clone();
        let path = pane.path.clone();

        tokio::spawn(async move {
            let fs_target = if remote.is_empty() {
                path.clone()
            } else {
                let clean_remote = remote.trim_end_matches(':');
                let clean_path = if path.starts_with('/') {
                    path.clone()
                } else {
                    format!("/{}", path)
                };
                format!("{}:{}", clean_remote, clean_path)
            };

            let input_param = json!({
                "fs": fs_target,
                "remote": "",
            })
            .to_string();

            let list_future = rclone::rpc_async("operations/list".to_string(), input_param.clone());
            let mut res = match tokio::time::timeout(std::time::Duration::from_secs(15), list_future).await {
                Ok(inner_res) => inner_res,
                Err(_) => Err("Hết thời gian chờ phản hồi từ Cloud (Timeout)".to_string()),
            };

            // Tự động tạo thư mục nếu phát hiện lỗi "không tìm thấy thư mục" trên remote
            let mut directory_not_found = false;
            if let Ok(ref rpc_res) = res {
                if rpc_res.status != 200 {
                    if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                        if let Some(err_str) = val.get("error").and_then(|e| e.as_str()) {
                            let lower = err_str.to_lowercase();
                            if lower.contains("directory not found")
                                || lower.contains("not found")
                                || lower.contains("does not exist")
                                || lower.contains("no such file or directory")
                            {
                                directory_not_found = true;
                            }
                        }
                    }
                }
            } else if let Err(ref e) = res {
                let lower = e.to_lowercase();
                if lower.contains("directory not found")
                    || lower.contains("not found")
                    || lower.contains("does not exist")
                    || lower.contains("no such file or directory")
                {
                    directory_not_found = true;
                }
            }

            if directory_not_found && !remote.is_empty() {
                let mkdir_param = json!({
                    "fs": fs_target.clone(),
                    "remote": "",
                })
                .to_string();
                let mkdir_res = rclone::rpc_async("operations/mkdir".to_string(), mkdir_param).await;
                if mkdir_res.is_ok() {
                    let list_future2 = rclone::rpc_async("operations/list".to_string(), input_param);
                    res = match tokio::time::timeout(std::time::Duration::from_secs(15), list_future2).await {
                        Ok(inner_res) => inner_res,
                        Err(_) => Err("Hết thời gian chờ phản hồi từ Cloud (Timeout)".to_string()),
                    };
                }
            }

            match res {
                Ok(rpc_res) => {
                    if rpc_res.status == 200 {
                        if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                            if let Some(err_str) = val.get("error").and_then(|e| e.as_str()) {
                                let _ = tx.send(AppEvent::ExplorerListResult {
                                    pane: pane_type,
                                    result: Err(err_str.to_string()),
                                });
                            } else if let Some(list_arr) = val.get("list").and_then(|l| l.as_array()) {
                                let mut items = Vec::new();
                                for item_val in list_arr {
                                    let name = item_val
                                        .get("Name")
                                        .and_then(|n| n.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let size =
                                        item_val.get("Size").and_then(|s| s.as_u64()).unwrap_or(0);
                                    let is_dir = item_val
                                        .get("IsDir")
                                        .and_then(|d| d.as_bool())
                                        .unwrap_or(false);
                                    let mod_time = item_val
                                        .get("ModTime")
                                        .and_then(|m| m.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    // Clean mod_time ISO8601 string (Bug 89)
                                    let cleaned_time = mod_time
                                        .chars()
                                        .take(19)
                                        .collect::<String>()
                                        .replace("T", " ");

                                    items.push(ui::explorer::FileItem {
                                        name,
                                        size,
                                        is_dir,
                                        mod_time: cleaned_time,
                                    });
                                }
                                // Sắp xếp thư mục lên trước, file sau
                                items.sort_by(|a, b| {
                                    b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name))
                                });

                                // Bổ sung thư mục cha ".." nếu không ở thư mục gốc
                                let at_root = if remote.is_empty() {
                                    path == "/" || path.is_empty()
                                } else {
                                    path.is_empty()
                                };
                                if !at_root {
                                    items.insert(
                                        0,
                                        ui::explorer::FileItem {
                                            name: "..".to_string(),
                                            size: 0,
                                            is_dir: true,
                                            mod_time: "---".to_string(),
                                        },
                                    );
                                }

                                let _ = tx.send(AppEvent::ExplorerListResult {
                                    pane: pane_type,
                                    result: Ok(items),
                                });
                            } else {
                                let _ = tx.send(AppEvent::ExplorerListResult {
                                    pane: pane_type,
                                    result: Err("Không thể phân tích dữ liệu danh sách".to_string()),
                                });
                            }
                        } else {
                            let _ = tx.send(AppEvent::ExplorerListResult {
                                pane: pane_type,
                                result: Err("JSON lỗi".to_string()),
                            });
                        }
                    } else {
                        // Trích xuất lỗi chi tiết từ JSON nếu có
                        let err_msg = serde_json::from_str::<Value>(&rpc_res.output)
                            .ok()
                            .and_then(|val| val.get("error").and_then(|e| e.as_str()).map(|s| s.to_string()))
                            .unwrap_or_else(|| format!("Lỗi RPC (Mã {}): {}", rpc_res.status, rpc_res.output));
                        let _ = tx.send(AppEvent::ExplorerListResult {
                            pane: pane_type,
                            result: Err(err_msg),
                        });
                    }
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::ExplorerListResult {
                        pane: pane_type,
                        result: Err(e),
                    });
                }
            }
        });
    }

    pub async fn refresh_wizard_gui_list(
        &mut self,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        let (remote, path) = match self.services_state.wizard {
            ui::services::ServicesWizardState::GuiSelectPath {
                ref remote,
                ref current_path,
                ref mut loading,
                ..
            } => {
                *loading = true;
                (remote.clone(), current_path.clone())
            }
            ui::services::ServicesWizardState::GuiSelectLocalPath {
                ref mut loading,
                ref current_path,
                ..
            } => {
                *loading = true;
                (String::new(), current_path.clone())
            }
            _ => return,
        };

        tokio::spawn(async move {
                let fs_target = if remote.is_empty() {
                    path.clone()
                } else {
                    let clean_remote = remote.trim_end_matches(':');
                    let clean_path = if path.starts_with('/') {
                        path.clone()
                    } else {
                        format!("/{}", path)
                    };
                    format!("{}:{}", clean_remote, clean_path)
                };

                let input_param = serde_json::json!({
                    "fs": fs_target,
                    "remote": "",
                })
                .to_string();

                let list_future = rclone::rpc_async("operations/list".to_string(), input_param.clone());
                let mut res = match tokio::time::timeout(std::time::Duration::from_secs(15), list_future).await {
                    Ok(inner_res) => inner_res,
                    Err(_) => Err("Hết thời gian chờ phản hồi từ Cloud (Timeout)".to_string()),
                };

                let mut directory_not_found = false;
                if let Ok(ref rpc_res) = res {
                    if rpc_res.status != 200 {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&rpc_res.output) {
                            if let Some(err_str) = val.get("error").and_then(|e| e.as_str()) {
                                let lower = err_str.to_lowercase();
                                if lower.contains("directory not found")
                                    || lower.contains("not found")
                                    || lower.contains("does not exist")
                                    || lower.contains("no such file or directory")
                                {
                                    directory_not_found = true;
                                }
                            }
                        }
                    }
                } else if let Err(ref e) = res {
                    let lower = e.to_lowercase();
                    if lower.contains("directory not found")
                        || lower.contains("not found")
                        || lower.contains("does not exist")
                        || lower.contains("no such file or directory")
                    {
                        directory_not_found = true;
                    }
                }

                if directory_not_found && !remote.is_empty() {
                    let mkdir_param = serde_json::json!({
                        "fs": fs_target.clone(),
                        "remote": "",
                    })
                    .to_string();
                    let mkdir_res = rclone::rpc_async("operations/mkdir".to_string(), mkdir_param).await;
                    if mkdir_res.is_ok() {
                        let list_future2 = rclone::rpc_async("operations/list".to_string(), input_param);
                        res = match tokio::time::timeout(std::time::Duration::from_secs(15), list_future2).await {
                            Ok(inner_res) => inner_res,
                            Err(_) => Err("Hết thời gian chờ phản hồi từ Cloud (Timeout)".to_string()),
                        };
                    }
                }

                match res {
                    Ok(rpc_res) => {
                        if rpc_res.status == 200 {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&rpc_res.output) {
                                if let Some(err_str) = val.get("error").and_then(|e| e.as_str()) {
                                    let _ = tx.send(AppEvent::WizardGuiListResult {
                                        result: Err(err_str.to_string()),
                                    });
                                } else if let Some(list_arr) = val.get("list").and_then(|l| l.as_array()) {
                                    let mut items = Vec::new();
                                    for item_val in list_arr {
                                        let name = item_val
                                            .get("Name")
                                            .and_then(|n| n.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let size =
                                            item_val.get("Size").and_then(|s| s.as_u64()).unwrap_or(0);
                                        let is_dir = item_val
                                            .get("IsDir")
                                            .and_then(|d| d.as_bool())
                                            .unwrap_or(false);
                                        let mod_time = item_val
                                            .get("ModTime")
                                            .and_then(|m| m.as_str())
                                            .unwrap_or("")
                                            .to_string();

                                        let cleaned_time = mod_time
                                            .chars()
                                            .take(19)
                                            .collect::<String>()
                                            .replace("T", " ");

                                        if is_dir {
                                            items.push(ui::explorer::FileItem {
                                                name,
                                                size,
                                                is_dir,
                                                mod_time: cleaned_time,
                                            });
                                        }
                                    }
                                    items.sort_by(|a, b| a.name.cmp(&b.name));

                                    let at_root = if remote.is_empty() {
                                        path == "/" || path.is_empty()
                                    } else {
                                        path.is_empty() || path == "/"
                                    };
                                    if !at_root {
                                        items.insert(
                                            0,
                                            ui::explorer::FileItem {
                                                name: "..".to_string(),
                                                size: 0,
                                                is_dir: true,
                                                mod_time: String::new(),
                                            },
                                        );
                                    }

                                    let _ = tx.send(AppEvent::WizardGuiListResult {
                                        result: Ok(items),
                                    });
                                }
                            }
                        } else {
                            let _ = tx.send(AppEvent::WizardGuiListResult {
                                result: Err(format!("Lỗi kết nối RPC: {}", rpc_res.status)),
                            });
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::WizardGuiListResult { result: Err(e) });
                    }
                }
            });
    }

    fn handle_explorer_list_result(
        &mut self,
        pane_type: ui::explorer::ActivePane,
        result: Result<Vec<ui::explorer::FileItem>, String>,
    ) {
        let pane = match pane_type {
            ui::explorer::ActivePane::Left => &mut self.explorer_state.left_pane,
            ui::explorer::ActivePane::Right => &mut self.explorer_state.right_pane,
        };
        pane.loading = false;
        match result {
            Ok(items) => {
                let new_len = items.len();
                pane.items = items;
                // Clamp chỉ mục chọn theo độ dài thực tế của danh sách mới
                if new_len == 0 {
                    pane.selected_idx = 0;
                    pane.scroll_offset = 0;
                } else {
                    if pane.selected_idx >= new_len {
                        pane.selected_idx = new_len - 1;
                    }
                    if pane.scroll_offset >= new_len {
                        pane.scroll_offset = new_len.saturating_sub(1);
                    }
                }
                self.explorer_state.error_message = None;
            }
            Err(e) => {
                pane.items = Vec::new();
                self.explorer_state.error_message = Some(e);
            }
        }
    }

    fn save_features_cache(&self) {
        let cache_path = crate::app_config::AppConfig::config_dir().join("features_cache.json");
        if let Some(parent) = cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(serialized) = serde_json::to_string_pretty(&self.features_cache) {
            let _ = std::fs::write(&cache_path, serialized);
        }
    }

    fn check_features_and_execute(
        &mut self,
        action_type: &str,
        src: String,
        dest: String,
        is_dir: bool,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        let action = action_type.to_string();

        let get_remote = |fs: &str| -> Option<String> {
            if let Some(idx) = fs.find(':') {
                let name = &fs[..idx];
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
            None
        };

        let src_remote = get_remote(&src);
        let dest_remote = get_remote(&dest);

        // Check cache first
        let src_features_opt = if let Some(ref r) = src_remote {
            self.features_cache.get(r).cloned()
        } else {
            Some(json!({
                "Features": {
                    "Move": true,
                    "DirMove": true,
                    "Copy": true,
                    "Purge": true,
                }
            }))
        };

        let dst_features_opt = if let Some(ref r) = dest_remote {
            self.features_cache.get(r).cloned()
        } else {
            Some(json!({
                "Features": {
                    "Move": true,
                    "DirMove": true,
                    "Copy": true,
                    "Purge": true,
                }
            }))
        };

        // If both are cached, execute immediately!
        if src_features_opt.is_some() && dst_features_opt.is_some() {
            let _ = tx.send(AppEvent::FeaturesChecked {
                action_type: action,
                src,
                dest,
                src_features: src_features_opt,
                dst_features: dst_features_opt,
                is_dir,
            });
            return;
        }

        let tx_check = tx.clone();
        let src_remote_spawn = src_remote.clone();
        let dest_remote_spawn = dest_remote.clone();

        tokio::spawn(async move {
            let mut src_features = src_features_opt;
            if src_features.is_none() {
                if let Some(ref r) = src_remote_spawn {
                    let param = json!({ "fs": format!("{}:", r) }).to_string();
                    if let Ok(res) = rclone::rpc_async("operations/fsinfo".to_string(), param).await {
                        if res.status == 200 {
                            src_features = serde_json::from_str::<Value>(&res.output).ok();
                        }
                    }
                }
            }

            let mut dst_features = dst_features_opt;
            if dst_features.is_none() {
                if let Some(ref r) = dest_remote_spawn {
                    let param = json!({ "fs": format!("{}:", r) }).to_string();
                    if let Ok(res) = rclone::rpc_async("operations/fsinfo".to_string(), param).await {
                        if res.status == 200 {
                            dst_features = serde_json::from_str::<Value>(&res.output).ok();
                        }
                    }
                }
            }

            let _ = tx_check.send(AppEvent::FeaturesChecked {
                action_type: action,
                src,
                dest,
                src_features,
                dst_features,
                is_dir,
            });
        });
    }

    async fn handle_features_checked(
        &mut self,
        action_type: String,
        src: String,
        dest: String,
        src_features: Option<serde_json::Value>,
        dst_features: Option<serde_json::Value>,
        is_dir: bool,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        let get_remote = |fs: &str| -> Option<String> {
            if let Some(idx) = fs.find(':') {
                let name = &fs[..idx];
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
            None
        };

        let src_remote = get_remote(&src);
        let dest_remote = get_remote(&dest);

        let mut cache_updated = false;

        if let Some(ref r) = src_remote {
            if !self.features_cache.contains_key(r) {
                if let Some(ref feats) = src_features {
                    self.features_cache.insert(r.clone(), feats.clone());
                    cache_updated = true;
                }
            }
        }

        if let Some(ref r) = dest_remote {
            if !self.features_cache.contains_key(r) {
                if let Some(ref feats) = dst_features {
                    self.features_cache.insert(r.clone(), feats.clone());
                    cache_updated = true;
                }
            }
        }

        if cache_updated {
            self.save_features_cache();
        }

        let get_feature = |feats: &Option<serde_json::Value>, key: &str| -> bool {
            if let Some(f) = feats {
                if let Some(features_map) = f.get("Features") {
                    if let Some(val) = features_map.get(key) {
                        return val.as_bool().unwrap_or(true);
                    }
                }
            }
            true
        };

        let src_move = get_feature(&src_features, "Move");
        let src_dirmove = get_feature(&src_features, "DirMove");
        let dst_copy = get_feature(&dst_features, "Copy");
        let src_purge = get_feature(&src_features, "Purge");

        if action_type == "move" {
            let supports_native_move = if is_dir { src_dirmove } else { src_move };
            if supports_native_move {
                self.explorer_state.popup = ui::explorer::ExplorerPopup::MoveProgress {
                    src: src.clone(),
                    dest: dest.clone(),
                    pct: 0.0,
                    job_id: None,
                };
                let tx_move = tx.clone();
                let src_clone = src.clone();
                let dest_clone = dest.clone();
                tokio::spawn(async move {
                    let res = run_rpc_job_async_with_progress(
                        "sync/move".to_string(),
                        json!({
                            "srcFs": src_clone,
                            "dstFs": dest_clone,
                        }),
                        Some((src_clone, dest_clone, false)),
                        Some(tx_move.clone()),
                    ).await;
                    let _ = tx_move.send(AppEvent::ExplorerOperationFinished {
                        pane: ui::explorer::ActivePane::Left,
                        op_name: "di chuyển (move)".to_string(),
                        result: res,
                    });
                });
            } else {
                let mut options = Vec::new();
                let mut actions = Vec::new();

                if dst_copy && src_purge {
                    options.push("Sử dụng Sao chép & Xóa (Copy & Delete) trên máy chủ".to_string());
                    actions.push(ui::explorer::FallbackAction::MoveCopyDelete {
                        src: src.clone(),
                        dest: dest.clone(),
                    });
                }

                options.push("Tải về máy rồi Upload lên đích (Local Transfer - Rất chậm)".to_string());
                actions.push(ui::explorer::FallbackAction::MoveLocalTransfer {
                    src: src.clone(),
                    dest: dest.clone(),
                });

                options.push("Hủy bỏ tác vụ".to_string());
                actions.push(ui::explorer::FallbackAction::Cancel);

                self.explorer_state.popup = ui::explorer::ExplorerPopup::ConfirmFallback {
                    title: "DI CHUYỂN KHÔNG ĐƯỢC HỖ TRỢ".to_string(),
                    options,
                    selected_idx: 0,
                    actions,
                };
            }
        } else if action_type == "rename" {
            let supports_native = if is_dir { src_dirmove } else { src_move };
            if supports_native {
                self.explorer_state.popup = ui::explorer::ExplorerPopup::MoveProgress {
                    src: src.clone(),
                    dest: dest.clone(),
                    pct: 0.0,
                    job_id: None,
                };
                let tx_move = tx.clone();
                let is_dir_spawn = is_dir;
                tokio::spawn(async move {
                    let parse_path = |fs: &str| -> (String, String) {
                        let (remote_part, path_part) = if let Some(idx) = fs.find(':') {
                            (format!("{}:", &fs[..idx]), &fs[idx+1..])
                        } else {
                            (String::new(), fs)
                        };
                        if let Some(idx) = path_part.rfind('/') {
                            let parent = &path_part[..idx];
                            let name = &path_part[idx+1..];
                            (format!("{}{}", remote_part, parent), name.to_string())
                        } else {
                            (remote_part, path_part.to_string())
                        }
                    };

                    let res = if is_dir_spawn {
                        run_rpc_job_async("sync/move".to_string(), json!({
                            "srcFs": src,
                            "dstFs": dest,
                        })).await
                    } else {
                        let (src_fs, src_file) = parse_path(&src);
                        let (dst_fs, dst_file) = parse_path(&dest);
                        run_rpc_job_async("operations/movefile".to_string(), json!({
                            "srcFs": src_fs,
                            "srcRemote": src_file,
                            "dstFs": dst_fs,
                            "dstRemote": dst_file,
                        })).await
                    };

                    let _ = tx_move.send(AppEvent::ExplorerOperationFinished {
                        pane: ui::explorer::ActivePane::Left,
                        op_name: "đổi tên (rename)".to_string(),
                        result: res,
                    });
                });
            } else {
                let mut options = Vec::new();
                let mut actions = Vec::new();

                if dst_copy && src_purge {
                    options.push("Sử dụng Sao chép & Xóa (Copy & Delete) trên máy chủ".to_string());
                    actions.push(ui::explorer::FallbackAction::RenameCopyDelete {
                        src: src.clone(),
                        dest: dest.clone(),
                        is_dir,
                    });
                }

                options.push("Tải về máy rồi Upload lên đích (Local Transfer - Rất chậm)".to_string());
                actions.push(ui::explorer::FallbackAction::RenameLocalTransfer {
                    src: src.clone(),
                    dest: dest.clone(),
                    is_dir,
                });

                options.push("Hủy bỏ tác vụ".to_string());
                actions.push(ui::explorer::FallbackAction::Cancel);

                self.explorer_state.popup = ui::explorer::ExplorerPopup::ConfirmFallback {
                    title: "ĐỔI TÊN KHÔNG ĐƯỢC HỖ TRỢ".to_string(),
                    options,
                    selected_idx: 0,
                    actions,
                };
            }
        } else if action_type == "copy" {
            if dst_copy {
                self.explorer_state.popup = ui::explorer::ExplorerPopup::CopyProgress {
                    src: src.clone(),
                    dest: dest.clone(),
                    pct: 0.0,
                    job_id: None,
                };
                let tx_copy = tx.clone();
                let src_clone = src.clone();
                let dest_clone = dest.clone();
                tokio::spawn(async move {
                    let res = run_rpc_job_async_with_progress(
                        "sync/copy".to_string(),
                        json!({
                            "srcFs": src_clone,
                            "dstFs": dest_clone,
                        }),
                        Some((src_clone, dest_clone, true)),
                        Some(tx_copy.clone()),
                    ).await;
                    let _ = tx_copy.send(AppEvent::ExplorerOperationFinished {
                        pane: ui::explorer::ActivePane::Left,
                        op_name: "sao chép (copy)".to_string(),
                        result: res,
                    });
                });
            } else {
                let mut options = Vec::new();
                let mut actions = Vec::new();

                options.push("Tải về máy rồi Upload lên đích (Local Transfer - Rất chậm)".to_string());
                actions.push(ui::explorer::FallbackAction::CopyLocalTransfer {
                    src: src.clone(),
                    dest: dest.clone(),
                });

                options.push("Hủy bỏ tác vụ".to_string());
                actions.push(ui::explorer::FallbackAction::Cancel);

                self.explorer_state.popup = ui::explorer::ExplorerPopup::ConfirmFallback {
                    title: "SAO CHÉP KHÔNG ĐƯỢC HỖ TRỢ".to_string(),
                    options,
                    selected_idx: 0,
                    actions,
                };
            }
        }
    }

    async fn execute_fallback_action(
        &mut self,
        action: ui::explorer::FallbackAction,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        match action {
            ui::explorer::FallbackAction::MoveNative { src, dest }
            | ui::explorer::FallbackAction::MoveLocalTransfer { src, dest } => {
                self.explorer_state.popup = ui::explorer::ExplorerPopup::MoveProgress {
                    src: src.clone(),
                    dest: dest.clone(),
                    pct: 0.0,
                    job_id: None,
                };
                let tx_move = tx.clone();
                let src_clone = src.clone();
                let dest_clone = dest.clone();
                tokio::spawn(async move {
                    let res = run_rpc_job_async_with_progress(
                        "sync/move".to_string(),
                        json!({
                            "srcFs": src_clone,
                            "dstFs": dest_clone,
                        }),
                        Some((src_clone, dest_clone, false)),
                        Some(tx_move.clone()),
                    ).await;
                    let _ = tx_move.send(AppEvent::ExplorerOperationFinished {
                        pane: ui::explorer::ActivePane::Left,
                        op_name: "di chuyển (move)".to_string(),
                        result: res,
                    });
                });
            }
            ui::explorer::FallbackAction::MoveCopyDelete { src, dest } => {
                self.explorer_state.popup = ui::explorer::ExplorerPopup::MoveProgress {
                    src: src.clone(),
                    dest: dest.clone(),
                    pct: 0.0,
                    job_id: None,
                };
                let tx_move = tx.clone();
                let src_clone = src.clone();
                let dest_clone = dest.clone();
                tokio::spawn(async move {
                    let mkdir_res = run_rpc_job_async("operations/mkdir".to_string(), json!({
                        "fs": dest_clone,
                        "remote": "",
                    })).await;
                    let copy_res = if mkdir_res.is_err() {
                        mkdir_res
                    } else {
                        run_rpc_job_async_with_progress(
                            "sync/copy".to_string(),
                            json!({
                                "srcFs": src_clone,
                                "dstFs": dest_clone,
                            }),
                            Some((src_clone, dest_clone, true)),
                            Some(tx_move.clone()),
                        ).await
                    };
                    match copy_res {
                        Ok(_) => {
                            let del_res = run_rpc_job_async("operations/purge".to_string(), json!({
                                "fs": src,
                                "remote": "",
                            })).await;
                            let outcome = match del_res {
                                Ok(_) => Ok(()),
                                Err(e) => Err(e),
                            };
                            let _ = tx_move.send(AppEvent::ExplorerOperationFinished {
                                pane: ui::explorer::ActivePane::Left,
                                op_name: "Move Copy-Delete (Purge)".to_string(),
                                result: outcome,
                            });
                        }
                        Err(e) => {
                            let _ = tx_move.send(AppEvent::ExplorerOperationFinished {
                                pane: ui::explorer::ActivePane::Left,
                                op_name: "Move Copy-Delete (Copy failed)".to_string(),
                                result: Err(e),
                            });
                        }
                    }
                });
            }
            ui::explorer::FallbackAction::CopyNative { src, dest }
            | ui::explorer::FallbackAction::CopyLocalTransfer { src, dest } => {
                self.explorer_state.popup = ui::explorer::ExplorerPopup::CopyProgress {
                    src: src.clone(),
                    dest: dest.clone(),
                    pct: 0.0,
                    job_id: None,
                };
                let tx_copy = tx.clone();
                let src_clone = src.clone();
                let dest_clone = dest.clone();
                tokio::spawn(async move {
                    let res = run_rpc_job_async_with_progress(
                        "sync/copy".to_string(),
                        json!({
                            "srcFs": src_clone,
                            "dstFs": dest_clone,
                        }),
                        Some((src_clone, dest_clone, true)),
                        Some(tx_copy.clone()),
                    ).await;
                    let _ = tx_copy.send(AppEvent::ExplorerOperationFinished {
                        pane: ui::explorer::ActivePane::Left,
                        op_name: "sao chép (copy)".to_string(),
                        result: res,
                    });
                });
            }
            ui::explorer::FallbackAction::DeleteNative { target, is_dir } => {
                let pane_type = self.explorer_state.active_pane.clone();
                let tx_del = tx.clone();
                tokio::spawn(async move {
                    let op_name = if is_dir { "operations/purge" } else { "operations/deletefile" };
                    let param = if is_dir {
                        json!({ "fs": target })
                    } else {
                        if let Some(idx) = target.rfind('/') {
                            let parent = &target[..idx];
                            let file = &target[idx+1..];
                            json!({ "fs": parent, "remote": file })
                        } else {
                            json!({ "fs": target, "remote": "" })
                        }
                    };
                    let res = run_rpc_job_async(op_name.to_string(), param).await;
                    let _ = tx_del.send(AppEvent::ExplorerOperationFinished {
                        pane: pane_type,
                        op_name: "Xóa tệp/thư mục".to_string(),
                        result: res,
                    });
                });
            }
            ui::explorer::FallbackAction::DeleteIndividual { target } => {
                let pane_type = self.explorer_state.active_pane.clone();
                let tx_del = tx.clone();
                tokio::spawn(async move {
                    let res = run_rpc_job_async("operations/purge".to_string(), json!({ "fs": target, "remote": "" })).await;
                    let _ = tx_del.send(AppEvent::ExplorerOperationFinished {
                        pane: pane_type,
                        op_name: "Xóa tệp/thư mục (Dự phòng)".to_string(),
                        result: res,
                    });
                });
            }
            ui::explorer::FallbackAction::RenameCopyDelete { src, dest, is_dir } => {
                self.explorer_state.popup = ui::explorer::ExplorerPopup::MoveProgress {
                    src: src.clone(),
                    dest: dest.clone(),
                    pct: 0.0,
                    job_id: None,
                };
                let tx_move = tx.clone();
                let src_clone = src.clone();
                let dest_clone = dest.clone();
                tokio::spawn(async move {
                    let parse_path = |fs: &str| -> (String, String) {
                        let (remote_part, path_part) = if let Some(idx) = fs.find(':') {
                            (format!("{}:", &fs[..idx]), &fs[idx+1..])
                        } else {
                            (String::new(), fs)
                        };
                        if let Some(idx) = path_part.rfind('/') {
                            let parent = &path_part[..idx];
                            let name = &path_part[idx+1..];
                            (format!("{}{}", remote_part, parent), name.to_string())
                        } else {
                            (remote_part, path_part.to_string())
                        }
                    };

                    let copy_res = if is_dir {
                        let mkdir_res = run_rpc_job_async("operations/mkdir".to_string(), json!({
                            "fs": dest_clone,
                            "remote": "",
                        })).await;
                        if mkdir_res.is_err() {
                            mkdir_res
                        } else {
                            run_rpc_job_async_with_progress(
                                "sync/copy".to_string(),
                                json!({
                                    "srcFs": src_clone,
                                    "dstFs": dest_clone,
                                }),
                                Some((src_clone.clone(), dest_clone.clone(), true)),
                                Some(tx_move.clone()),
                            ).await
                        }
                    } else {
                        let (src_fs, src_file) = parse_path(&src_clone);
                        let (dst_fs, dst_file) = parse_path(&dest_clone);
                        run_rpc_job_async("operations/copyfile".to_string(), json!({
                            "srcFs": src_fs,
                            "srcRemote": src_file,
                            "dstFs": dst_fs,
                            "dstRemote": dst_file,
                        })).await
                    };

                    match copy_res {
                        Ok(_) => {
                            let del_res = if is_dir {
                                run_rpc_job_async("operations/purge".to_string(), json!({ "fs": src_clone, "remote": "" })).await
                            } else {
                                let (src_fs, src_file) = parse_path(&src_clone);
                                run_rpc_job_async("operations/deletefile".to_string(), json!({ "fs": src_fs, "remote": src_file })).await
                            };

                            let _ = tx_move.send(AppEvent::ExplorerOperationFinished {
                                pane: ui::explorer::ActivePane::Left,
                                op_name: "đổi tên dự phòng (copy+delete)".to_string(),
                                result: del_res,
                            });
                        }
                        Err(e) => {
                            let _ = tx_move.send(AppEvent::ExplorerOperationFinished {
                                pane: ui::explorer::ActivePane::Left,
                                op_name: "đổi tên dự phòng (copy+delete)".to_string(),
                                result: Err(e),
                            });
                        }
                    }
                });
            }
            ui::explorer::FallbackAction::RenameLocalTransfer { src, dest, is_dir } => {
                self.explorer_state.popup = ui::explorer::ExplorerPopup::MoveProgress {
                    src: src.clone(),
                    dest: dest.clone(),
                    pct: 0.0,
                    job_id: None,
                };
                let tx_move = tx.clone();
                let src_clone = src.clone();
                let dest_clone = dest.clone();
                tokio::spawn(async move {
                    let parse_path = |fs: &str| -> (String, String) {
                        let (remote_part, path_part) = if let Some(idx) = fs.find(':') {
                            (format!("{}:", &fs[..idx]), &fs[idx+1..])
                        } else {
                            (String::new(), fs)
                        };
                        if let Some(idx) = path_part.rfind('/') {
                            let parent = &path_part[..idx];
                            let name = &path_part[idx+1..];
                            (format!("{}{}", remote_part, parent), name.to_string())
                        } else {
                            (remote_part, path_part.to_string())
                        }
                    };

                    let res = if is_dir {
                        let mkdir_res = run_rpc_job_async("operations/mkdir".to_string(), json!({
                            "fs": dest_clone,
                            "remote": "",
                        })).await;
                        if mkdir_res.is_err() {
                            mkdir_res
                        } else {
                            let move_res = run_rpc_job_async_with_progress(
                                "sync/move".to_string(),
                                json!({
                                    "srcFs": src_clone,
                                    "dstFs": dest_clone,
                                }),
                                Some((src_clone.clone(), dest_clone.clone(), false)),
                                Some(tx_move.clone()),
                            ).await;
                            if move_res.is_err() {
                                move_res
                            } else {
                                run_rpc_job_async("operations/purge".to_string(), json!({
                                    "fs": src_clone,
                                    "remote": "",
                                })).await
                            }
                        }
                    } else {
                        let (src_fs, src_file) = parse_path(&src_clone);
                        let (dst_fs, dst_file) = parse_path(&dest_clone);
                        run_rpc_job_async("operations/movefile".to_string(), json!({
                            "srcFs": src_fs,
                            "srcRemote": src_file,
                            "dstFs": dst_fs,
                            "dstRemote": dst_file,
                        })).await
                    };

                    let _ = tx_move.send(AppEvent::ExplorerOperationFinished {
                        pane: ui::explorer::ActivePane::Left,
                        op_name: "đổi tên dự phòng (local transfer)".to_string(),
                        result: res,
                    });
                });
            }
            ui::explorer::FallbackAction::CleanupCloud { fs } => {
                let tx_clone = tx.clone();
                tokio::spawn(async move {
                    let res = run_rpc_job_async("operations/cleanup".to_string(), json!({ "fs": fs })).await;
                    let msg = match res {
                        Ok(_) => "Dọn rác hoàn tất thành công!".to_string(),
                        Err(e) => format!("Lỗi khi dọn rác: {}", e),
                    };
                    let _ = tx_clone.send(AppEvent::CryptdecodeResult { result: Ok(msg) });
                });
            }
            ui::explorer::FallbackAction::Rmdir { fs, remote } => {
                let tx_clone = tx.clone();
                tokio::spawn(async move {
                    let res = run_rpc_job_async("operations/rmdir".to_string(), json!({ "fs": fs, "remote": remote })).await;
                    let msg = match &res {
                        Ok(_) => "Xóa thư mục rỗng thành công!".to_string(),
                        Err(e) => format!("Lỗi khi xóa: {}", e),
                    };
                    let _ = tx_clone.send(AppEvent::ExplorerOperationFinished {
                        pane: ui::explorer::ActivePane::Left,
                        op_name: "xóa thư mục rỗng (rmdir)".to_string(),
                        result: res.clone(),
                    });
                    let _ = tx_clone.send(AppEvent::CryptdecodeResult { result: Ok(msg) });
                });
            }
            ui::explorer::FallbackAction::Rmdirs { fs, remote } => {
                let tx_clone = tx.clone();
                tokio::spawn(async move {
                    let res = run_rpc_job_async("operations/rmdirs".to_string(), json!({ "fs": fs, "remote": remote })).await;
                    let msg = match &res {
                        Ok(_) => "Xóa đệ quy các thư mục rỗng thành công!".to_string(),
                        Err(e) => format!("Lỗi khi xóa: {}", e),
                    };
                    let _ = tx_clone.send(AppEvent::ExplorerOperationFinished {
                        pane: ui::explorer::ActivePane::Left,
                        op_name: "xóa đệ quy thư mục rỗng (rmdirs)".to_string(),
                        result: res.clone(),
                    });
                    let _ = tx_clone.send(AppEvent::CryptdecodeResult { result: Ok(msg) });
                });
            }
            ui::explorer::FallbackAction::Cancel => {}
        }
    }

    /// Khởi chạy vòng lặp sự kiện chính của ứng dụng
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        // 1. Luồng Tick định kỳ 250ms
        let tx_tick = tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(250)).await;
                if tx_tick.send(AppEvent::Tick).is_err() {
                    break;
                }
            }
        });

        // 2. Luồng nạp phím từ Crossterm
        let tx_input = tx.clone();
        tokio::spawn(async move {
            loop {
                if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                    if let Ok(Event::Key(key)) = event::read() {
                        if key.kind != event::KeyEventKind::Release {
                            if tx_input.send(AppEvent::Input(key)).is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        });

        // Nạp biến môi trường cho tệp cấu hình (Config Profile)
        let active_profile = self.config.get_active_profile_path();
        let active_profile_dir = Path::new(&active_profile).parent().unwrap();
        let _ = fs::create_dir_all(active_profile_dir);
        if !Path::new(&active_profile).exists() {
            let _ = fs::write(&active_profile, "");
        }
        unsafe {
            std::env::set_var("RCLONE_CONFIG", &active_profile);
        }
        // Đồng bộ Go core
        let _ = rclone::rpc(
            "config/setpath",
            &json!({"path": active_profile}).to_string(),
        );

        // Tải các tiến trình chạy ngầm
        self.load_active_services_from_file();

        // 3. Khởi chạy luồng kiểm tra trạng thái các remote tuần hoàn/chạy ngầm
        let (status_tx, mut status_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        self.status_trigger_tx = Some(status_tx);

        let tx_status = tx.clone();
        tokio::spawn(async move {
            loop {
                // Fetch list of remotes
                let res =
                    rclone::rpc_async("config/listremotes".to_string(), "{}".to_string()).await;
                if let Ok(rpc_res) = res {
                    if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                        if let Some(arr) = val.get("remotes").and_then(|r| r.as_array()) {
                            let remotes: Vec<String> = arr
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect();

                            // Xây dựng dependency map ngay trong tác vụ ngầm từ config/dump
                            let mut local_dependencies = HashMap::new();
                            let dump_res = rclone::rpc_async("config/dump".to_string(), "{}".to_string()).await;
                            if let Ok(rpc_dump) = dump_res {
                                if let Ok(dump_val) = serde_json::from_str::<Value>(&rpc_dump.output) {
                                    if let Some(obj) = dump_val.as_object() {
                                        for (name, details) in obj {
                                            if let Some(details_obj) = details.as_object() {
                                                if let Some(r_type) = details_obj.get("type").and_then(|t| t.as_str()) {
                                                    if r_type == "crypt" || r_type == "alias" {
                                                        if let Some(base_remote_path) = details_obj.get("remote").and_then(|r| r.as_str()) {
                                                            let base_name = if let Some(idx) = base_remote_path.find(':') {
                                                                &base_remote_path[..idx]
                                                            } else {
                                                                base_remote_path
                                                            };
                                                            local_dependencies.insert(name.clone(), base_name.to_string());
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Phân phối kiểm tra theo các nhóm (pool) tối đa 10 remote
                            // Lọc bỏ các remote phụ thuộc (chúng sẽ tự động kế thừa trạng thái từ remote chính)
                            let independent_remotes: Vec<String> = remotes
                                .iter()
                                .filter(|r| !local_dependencies.contains_key(*r))
                                .cloned()
                                .collect();

                            for chunk in independent_remotes.chunks(10) {
                                let mut join_handles = Vec::new();
                                for remote in chunk {
                                    let tx_clone = tx_status.clone();
                                    let remote_clone = remote.clone();
                                    let handle = tokio::spawn(async move {
                                        let mut status = "🔴 Ngoại tuyến / Lỗi".to_string();

                                        // Thử lại tối đa 5 lần
                                        for attempt in 1..=5 {
                                            let param =
                                                json!({ "fs": format!("{}:", remote_clone) })
                                                    .to_string();

                                            // 1. Thử gọi operations/about trước để lấy dung lượng thực tế
                                            let about_future = rclone::rpc_async(
                                                "operations/about".to_string(),
                                                param.clone(),
                                            );
                                            let about_result = tokio::time::timeout(
                                                std::time::Duration::from_secs(5),
                                                about_future,
                                            )
                                            .await;

                                            match about_result {
                                                Ok(Ok(rpc_res)) => {
                                                    if rpc_res.status == 200 {
                                                        if let Ok(space_val) =
                                                            serde_json::from_str::<Value>(
                                                                &rpc_res.output,
                                                            )
                                                        {
                                                            if let (Some(total), Some(used)) = (
                                                                space_val.get("total"),
                                                                space_val.get("used"),
                                                            ) {
                                                                let total_bytes =
                                                                    total.as_u64().unwrap_or(0);
                                                                let used_bytes =
                                                                    used.as_u64().unwrap_or(0);
                                                                status = format!(
                                                                    "🟢 Trực tuyến (Đã dùng {} / {})",
                                                                    crate::ui::format_size(used_bytes),
                                                                    crate::ui::format_size(total_bytes)
                                                                );
                                                            } else {
                                                                status =
                                                                    "🟢 Trực tuyến".to_string();
                                                            }
                                                        } else {
                                                            status = "🟢 Trực tuyến".to_string();
                                                        }
                                                        break; // Thành công, thoát vòng lặp retry
                                                    } else {
                                                        // operations/about lỗi. Kiểm tra chi tiết lỗi
                                                        let err_msg =
                                                            serde_json::from_str::<Value>(
                                                                &rpc_res.output,
                                                            )
                                                            .ok()
                                                            .and_then(|v| {
                                                                v.get("error")
                                                                    .and_then(|e| e.as_str())
                                                                    .map(|s| s.to_string())
                                                            })
                                                            .unwrap_or_default();

                                                        let lower = err_msg.to_lowercase();

                                                        // Phán đoán không hỗ trợ about
                                                        let mut is_not_supported = lower
                                                            .contains("not supported")
                                                            || lower.contains("about")
                                                            || lower.contains("optional feature")
                                                            || lower.contains("unknown command");

                                                        // Dự phòng: gọi operations/fsinfo kiểm tra nếu error message không rõ ràng nhưng remote vẫn online
                                                        if !is_not_supported {
                                                            let fsinfo_future = rclone::rpc_async(
                                                                "operations/fsinfo".to_string(),
                                                                param.clone(),
                                                            );
                                                            if let Ok(Ok(fsinfo_res)) =
                                                                tokio::time::timeout(
                                                                    std::time::Duration::from_secs(
                                                                        5,
                                                                    ),
                                                                    fsinfo_future,
                                                                )
                                                                .await
                                                            {
                                                                if fsinfo_res.status == 200 {
                                                                    is_not_supported = true;
                                                                }
                                                            }
                                                        }

                                                        if is_not_supported {
                                                            let is_already_running = {
                                                                let mut checks = RUNNING_SIZE_CHECKS.lock().unwrap();
                                                                if checks.contains(&remote_clone) {
                                                                    true
                                                                } else {
                                                                    checks.insert(remote_clone.clone());
                                                                    false
                                                                }
                                                            };

                                                            if is_already_running {
                                                                status = "🟢 Trực tuyến (Đang tính dung lượng...)".to_string();
                                                                break;
                                                            }

                                                            // Remote online nhưng không hỗ trợ about. Cập nhật trạng thái tạm thời
                                                            let _ = tx_clone.send(AppEvent::RemoteStatusUpdate {
                                                                remote: remote_clone.clone(),
                                                                status: "🟢 Trực tuyến (Đang tính dung lượng...)".to_string(),
                                                            });

                                                            // Chạy đếm dung lượng ngầm (operations/size) hoàn toàn bất đồng bộ không chặn status checker
                                                            let tx_size = tx_clone.clone();
                                                            let remote_size = remote_clone.clone();
                                                            let param_size = param.clone();
                                                            tokio::spawn(async move {
                                                                struct SizeCheckGuard(String);
                                                                impl Drop for SizeCheckGuard {
                                                                    fn drop(&mut self) {
                                                                        RUNNING_SIZE_CHECKS.lock().unwrap().remove(&self.0);
                                                                    }
                                                                }
                                                                let _guard = SizeCheckGuard(remote_size.clone());

                                                                let size_future = rclone::rpc_async(
                                                                    "operations/size".to_string(),
                                                                    param_size,
                                                                );
                                                                // Không giới hạn thời gian chờ, đặt timeout 1 tiếng
                                                                match tokio::time::timeout(
                                                                    std::time::Duration::from_secs(3600),
                                                                    size_future,
                                                                )
                                                                .await
                                                                {
                                                                    Ok(Ok(size_rpc_res)) => {
                                                                        if size_rpc_res.status == 200 {
                                                                            if let Ok(size_val) =
                                                                                serde_json::from_str::<Value>(
                                                                                    &size_rpc_res.output,
                                                                                )
                                                                            {
                                                                                if let Some(used_bytes) = size_val
                                                                                    .get("bytes")
                                                                                    .and_then(|b| b.as_u64())
                                                                                {
                                                                                    let _ = tx_size.send(AppEvent::RemoteStatusUpdate {
                                                                                        remote: remote_size,
                                                                                        status: format!(
                                                                                            "🟢 Trực tuyến (Đã dùng {} / ∞)",
                                                                                            crate::ui::format_size(used_bytes)
                                                                                        ),
                                                                                    });
                                                                                    return;
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                    _ => {}
                                                                }
                                                                let _ = tx_size.send(AppEvent::RemoteStatusUpdate {
                                                                    remote: remote_size,
                                                                    status: "🟢 Trực tuyến (Không giới hạn)".to_string(),
                                                                });
                                                            });

                                                            status = "🟢 Trực tuyến (Đang tính dung lượng...)".to_string();
                                                            break; // Hoàn thành quét trạng thái của remote này
                                                        } else {
                                                            status = if !err_msg.is_empty() {
                                                                format!(
                                                                    "🔴 Lỗi kết nối: {}",
                                                                    err_msg
                                                                )
                                                            } else {
                                                                format!(
                                                                    "🔴 Lỗi kết nối (Mã lỗi: {})",
                                                                    rpc_res.status
                                                                )
                                                            };
                                                        }
                                                    }
                                                }
                                                Ok(Err(_)) => {
                                                    status = "🔴 Ngoại tuyến / Lỗi".to_string();
                                                }
                                                Err(_) => {
                                                    status = "🟡 Hết thời gian chờ".to_string();
                                                }
                                            }
                                            // Chờ 500ms trước khi thử lại để tránh spam
                                            if attempt < 5 {
                                                tokio::time::sleep(Duration::from_millis(500))
                                                    .await;
                                            }
                                        }

                                        let _ = tx_clone.send(AppEvent::RemoteStatusUpdate {
                                            remote: remote_clone,
                                            status,
                                        });
                                    });
                                    join_handles.push(handle);
                                }
                                // Đợi cả nhóm 10 remote hoàn tất
                                for handle in join_handles {
                                    let _ = handle.await;
                                }
                            }
                        }
                    }
                }

                // Đợi 60 giây hoặc cho đến khi nhận được tín hiệu kích hoạt thủ công
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(60)) => {}
                    msg = status_rx.recv() => {
                        if msg.is_none() {
                            break;
                        }
                    }
                }
            }
        });

        // Gọi load_remotes lần đầu để nạp danh sách và kích hoạt kiểm tra
        self.load_remotes(tx.clone()).await;

        while !self.should_exit {
            let active_profile_name = self.config.active_profile.clone();

            // Vẽ giao diện
            let is_fuse_installed =
                which::which("fusermount3").is_ok() || which::which("fusermount").is_ok();
            terminal.draw(|f| {
                let size = f.size();
                // Phân chia Grid: 3 dòng trên cho Header, phần còn lại cho Main Area
                let main_layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3), // Top Bar
                        Constraint::Min(10),   // Screen Content
                    ])
                    .split(size);

                let top_text = format!(
                    " === Rclone TUI Engine === [Profile: {}] [FUSE: {}] [VFS Cache: Bật]",
                    active_profile_name,
                    if is_fuse_installed {
                        "Đã cài đặt"
                    } else {
                        "Chưa cài đặt"
                    }
                );
                let top_paragraph = Paragraph::new(top_text)
                    .style(Style::default().add_modifier(Modifier::BOLD))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Cyan)),
                    );
                f.render_widget(top_paragraph, main_layout[0]);

                match self.screen {
                    Screen::MainMenu => ui::menu::draw(&self.menu_state, f, main_layout[1]),
                    Screen::ConnectionManager => {
                        ui::connection::draw(&self.connection_state, f, main_layout[1])
                    }
                    Screen::FileExplorer => {
                        ui::explorer::draw(&mut self.explorer_state, f, main_layout[1])
                    }
                    Screen::JobMonitor => ui::monitor::draw(&self.monitor_state, f, main_layout[1]),
                    Screen::ConfigProfileManager => ui::profile::draw(
                        &self.profile_state,
                        f,
                        main_layout[1],
                        &active_profile_name,
                    ),
                    Screen::ServicesAndMounts => {
                        ui::services::draw(&self.services_state, f, main_layout[1])
                    }
                    Screen::LanguageSelect => self.draw_language_select(f, main_layout[1]),
                }

                if let Some(ref target) = self.delete_confirm {
                    let (title, message) = match target {
                        DeleteTarget::Connection(name) => (
                            crate::lang::translate("confirm_delete_remote_title"),
                            crate::lang::translate("confirm_delete_remote").replace("{}", name),
                        ),
                        DeleteTarget::FileExplorer(name) => (
                            crate::lang::translate("confirm_delete_file_title"),
                            crate::lang::translate("confirm_delete_file").replace("{}", name),
                        ),
                        DeleteTarget::FileExplorerMultiple(names) => (
                            crate::lang::translate("confirm_delete_multiple_title"),
                            crate::lang::translate("confirm_delete_multiple").replace("{}", &names.len().to_string()),
                        ),
                        DeleteTarget::Service(idx) => {
                            let service_details = if *idx < self.services_state.active_services.len() {
                                &self.services_state.active_services[*idx].details
                            } else {
                                ""
                            };
                            (
                                crate::lang::translate("confirm_delete_service_title"),
                                crate::lang::translate("confirm_delete_service").replace("{}", service_details),
                            )
                        }
                        DeleteTarget::SystemdService(idx) => {
                            let name = if *idx < self.services_state.systemd_services.len() {
                                &self.services_state.systemd_services[*idx].name
                            } else {
                                ""
                            };
                            (
                                crate::lang::translate("confirm_delete_systemd_title"),
                                crate::lang::translate("confirm_delete_systemd").replace("{}", name),
                            )
                        }
                    };
                    ui::draw_popup(f, &title, &message, 60, 30);
                }
            })?;

            // Lấy sự kiện
            if let Some(event) = rx.recv().await {
                match event {
                    AppEvent::Input(key) => {
                        self.handle_key_event(key, tx.clone()).await;
                    }
                    AppEvent::Tick => {
                        self.handle_tick_event(tx.clone()).await;
                    }
                    AppEvent::ExplorerListResult { pane, result } => {
                        self.handle_explorer_list_result(pane, result);
                    }
                    AppEvent::WizardGuiListResult { result } => {
                        match self.services_state.wizard {
                            ui::services::ServicesWizardState::GuiSelectPath {
                                ref mut items,
                                ref mut loading,
                                ref mut error_msg,
                                ..
                            } | ui::services::ServicesWizardState::GuiSelectLocalPath {
                                ref mut items,
                                ref mut loading,
                                ref mut error_msg,
                                ..
                            } => {
                                *loading = false;
                                match result {
                                    Ok(res_items) => {
                                        *items = res_items;
                                        *error_msg = None;
                                    }
                                    Err(e) => {
                                        *items = Vec::new();
                                        *error_msg = Some(e);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    AppEvent::WizardGuiRefresh => {
                        self.refresh_wizard_gui_list(tx.clone()).await;
                    }
                    AppEvent::CopyProgress { src, dest, pct, job_id } => {
                        if let ui::explorer::ExplorerPopup::CopyProgress { .. } =
                            self.explorer_state.popup
                        {
                            if pct >= 100.0 {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                                self.refresh_explorer_pane(
                                    ui::explorer::ActivePane::Left,
                                    tx.clone(),
                                )
                                .await;
                                self.refresh_explorer_pane(
                                    ui::explorer::ActivePane::Right,
                                    tx.clone(),
                                )
                                .await;
                            } else {
                                self.explorer_state.popup =
                                    ui::explorer::ExplorerPopup::CopyProgress { src, dest, pct, job_id };
                            }
                        }
                    }
                    AppEvent::MoveProgress { src, dest, pct, job_id } => {
                        if let ui::explorer::ExplorerPopup::MoveProgress { .. } =
                            self.explorer_state.popup
                        {
                            if pct >= 100.0 {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                                self.refresh_explorer_pane(
                                    ui::explorer::ActivePane::Left,
                                    tx.clone(),
                                )
                                .await;
                                self.refresh_explorer_pane(
                                    ui::explorer::ActivePane::Right,
                                    tx.clone(),
                                )
                                .await;
                            } else {
                                self.explorer_state.popup =
                                    ui::explorer::ExplorerPopup::MoveProgress { src, dest, pct, job_id };
                            }
                        }
                    }
                    AppEvent::JobStatsUpdate {
                        speed,
                        transferred,
                        total,
                        active,
                    } => {
                        self.monitor_state.speed = speed;
                        self.monitor_state.bytes_transferred = transferred;
                        self.monitor_state.total_bytes = total;
                        self.monitor_state.active_jobs = active;
                        if self.monitor_state.selected_job_idx >= self.monitor_state.active_jobs.len() {
                            self.monitor_state.selected_job_idx = 0;
                        }
                        self.stats_scan_in_progress = false;
                    }
                    AppEvent::OAuthFinished { result } => {
                        if let ui::connection::WizardState::SimpleOAuthLoop {
                            selected_providers,
                            ..
                        } = &self.connection_state.wizard
                        {
                            match result {
                                Ok(_) => {
                                    self.connection_state.info_message =
                                        Some("Cấu hình kết nối thành công!".to_string());
                                    self.advance_connection_wizard(
                                        selected_providers.clone(),
                                        tx.clone(),
                                    )
                                    .await;
                                }
                                Err(e) => {
                                    self.connection_state.error_message =
                                        Some(format!("OAuth lỗi: {}", e));
                                    self.connection_state.wizard =
                                        ui::connection::WizardState::None;
                                }
                            }
                            self.load_remotes(tx.clone()).await;
                        }
                    }
                    AppEvent::OAuthUrlReceived { url } => {
                        if let ui::connection::WizardState::SimpleOAuthLoop {
                            provider,
                            remote_name,
                            selected_providers,
                            ..
                        } = &self.connection_state.wizard
                        {
                            self.connection_state.wizard = ui::connection::WizardState::SimpleOAuthLoop {
                                provider: provider.clone(),
                                remote_name: remote_name.clone(),
                                auth_url: url,
                                selected_providers: selected_providers.clone(),
                            };
                        }
                    }
                    AppEvent::ActiveServicesLoaded(services) => {
                        self.services_state.active_services = services;
                    }
                    AppEvent::RemoteStatusUpdate { remote, status } => {
                        self.connection_state.remote_statuses.insert(remote.clone(), status.clone());
                        let mut updates = Vec::new();
                        for (dep_name, base_name) in &self.remote_dependencies {
                            if base_name == &remote {
                                let inherited_status = status.replace(&format!("{}:", base_name), &format!("{}:", dep_name));
                                updates.push((dep_name.clone(), inherited_status));
                            }
                        }
                        for (dep, stat) in updates {
                            self.connection_state.remote_statuses.insert(dep, stat);
                        }
                    }
                    AppEvent::ExplorerOperationFinished { pane: _, op_name, result } => {
                        if matches!(
                            self.explorer_state.popup,
                            ui::explorer::ExplorerPopup::CopyProgress { .. }
                                | ui::explorer::ExplorerPopup::MoveProgress { .. }
                        ) {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                        }
                        match result {
                            Ok(_) => {
                                self.refresh_explorer_pane(ui::explorer::ActivePane::Left, tx.clone()).await;
                                self.refresh_explorer_pane(ui::explorer::ActivePane::Right, tx.clone()).await;
                            }
                            Err(e) => {
                                self.explorer_state.error_message = Some(format!("Lỗi khi thực hiện {}: {}", op_name, e));
                            }
                        }
                    }
                    AppEvent::FeaturesChecked {
                        action_type,
                        src,
                        dest,
                        src_features,
                        dst_features,
                        is_dir,
                    } => {
                        self.handle_features_checked(action_type, src, dest, src_features, dst_features, is_dir, tx.clone()).await;
                    }
                    AppEvent::FileViewLoaded { file_name, result } => {
                        match result {
                            Ok(lines) => {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::ViewFile {
                                    file_name,
                                    content: lines,
                                    scroll_offset: 0,
                                };
                            }
                            Err(e) => {
                                self.explorer_state.error_message = Some(format!("Lỗi đọc file: {}", e));
                            }
                        }
                    }
                    AppEvent::TuiSelectorListResult { result } => {
                        if let ui::explorer::ExplorerPopup::TuiExplorerSelector {
                            ref mut items,
                            ref mut loading,
                            ..
                        } = self.explorer_state.popup
                        {
                            *loading = false;
                            match result {
                                Ok(res_items) => {
                                    *items = res_items;
                                }
                                Err(e) => {
                                    *items = Vec::new();
                                    self.explorer_state.error_message = Some(format!("Lỗi tải thư mục: {}", e));
                                }
                            }
                        }
                    }
                    AppEvent::CryptdecodeFinished { result } => {
                        if let ui::explorer::ExplorerPopup::CryptdecodeForm {
                            remote_input,
                            encrypted_input,
                            is_remote_focused,
                            ..
                        } = &self.explorer_state.popup
                        {
                            let output = match result {
                                Ok(decrypted) => decrypted,
                                Err(e) => format!("Lỗi: {}", e),
                            };
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::CryptdecodeForm {
                                remote_input: remote_input.clone(),
                                encrypted_input: encrypted_input.clone(),
                                is_remote_focused: *is_remote_focused,
                                output_result: Some(output),
                            };
                        }
                    }
                    AppEvent::CryptdecodeResult { result } => {
                        match result {
                            Ok(msg) => {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::SpecialActionMessage {
                                    title: "Kết quả".to_string(),
                                    message: msg,
                                };
                            }
                            Err(e) => {
                                self.explorer_state.error_message = Some(e);
                            }
                        }
                    }
                }
            }
        }

        // Tắt Alternate Screen và Raw Mode khi thoát
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        // Giữ lại các FUSE mount và dịch vụ ngầm khi thoát (chỉ tắt khi người dùng chọn Delete)
        // self.kill_all_active_services();

        Ok(())
    }

    /// Định kỳ cập nhật thông tin Stats và trạng thái các Service chạy ngầm
    async fn handle_tick_event(&mut self, tx: tokio::sync::mpsc::UnboundedSender<AppEvent>) {
        if self.screen == Screen::ServicesAndMounts {
            // Throttle: chỉ quét tối đa mỗi 4 giây
            if self.last_services_scan.elapsed() >= std::time::Duration::from_secs(4) {
                self.scan_running_services();
                self.scan_systemd_services();
                self.last_services_scan = std::time::Instant::now();
            }
        }
        if self.screen == Screen::JobMonitor {
            if self.stats_scan_in_progress {
                return;
            }
            // Throttle: chỉ cập nhật stats tối đa mỗi 1.5 giây
            if self.last_stats_scan.elapsed() < std::time::Duration::from_millis(1500) {
                return;
            }
            self.last_stats_scan = std::time::Instant::now();
            self.stats_scan_in_progress = true;

            let tx_clone = tx.clone();
            tokio::spawn(async move {
                // Lấy Stats từ core Rclone RPC
                let res = rclone::rpc_async("core/stats".to_string(), "{}".to_string()).await;
                
                // Lấy danh sách Job ID
                let list_res = rclone::rpc_async("job/list".to_string(), "{}".to_string()).await;
                
                let mut active = Vec::new();
                let mut speed = 0.0;
                let mut transferred = 0;
                let mut total = 0;

                if let Ok(rpc_res) = res {
                    if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                        speed = val.get("speed").and_then(|s| s.as_f64()).unwrap_or(0.0);
                        transferred = val.get("bytes").and_then(|b| b.as_u64()).unwrap_or(0);
                        total = val.get("totalBytes").and_then(|t| t.as_u64()).unwrap_or(0);

                        if let Some(transfers) = val.get("transferring").and_then(|t| t.as_array()) {
                            for t_val in transfers {
                                let name = t_val
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let size = t_val.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                                let bytes = t_val.get("bytes").and_then(|b| b.as_u64()).unwrap_or(0);
                                let speed_t = t_val.get("speed").and_then(|s| s.as_u64()).unwrap_or(0);
                                let percentage = t_val
                                    .get("percentage")
                                    .and_then(|p| p.as_u64())
                                    .unwrap_or(0) as u16;
                                let eta = t_val.get("eta").and_then(|e| e.as_i64()).unwrap_or(-1);

                                active.push(ui::monitor::TransferJob {
                                    name,
                                    size,
                                    bytes,
                                    speed: speed_t,
                                    percentage,
                                    eta,
                                    job_id: None,
                                    start_time: String::new(),
                                    duration: 0.0,
                                    group: String::new(),
                                    description: String::new(),
                                });
                            }
                        }
                    }
                }

                // Thêm các background jobs vào danh sách active (chỉ duyệt qua các job đang thực sự chạy)
                if let Ok(list_rpc) = list_res {
                    if let Ok(list_val) = serde_json::from_str::<Value>(&list_rpc.output) {
                        let ids_to_check = if let Some(r_ids) = list_val.get("runningIds").and_then(|j| j.as_array()) {
                            r_ids.clone()
                        } else if let Some(job_ids) = list_val.get("jobids").and_then(|j| j.as_array()) {
                            job_ids.clone()
                        } else {
                            Vec::new()
                        };

                        for id_val in ids_to_check {
                            if let Some(id) = id_val.as_i64() {
                                // Lấy thông tin chi tiết từng Job
                                let status_res = rclone::rpc_async(
                                    "job/status".to_string(),
                                    json!({ "jobid": id }).to_string(),
                                )
                                .await;
                                if let Ok(sr) = status_res {
                                    if let Ok(sval) = serde_json::from_str::<Value>(&sr.output) {
                                        let finished = sval.get("finished").and_then(|f| f.as_bool()).unwrap_or(false);
                                        if !finished {
                                            let desc_opt = get_job_description(id);
                                            let desc = desc_opt.as_deref().unwrap_or_else(|| {
                                                sval.get("description").and_then(|d| d.as_str()).unwrap_or("Tác vụ nền")
                                            });
                                            let duration = sval.get("duration").and_then(|d| d.as_f64()).unwrap_or(0.0);
                                            
                                            // Lấy stats cho group tương ứng "job/<id>"
                                            let mut speed_job = 0;
                                            let mut bytes_job = 0;
                                            let mut size_job = 0;
                                            let mut pct_job = 0;
                                            let mut eta_job = -1;

                                            let group_stats_res = rclone::rpc_async(
                                                "core/stats".to_string(),
                                                json!({ "group": format!("job/{}", id) }).to_string(),
                                            )
                                            .await;
                                            if let Ok(st_res) = group_stats_res {
                                                if let Ok(st_val) = serde_json::from_str::<Value>(&st_res.output) {
                                                    speed_job = st_val.get("speed").and_then(|s| s.as_u64()).unwrap_or(0);
                                                    bytes_job = st_val.get("bytes").and_then(|b| b.as_u64()).unwrap_or(0);
                                                    size_job = st_val.get("totalBytes").and_then(|b| b.as_u64()).unwrap_or(0);
                                                    
                                                    // Cộng vào global stats để hiển thị tổng quan đúng
                                                    speed += speed_job as f64;
                                                    transferred += bytes_job;
                                                    total += size_job;

                                                    if size_job > 0 {
                                                        pct_job = ((bytes_job as f64 / size_job as f64) * 100.0) as u16;
                                                    }
                                                    eta_job = st_val.get("eta").and_then(|e| e.as_i64()).unwrap_or(-1);

                                                    // Thêm các file đang truyền của job này vào danh sách active
                                                    if let Some(transfers) = st_val.get("transferring").and_then(|t| t.as_array()) {
                                                        for t_val in transfers {
                                                            let name = t_val.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                                                            let size = t_val.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                                                            let bytes = t_val.get("bytes").and_then(|b| b.as_u64()).unwrap_or(0);
                                                            let speed_t = t_val.get("speed").and_then(|s| s.as_u64()).unwrap_or(0);
                                                            let percentage = t_val.get("percentage").and_then(|p| p.as_u64()).unwrap_or(0) as u16;
                                                            let eta = t_val.get("eta").and_then(|e| e.as_i64()).unwrap_or(-1);

                                                            active.push(ui::monitor::TransferJob {
                                                                name: format!("[Job {}] {}", id, name),
                                                                size,
                                                                bytes,
                                                                speed: speed_t,
                                                                percentage,
                                                                eta,
                                                                job_id: None,
                                                                start_time: String::new(),
                                                                duration: 0.0,
                                                                group: String::new(),
                                                                description: String::new(),
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                            
                                            // Thêm vào danh sách active
                                            active.push(ui::monitor::TransferJob {
                                                name: format!("[Job {}] {} (Chạy {:.1}s)", id, desc, duration),
                                                size: size_job,
                                                bytes: bytes_job,
                                                speed: speed_job,
                                                percentage: pct_job,
                                                eta: eta_job,
                                                job_id: Some(id),
                                                start_time: sval.get("startTime").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                                                duration,
                                                group: sval.get("group").and_then(|g| g.as_str()).unwrap_or("").to_string(),
                                                description: desc.to_string(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                let _ = tx_clone.send(AppEvent::JobStatsUpdate {
                    speed,
                    transferred,
                    total,
                    active,
                });
            });
        }
    }

    /// Diệt sạch các tiến trình dịch vụ ngầm khi đóng app (Bug 54, 100)
    #[allow(dead_code)]
    fn kill_all_active_services(&mut self) {
        for s in &self.services_state.active_services {
            // Gửi tín hiệu kill PID
            #[cfg(unix)]
            {
                let _ = Command::new("kill").arg(s.pid.to_string()).status();
                // Nếu là mount, cố unmount point cưỡng chế (Bug 95)
                if s.service_type_str == "Mount" {
                    let _ = Command::new("fusermount").args(["-uz", &s.path]).status();
                }
            }
            #[cfg(not(unix))]
            {
                let _ = Command::new("taskkill").args(["/F", "/PID", &s.pid.to_string()]).status();
            }
        }
        self.services_state.active_services.clear();
        self.save_active_services_to_file();
    }

    /// Từng bước thiết lập các cờ trong connection Wizard
    async fn advance_connection_wizard(
        &mut self,
        mut remaining_providers: Vec<String>,
        _tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        if remaining_providers.is_empty() {
            self.connection_state.wizard = ui::connection::WizardState::None;
            return;
        }

        let provider = remaining_providers.remove(0);
        self.connection_state.wizard = ui::connection::WizardState::InputRemoteName {
            provider,
            input_buffer: String::new(),
            selected_providers: remaining_providers,
        };
    }

    /// Xử lý phím bấm phân loại theo từng Screen
    async fn handle_key_event(
        &mut self,
        key: KeyEvent,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        if let Some(target) = self.delete_confirm.clone() {
            match key.code {
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.delete_confirm = None;
                }
                KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.delete_confirm = None;
                    match target {
                        DeleteTarget::Connection(remote_name) => {
                            let param = serde_json::json!({"name": remote_name}).to_string();
                            let _ = rclone::rpc("config/delete", &param);
                            self.load_remotes(tx.clone()).await;
                        }
                        DeleteTarget::FileExplorer(item_name) => {
                            let pane = self.explorer_state.get_active_pane();
                            let is_dir = pane.items.iter()
                                .find(|item| item.name == item_name)
                                .map(|item| item.is_dir)
                                .unwrap_or(false);

                            let remote = pane.remote.clone();
                            let pane_path = pane.path.clone();
                            let pane_type = self.explorer_state.active_pane.clone();
                            let tx_op = tx.clone();

                            tokio::spawn(async move {
                                let (op_name, method, param) = if is_dir {
                                    let target = if remote.is_empty() {
                                        std::path::PathBuf::from(&pane_path)
                                            .join(&item_name)
                                            .to_string_lossy()
                                            .to_string()
                                    } else {
                                        let clean_remote = remote.trim_end_matches(':');
                                        let clean_path = if pane_path.starts_with('/') {
                                            pane_path.clone()
                                        } else {
                                            format!("/{}", pane_path)
                                        };
                                        if clean_path.ends_with('/') {
                                            format!("{}:{}{}", clean_remote, clean_path, item_name)
                                        } else {
                                            format!("{}:{}/{}", clean_remote, clean_path, item_name)
                                        }
                                    };
                                    (
                                        "xóa thư mục (purge)".to_string(),
                                        "operations/purge".to_string(),
                                        serde_json::json!({
                                            "fs": target,
                                            "remote": "",
                                        })
                                    )
                                } else {
                                    let fs = if remote.is_empty() {
                                        pane_path
                                    } else {
                                        let clean_remote = remote.trim_end_matches(':');
                                        if pane_path.is_empty() {
                                            format!("{}:", clean_remote)
                                        } else {
                                            format!("{}:{}", clean_remote, pane_path)
                                        }
                                    };
                                    (
                                        "xóa tệp (deletefile)".to_string(),
                                        "operations/deletefile".to_string(),
                                        serde_json::json!({
                                            "fs": fs,
                                            "remote": item_name,
                                        })
                                    )
                                };

                                let op_res = rclone::rpc_async(method, param.to_string()).await;
                                let res = match op_res {
                                    Ok(r) if r.status == 200 => Ok(()),
                                    Ok(r) => Err(format!("Mã lỗi: {}", r.status)),
                                    Err(e) => Err(e),
                                };
                                let _ = tx_op.send(AppEvent::ExplorerOperationFinished {
                                    pane: pane_type,
                                    op_name,
                                    result: res,
                                });
                            });
                        }
                        DeleteTarget::FileExplorerMultiple(item_names) => {
                            let pane_type = self.explorer_state.active_pane.clone();
                            let pane = self.explorer_state.get_active_pane_mut();
                            pane.selected_names.clear();
                            pane.shift_anchor = None;
                            pane.shift_active = false;
                            pane.alt_anchor = None;
                            pane.alt_active = false;
                            let remote = pane.remote.clone();
                            let pane_path = pane.path.clone();
                            let tx_op = tx.clone();
                            
                            let items_with_type: Vec<(String, bool)> = item_names
                                .into_iter()
                                .map(|name| {
                                    let is_dir = pane.items.iter()
                                        .find(|item| item.name == name)
                                        .map(|item| item.is_dir)
                                        .unwrap_or(false);
                                    (name, is_dir)
                                })
                                .collect();

                            tokio::spawn(async move {
                                let mut last_err = None;
                                for (item_name, is_dir) in items_with_type {
                                    let (method, param) = if is_dir {
                                        let target = if remote.is_empty() {
                                            std::path::PathBuf::from(&pane_path)
                                                .join(&item_name)
                                                .to_string_lossy()
                                                .to_string()
                                        } else {
                                            let clean_remote = remote.trim_end_matches(':');
                                            let clean_path = if pane_path.starts_with('/') {
                                                pane_path.clone()
                                            } else {
                                                format!("/{}", pane_path)
                                            };
                                            if clean_path.ends_with('/') {
                                                format!("{}:{}{}", clean_remote, clean_path, item_name)
                                            } else {
                                                format!("{}:{}/{}", clean_remote, clean_path, item_name)
                                            }
                                        };
                                        (
                                            "operations/purge".to_string(),
                                            serde_json::json!({
                                                "fs": target,
                                                "remote": "",
                                            })
                                        )
                                    } else {
                                        let fs = if remote.is_empty() {
                                            pane_path.clone()
                                        } else {
                                            let clean_remote = remote.trim_end_matches(':');
                                            if pane_path.is_empty() {
                                                format!("{}:", clean_remote)
                                            } else {
                                                format!("{}:{}", clean_remote, pane_path)
                                            }
                                        };
                                        (
                                            "operations/deletefile".to_string(),
                                            serde_json::json!({
                                                "fs": fs,
                                                "remote": item_name,
                                            })
                                        )
                                    };

                                    let op_res = rclone::rpc_async(method, param.to_string()).await;
                                    match op_res {
                                        Ok(r) if r.status == 200 => {}
                                        Ok(r) => last_err = Some(format!("Mã lỗi: {}", r.status)),
                                        Err(e) => last_err = Some(e),
                                    }
                                }
                                
                                let res = match last_err {
                                    None => Ok(()),
                                    Some(e) => Err(e),
                                };
                                let _ = tx_op.send(AppEvent::ExplorerOperationFinished {
                                    pane: pane_type,
                                    op_name: "xóa nhiều mục".to_string(),
                                    result: res,
                                });
                            });
                        }
                        DeleteTarget::Service(idx) => {
                            if idx < self.services_state.active_services.len() {
                                let service = &self.services_state.active_services[idx];
                                // Diệt tiến trình
                                #[cfg(unix)]
                                {
                                    let _ = Command::new("kill").arg(service.pid.to_string()).status();
                                    if service.service_type_str == "Mount" {
                                        let _ = Command::new("fusermount")
                                            .args(["-uz", &service.path])
                                            .status();
                                    }
                                }
                                #[cfg(not(unix))]
                                {
                                    let _ = Command::new("taskkill")
                                        .args(["/F", "/PID", &service.pid.to_string()])
                                        .status();
                                }
                                std::thread::sleep(std::time::Duration::from_millis(100));
                                self.scan_running_services();
                                self.services_state.selected_active_idx = 0;
                            }
                        }
                        DeleteTarget::SystemdService(idx) => {
                            if idx < self.services_state.systemd_services.len() {
                                let service = self.services_state.systemd_services[idx].clone();
                                
                                // Stop và disable
                                if service.is_user {
                                    let _ = Command::new("systemctl")
                                        .args(["--user", "stop", &service.name])
                                        .status();
                                    let _ = Command::new("systemctl")
                                        .args(["--user", "disable", &service.name])
                                        .status();
                                } else {
                                    let _ = Command::new("pkexec")
                                        .args(["systemctl", "stop", &service.name])
                                        .status();
                                    let _ = Command::new("pkexec")
                                        .args(["systemctl", "disable", &service.name])
                                        .status();
                                }

                                // Xóa file cấu hình dịch vụ
                                let res = if service.is_user {
                                    std::fs::remove_file(&service.file_path)
                                } else {
                                    Command::new("pkexec")
                                        .args(["rm", "-f", &service.file_path])
                                        .status()
                                        .map(|_| ())
                                        .map_err(|e| e)
                                };

                                // Reload daemon
                                if service.is_user {
                                    let _ = Command::new("systemctl")
                                        .args(["--user", "daemon-reload"])
                                        .status();
                                } else {
                                    let _ = Command::new("pkexec")
                                        .args(["systemctl", "daemon-reload"])
                                        .status();
                                }

                                match res {
                                    Ok(_) => {
                                        self.services_state.info_message = Some(format!(
                                            "Đã dừng, tắt và xóa thành công dịch vụ '{}'!",
                                            service.name
                                        ));
                                    }
                                    Err(e) => {
                                        self.services_state.error_message = Some(format!(
                                            "Lỗi khi xóa tệp dịch vụ: {}", e
                                        ));
                                    }
                                }

                                self.scan_systemd_services();
                                self.services_state.selected_systemd_idx = 0;
                            }
                        }
                    }
                }
                _ => {}
            }
            return;
        }

        // ESC hủy popups thông báo chung
        if key.code == KeyCode::Esc {
            if self.connection_state.error_message.is_some()
                || self.connection_state.info_message.is_some()
            {
                self.connection_state.error_message = None;
                self.connection_state.info_message = None;
                return;
            }
            if self.explorer_state.error_message.is_some() {
                self.explorer_state.error_message = None;
                return;
            }
            if self.profile_state.error_message.is_some() {
                self.profile_state.error_message = None;
                return;
            }
            if self.services_state.error_message.is_some()
                || self.services_state.info_message.is_some()
            {
                self.services_state.error_message = None;
                self.services_state.info_message = None;
                return;
            }
        }

        match self.screen {
            Screen::MainMenu => self.handle_menu_key(key, tx).await,
            Screen::ConnectionManager => self.handle_connection_key(key, tx).await,
            Screen::FileExplorer => self.handle_explorer_key(key, tx).await,
            Screen::JobMonitor => self.handle_monitor_key(key, tx).await,
            Screen::ConfigProfileManager => self.handle_profile_key(key, tx).await,
            Screen::ServicesAndMounts => self.handle_services_key(key, tx).await,
            Screen::LanguageSelect => self.handle_language_key(key).await,
        }
    }

    async fn handle_menu_key(
        &mut self,
        key: KeyEvent,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        match key.code {
            KeyCode::Up => self.menu_state.prev(),
            KeyCode::Down => self.menu_state.next(),
            KeyCode::Enter => match self.menu_state.selected_idx {
                0 => {
                    self.screen = Screen::ConnectionManager;
                    self.load_remotes(tx.clone()).await;
                }
                1 => {
                    self.screen = Screen::FileExplorer;
                    // Giữ nguyên trạng thái explorer nếu đã có dữ liệu
                    if self.explorer_state.left_pane.items.is_empty() && self.explorer_state.right_pane.items.is_empty() {
                        // Chỉ khởi tạo lại nếu chưa từng mở File Explorer
                        self.explorer_state = ui::explorer::ExplorerState::new();
                    }
                    self.load_remotes(tx.clone()).await;
                    self.refresh_explorer_pane(ui::explorer::ActivePane::Left, tx.clone())
                        .await;
                    self.refresh_explorer_pane(ui::explorer::ActivePane::Right, tx.clone())
                        .await;
                }
                2 => {
                    self.screen = Screen::JobMonitor;
                    // Kích hoạt quét tức thời khi chuyển sang màn hình Job Monitor
                    self.last_stats_scan = std::time::Instant::now() - std::time::Duration::from_secs(10);
                }
                3 => {
                    self.screen = Screen::ConfigProfileManager;
                    self.load_profile_list();
                }
                4 => {
                    self.screen = Screen::ServicesAndMounts;
                    self.load_remotes(tx.clone()).await;
                    // Kích hoạt quét tức thời khi chuyển sang màn hình dịch vụ
                    self.scan_running_services();
                    self.scan_systemd_services();
                    self.last_services_scan = std::time::Instant::now();
                }
                5 => {
                    self.screen = Screen::LanguageSelect;
                    self.available_languages = crate::lang::get_available_languages();
                    self.selected_lang_idx = self
                        .available_languages
                        .iter()
                        .position(|l| l == &self.config.active_language)
                        .unwrap_or(0);
                }
                6 => {
                    self.should_exit = true;
                }
                _ => {}
            },
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.should_exit = true;
            }
            _ => {}
        }
    }
    async fn handle_language_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                if !self.available_languages.is_empty() {
                    if self.selected_lang_idx == 0 {
                        self.selected_lang_idx = self.available_languages.len() - 1;
                    } else {
                        self.selected_lang_idx -= 1;
                    }
                }
            }
            KeyCode::Down => {
                if !self.available_languages.is_empty() {
                    self.selected_lang_idx =
                        (self.selected_lang_idx + 1) % self.available_languages.len();
                }
            }
            KeyCode::Enter => {
                if let Some(lang) = self
                    .available_languages
                    .get(self.selected_lang_idx)
                    .cloned()
                {
                    self.config.active_language = lang.clone();
                    let _ = self.config.save();
                    crate::lang::load_translation(&lang);
                }
                self.screen = Screen::MainMenu;
            }
            KeyCode::Esc => {
                self.screen = Screen::MainMenu;
            }
            _ => {}
        }
    }

    fn draw_language_select(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        use ratatui::widgets::{List, ListItem};
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Hướng dẫn
                Constraint::Min(5),    // Danh sách
            ])
            .split(area);

        let welcome = Paragraph::new(crate::lang::translate("lang_welcome"))
            .style(Style::default())
            .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));
        f.render_widget(welcome, chunks[0]);

        let items: Vec<ListItem> = self
            .available_languages
            .iter()
            .enumerate()
            .map(|(i, lang)| {
                let is_active = lang == &self.config.active_language;
                let text = if is_active {
                    format!("* {} ({})", lang, crate::lang::translate("lang_active"))
                } else {
                    format!("  {}", lang)
                };

                let style = if i == self.selected_lang_idx {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(text).style(style)
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .title(crate::lang::translate("lang_title"))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );
        f.render_widget(list, chunks[1]);
    }

    async fn handle_connection_key(
        &mut self,
        key: KeyEvent,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {


        let wizard = self.connection_state.wizard.clone();
        match wizard {
            ui::connection::WizardState::None => {
                match key.code {
                    KeyCode::Esc => {
                        self.screen = Screen::MainMenu;
                    }
                    KeyCode::Up => self.connection_state.prev(),
                    KeyCode::Down => self.connection_state.next(),
                    KeyCode::Insert => {
                        // Thêm kết nối mới: Bước 1 load providers
                        let res = rclone::rpc("config/providers", "{}");
                        if let Ok(rpc_res) = res {
                            if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                                if let Some(prov_arr) =
                                    val.get("providers").and_then(|p| p.as_array())
                                {
                                    let mut providers = Vec::new();
                                    for p_val in prov_arr {
                                        let name = p_val
                                            .get("Name")
                                            .and_then(|n| n.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let desc = p_val
                                            .get("Description")
                                            .and_then(|d| d.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        providers.push((name, desc, false));
                                    }
                                    providers.sort_by(|a, b| a.0.cmp(&b.0));
                                    self.connection_state.wizard =
                                        ui::connection::WizardState::SelectProviders {
                                            providers,
                                            selected_idx: 0,
                                            scroll_offset: 0,
                                        };
                                }
                            }
                        }
                    }
                    KeyCode::Char('e') | KeyCode::Char('E') if key.modifiers.contains(KeyModifiers::ALT) || (cfg!(target_os = "macos") && key.modifiers.contains(KeyModifiers::CONTROL)) => {
                        // Chỉnh sửa kết nối
                        if !self.connection_state.remotes.is_empty() {
                            let selected_remote = self.connection_state.remotes
                                [self.connection_state.selected_idx]
                                .clone();
                            let param = json!({"name": selected_remote}).to_string();
                            let res = rclone::rpc("config/get", &param);
                            if let Ok(rpc_res) = res {
                                if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                                    if let Some(current_config) = val.as_object() {
                                        let provider = current_config
                                            .get("type")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();

                                        // Truy vấn tất cả các options được hỗ trợ bởi provider này
                                        let mut fields = Vec::new();
                                        let prov_res = rclone::rpc("config/providers", "{}");
                                        if let Ok(prov_rpc_res) = prov_res {
                                            if let Ok(prov_val) =
                                                serde_json::from_str::<Value>(&prov_rpc_res.output)
                                            {
                                                if let Some(prov_arr) = prov_val
                                                    .get("providers")
                                                    .and_then(|p| p.as_array())
                                                {
                                                    // Tìm provider trùng khớp
                                                    if let Some(prov_obj) =
                                                        prov_arr.iter().find(|p| {
                                                            p.get("Name").and_then(|n| n.as_str())
                                                                == Some(&provider)
                                                        })
                                                    {
                                                        if let Some(opts_arr) = prov_obj
                                                            .get("Options")
                                                            .and_then(|o| o.as_array())
                                                        {
                                                            for opt_val in opts_arr {
                                                                let opt_name = opt_val
                                                                    .get("Name")
                                                                    .and_then(|n| n.as_str())
                                                                    .unwrap_or("")
                                                                    .to_string();
                                                                let opt_help = opt_val
                                                                    .get("Help")
                                                                    .and_then(|h| h.as_str())
                                                                    .unwrap_or("")
                                                                    .to_string();

                                                                // Lấy giá trị cấu hình hiện có của remote (nếu đã có)
                                                                let current_val = current_config
                                                                    .get(&opt_name)
                                                                    .map(|v| match v {
                                                                        Value::String(s) => {
                                                                            s.clone()
                                                                        }
                                                                        Value::Number(num) => {
                                                                            num.to_string()
                                                                        }
                                                                        Value::Bool(b) => {
                                                                            b.to_string()
                                                                        }
                                                                        _ => v.to_string(),
                                                                    })
                                                                    .unwrap_or_default();

                                                                // Loại bỏ trường "type" vì type là cố định của remote
                                                                if opt_name != "type" {
                                                                    let opt_type = opt_val
                                                                        .get("Type")
                                                                        .and_then(|t| t.as_str())
                                                                        .unwrap_or("");
                                                                    let mut choices = Vec::new();
                                                                    if opt_type == "bool" {
                                                                        choices.push(
                                                                            "true".to_string(),
                                                                        );
                                                                        choices.push(
                                                                            "false".to_string(),
                                                                        );
                                                                    }
                                                                    if let Some(examples_arr) =
                                                                        opt_val
                                                                            .get("Examples")
                                                                            .and_then(|e| {
                                                                                e.as_array()
                                                                            })
                                                                    {
                                                                        for ex in examples_arr {
                                                                            if let Some(val) = ex
                                                                                .get("Value")
                                                                                .and_then(|v| {
                                                                                    v.as_str()
                                                                                })
                                                                            {
                                                                                choices.push(
                                                                                    val.to_string(),
                                                                                );
                                                                            }
                                                                        }
                                                                    }
                                                                    let mut unique_choices =
                                                                        Vec::new();
                                                                    for c in choices {
                                                                        if !unique_choices
                                                                            .contains(&c)
                                                                        {
                                                                            unique_choices.push(c);
                                                                        }
                                                                    }
                                                                    let choices = unique_choices;
                                                                    fields.push((
                                                                        opt_name,
                                                                        opt_help,
                                                                        current_val,
                                                                        choices,
                                                                    ));
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // Dự phòng trong trường hợp không truy vấn được config/providers
                                        if fields.is_empty() {
                                            for (k, v) in current_config {
                                                if k != "type" {
                                                    let val_str = match v {
                                                        Value::String(s) => s.clone(),
                                                        Value::Number(num) => num.to_string(),
                                                        Value::Bool(b) => b.to_string(),
                                                        _ => v.to_string(),
                                                    };
                                                    fields.push((
                                                        k.clone(),
                                                        k.clone(),
                                                        val_str,
                                                        Vec::new(),
                                                    ));
                                                }
                                            }
                                        }

                                        // Sắp xếp: đưa các tham số có giá trị lên đầu, các tham số chưa cấu hình xuống dưới
                                        fields.sort_by(|a, b| {
                                            let a_has = !a.2.is_empty();
                                            let b_has = !b.2.is_empty();
                                            b_has.cmp(&a_has).then_with(|| a.0.cmp(&b.0))
                                        });

                                        fields.insert(0, (
                                            "_remote_name".to_string(),
                                            "Tên của remote / Name of the remote".to_string(),
                                            selected_remote.clone(),
                                            Vec::new(),
                                        ));

                                        self.connection_state.wizard =
                                            ui::connection::WizardState::EditSetup {
                                                remote_name: selected_remote,
                                                provider,
                                                fields,
                                                selected_idx: 0,
                                                scroll_offset: 0,
                                                is_editing: false,
                                                input_buffer: String::new(),
                                                adding_new_key: false,
                                                new_key_buffer: String::new(),
                                                active_tab: 0,
                                            };
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char('?') => {
                        if !self.connection_state.remotes.is_empty() {
                            let selected_remote = self.connection_state.remotes
                                [self.connection_state.selected_idx]
                                .clone();

                            // 1. Kiểm tra cấu hình xem có phải remote dạng union không
                            let param = json!({"name": selected_remote}).to_string();
                            let mut is_union = false;
                            let mut upstreams = Vec::new();
                            if let Ok(rpc_res) = rclone::rpc_async("config/get".to_string(), param).await {
                                if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                                    if let Some(cfg_obj) = val.as_object() {
                                        if cfg_obj.get("type").and_then(|v| v.as_str()) == Some("union") {
                                            is_union = true;
                                            if let Some(upstreams_str) = cfg_obj.get("upstreams").and_then(|v| v.as_str()) {
                                                for u in upstreams_str.split(|c| c == ' ' || c == ',') {
                                                    let u = u.trim();
                                                    if !u.is_empty() {
                                                        let r_name = match u.find(':') {
                                                            Some(idx) => &u[..idx],
                                                            None => u,
                                                        };
                                                        if !r_name.is_empty() && !upstreams.contains(&r_name.to_string()) {
                                                            upstreams.push(r_name.to_string());
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // 2. Truy vấn tính năng của remote (và các remote thành viên nếu là union)
                            let mut remotes_to_check = vec![selected_remote.clone()];
                            if is_union {
                                for u in &upstreams {
                                    if !remotes_to_check.contains(u) {
                                        remotes_to_check.push(u.clone());
                                    }
                                }
                            }

                            let mut remote_features = Vec::new();
                            for r in remotes_to_check {
                                let param = json!({ "fs": format!("{}:", r) }).to_string();
                                if let Ok(res) = rclone::rpc_async("operations/fsinfo".to_string(), param).await {
                                    if res.status == 200 {
                                        if let Ok(val) = serde_json::from_str::<Value>(&res.output) {
                                            if let Some(feats) = val.get("Features").and_then(|f| f.as_object()) {
                                                let mut feat_list = Vec::new();
                                                for (k, v) in feats {
                                                    if let Some(b) = v.as_bool() {
                                                        feat_list.push((k.clone(), b));
                                                    }
                                                }
                                                feat_list.sort_by(|a, b| a.0.cmp(&b.0));
                                                remote_features.push((r, feat_list));
                                            }
                                        }
                                    }
                                }
                            }

                            if !remote_features.is_empty() {
                                let selected_feats = remote_features.iter().find(|(name, _)| name == &selected_remote)
                                    .map(|(_, list)| list.clone()).unwrap_or_default();

                                let union_remotes_features = if is_union {
                                    let mut up_list = Vec::new();
                                    for u in &upstreams {
                                        if let Some((_, list)) = remote_features.iter().find(|(name, _)| name == u) {
                                            up_list.push((u.clone(), list.clone()));
                                        }
                                    }
                                    Some(up_list)
                                } else {
                                    None
                                };

                                self.connection_state.wizard = ui::connection::WizardState::ShowFeatures {
                                    remote_name: selected_remote,
                                    features: selected_feats,
                                    union_remotes_features,
                                };
                            } else {
                                self.connection_state.error_message = Some("Không thể tải thông tin tính năng của remote này.".to_string());
                            }
                        }
                    }
                    KeyCode::Delete => {
                        // Hiện cảnh báo xóa kết nối
                        if !self.connection_state.remotes.is_empty() {
                            let selected_remote = self.connection_state.remotes
                                [self.connection_state.selected_idx]
                                .clone();
                            self.delete_confirm = Some(DeleteTarget::Connection(selected_remote));
                        }
                    }
                    _ => {}
                }
            }
            ui::connection::WizardState::SelectProviders {
                mut providers,
                mut selected_idx,
                mut scroll_offset,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.connection_state.wizard = ui::connection::WizardState::None;
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = providers.len() - 1;
                        } else {
                            selected_idx -= 1;
                        }
                        let term_h =
                            crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                        let popup_h = term_h * 75 / 100;
                        let list_h = popup_h.saturating_sub(2);

                        scroll_offset = ui::update_scroll_offset(selected_idx, scroll_offset, list_h, providers.len());

                        self.connection_state.wizard =
                            ui::connection::WizardState::SelectProviders {
                                providers,
                                selected_idx,
                                scroll_offset,
                            };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % providers.len();
                        let term_h =
                            crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                        let popup_h = term_h * 75 / 100;
                        let list_h = popup_h.saturating_sub(2);

                        scroll_offset = ui::update_scroll_offset(selected_idx, scroll_offset, list_h, providers.len());

                        self.connection_state.wizard =
                            ui::connection::WizardState::SelectProviders {
                                providers,
                                selected_idx,
                                scroll_offset,
                            };
                    }
                    KeyCode::Char(' ') => {
                        // Toggle checkbox chọn provider (Bug 27)
                        providers[selected_idx].2 = !providers[selected_idx].2;
                        self.connection_state.wizard =
                            ui::connection::WizardState::SelectProviders {
                                providers,
                                selected_idx,
                                scroll_offset,
                            };
                    }
                    KeyCode::Enter => {
                        // Lấy các provider được tick chọn
                        let selected: Vec<String> = providers
                            .iter()
                            .filter(|(_, _, checked)| *checked)
                            .map(|(name, _, _)| name.clone())
                            .collect();

                        if !selected.is_empty() {
                            self.advance_connection_wizard(selected, tx.clone()).await;
                        } else {
                            // Nếu không tích chọn gì, lấy luôn cái đang hover làm mặc định
                            let current = providers[selected_idx].0.clone();
                            self.advance_connection_wizard(vec![current], tx.clone())
                                .await;
                        }
                    }
                    _ => {}
                }
            }
            ui::connection::WizardState::InputRemoteName {
                provider,
                mut input_buffer,
                selected_providers,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.connection_state.wizard = ui::connection::WizardState::None;
                    }
                    KeyCode::Char(c) => {
                        input_buffer.push(c);
                        self.connection_state.wizard =
                            ui::connection::WizardState::InputRemoteName {
                                provider,
                                input_buffer,
                                selected_providers,
                            };
                    }
                    KeyCode::Backspace => {
                        input_buffer.pop();
                        self.connection_state.wizard =
                            ui::connection::WizardState::InputRemoteName {
                                provider,
                                input_buffer,
                                selected_providers,
                            };
                    }
                    KeyCode::Enter => {
                        let name = input_buffer.trim().to_string();
                        if !name.is_empty() {
                            // Truy vấn các option của provider từ config/providers
                            let mut has_client_id = false;
                            let mut fields = Vec::new();
                            let prov_res = rclone::rpc("config/providers", "{}");
                            if let Ok(prov_rpc_res) = prov_res {
                                if let Ok(prov_val) =
                                    serde_json::from_str::<Value>(&prov_rpc_res.output)
                                {
                                    if let Some(prov_arr) =
                                        prov_val.get("providers").and_then(|p| p.as_array())
                                    {
                                        if let Some(prov_obj) = prov_arr.iter().find(|p| {
                                            p.get("Name").and_then(|n| n.as_str())
                                                == Some(&provider)
                                        }) {
                                            if let Some(opts_arr) =
                                                prov_obj.get("Options").and_then(|o| o.as_array())
                                            {
                                                for opt_val in opts_arr {
                                                    let opt_name = opt_val
                                                        .get("Name")
                                                        .and_then(|n| n.as_str())
                                                        .unwrap_or("")
                                                        .to_string();
                                                    let opt_help = opt_val
                                                        .get("Help")
                                                        .and_then(|h| h.as_str())
                                                        .unwrap_or("")
                                                        .to_string();
                                                    let opt_default = opt_val
                                                        .get("Default")
                                                        .map(|v| match v {
                                                            Value::String(s) => s.clone(),
                                                            Value::Number(num) => num.to_string(),
                                                            Value::Bool(b) => b.to_string(),
                                                            _ => v.to_string(),
                                                        })
                                                        .unwrap_or_default();

                                                    let opt_type = opt_val
                                                        .get("Type")
                                                        .and_then(|t| t.as_str())
                                                        .unwrap_or("");
                                                    let mut choices = Vec::new();
                                                    if opt_type == "bool" {
                                                        choices.push("true".to_string());
                                                        choices.push("false".to_string());
                                                    }
                                                    if let Some(examples_arr) = opt_val
                                                        .get("Examples")
                                                        .and_then(|e| e.as_array())
                                                    {
                                                        for ex in examples_arr {
                                                            if let Some(val) = ex
                                                                .get("Value")
                                                                .and_then(|v| v.as_str())
                                                            {
                                                                choices.push(val.to_string());
                                                            }
                                                        }
                                                    }
                                                    let mut unique_choices = Vec::new();
                                                    for c in choices {
                                                        if !unique_choices.contains(&c) {
                                                            unique_choices.push(c);
                                                        }
                                                    }
                                                    let choices = unique_choices;

                                                    if opt_name == "client_id" {
                                                        has_client_id = true;
                                                    }
                                                    if opt_name != "type" {
                                                        fields.push((
                                                            opt_name,
                                                            opt_help,
                                                            opt_default,
                                                            choices,
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            if has_client_id {
                                // Nhà cung cấp hỗ trợ OAuth (như drive, dropbox, ...) -> Hỏi chế độ auth
                                self.connection_state.wizard =
                                    ui::connection::WizardState::SelectAuthMode {
                                        provider,
                                        remote_name: name,
                                        selected_idx: 0,
                                        selected_providers,
                                    };
                            } else {
                                // Nhà cung cấp thông thường (như crypt, sftp, local, ...) -> Cấu hình trực tiếp tất cả tham số
                                self.connection_state.wizard =
                                    ui::connection::WizardState::AdvancedSetup {
                                        provider,
                                        remote_name: name,
                                        fields,
                                        selected_field_idx: 0,
                                        scroll_offset: 0,
                                        is_editing: false,
                                        input_buffer: String::new(),
                                        selected_providers,
                                        active_tab: 0,
                                    };
                            }
                        }
                    }
                    _ => {}
                }
            }
            ui::connection::WizardState::SelectAuthMode {
                provider,
                remote_name,
                mut selected_idx,
                selected_providers,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.connection_state.wizard = ui::connection::WizardState::None;
                    }
                    KeyCode::Up => {
                        selected_idx = if selected_idx == 0 { 2 } else { selected_idx - 1 };
                        self.connection_state.wizard =
                            ui::connection::WizardState::SelectAuthMode {
                                provider,
                                remote_name,
                                selected_idx,
                                selected_providers,
                            };
                    }
                    KeyCode::Down | KeyCode::Tab => {
                        selected_idx = (selected_idx + 1) % 3;
                        self.connection_state.wizard =
                            ui::connection::WizardState::SelectAuthMode {
                                provider,
                                remote_name,
                                selected_idx,
                                selected_providers,
                            };
                    }
                    KeyCode::Enter => {
                        if selected_idx == 0 {
                            // Simple OAuth: Tự động mở duyệt xác thực
                            let prov_clone = provider.clone();
                            let remote_clone = remote_name.clone();
                            let providers_clone = selected_providers.clone();

                            self.connection_state.wizard =
                                ui::connection::WizardState::SimpleOAuthLoop {
                                    provider: prov_clone.clone(),
                                    remote_name: remote_clone.clone(),
                                    auth_url:
                                        "Đang yêu cầu máy chủ Google/Rclone cấp link xác thực..."
                                            .to_string(),
                                    selected_providers: providers_clone.clone(),
                                };

                            let tx_oauth = tx.clone();
                            tokio::spawn(async move {
                                // Gọi API tạo config tự động
                                let param = json!({
                                    "name": remote_clone,
                                    "type": prov_clone,
                                    "parameters": {
                                        "config_is_local": "true",
                                        "config_automatic": "true"
                                    }
                                })
                                .to_string();

                                // Ở đây giả lập/gọi RPC Rclone thực tế.
                                // RPC config/create cho OAuth tự động trả URL trong stdout
                                let res =
                                    rclone::rpc_async("config/create".to_string(), param).await;
                                match res {
                                    Ok(_) => {
                                        let _ = tx_oauth
                                            .send(AppEvent::OAuthFinished { result: Ok(()) });
                                    }
                                    Err(e) => {
                                        let _ = tx_oauth
                                            .send(AppEvent::OAuthFinished { result: Err(e) });
                                    }
                                }
                            });

                            let tx_poll = tx.clone();
                            tokio::spawn(async move {
                                // Poll config/oauthstatus for 60 seconds (300 iterations * 200ms)
                                for _ in 0..300 {
                                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                                    let status_res = rclone::rpc_async("config/oauthstatus".to_string(), "{}".to_string()).await;
                                    if let Ok(res) = status_res {
                                        if let Ok(status_val) = serde_json::from_str::<serde_json::Value>(&res.output) {
                                            if status_val.get("status").and_then(|s| s.as_str()) == Some("running") {
                                                if let Some(auth_url) = status_val.get("authUrl").and_then(|u| u.as_str()) {
                                                    let _ = tx_poll.send(AppEvent::OAuthUrlReceived { url: auth_url.to_string() });
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            });
                        } else if selected_idx == 1 {
                            // Headless OAuth
                            self.connection_state.wizard =
                                ui::connection::WizardState::HeadlessOAuthInput {
                                    provider: provider.clone(),
                                    remote_name: remote_name.clone(),
                                    client_id: String::new(),
                                    client_secret: String::new(),
                                    token_input: String::new(),
                                    focused_idx: 0,
                                    selected_providers: selected_providers.clone(),
                                };
                        } else {
                            // Advanced Setup: Cấu hình tất cả tham số cho provider OAuth này
                            let mut fields = Vec::new();
                            let prov_res = rclone::rpc("config/providers", "{}");
                            if let Ok(prov_rpc_res) = prov_res {
                                if let Ok(prov_val) =
                                    serde_json::from_str::<Value>(&prov_rpc_res.output)
                                {
                                    if let Some(prov_arr) =
                                        prov_val.get("providers").and_then(|p| p.as_array())
                                    {
                                        if let Some(prov_obj) = prov_arr.iter().find(|p| {
                                            p.get("Name").and_then(|n| n.as_str())
                                                == Some(&provider)
                                        }) {
                                            if let Some(opts_arr) =
                                                prov_obj.get("Options").and_then(|o| o.as_array())
                                            {
                                                for opt_val in opts_arr {
                                                    let opt_name = opt_val
                                                        .get("Name")
                                                        .and_then(|n| n.as_str())
                                                        .unwrap_or("")
                                                        .to_string();
                                                    let opt_help = opt_val
                                                        .get("Help")
                                                        .and_then(|h| h.as_str())
                                                        .unwrap_or("")
                                                        .to_string();
                                                    let opt_default = opt_val
                                                        .get("Default")
                                                        .map(|v| match v {
                                                            Value::String(s) => s.clone(),
                                                            Value::Number(num) => num.to_string(),
                                                            Value::Bool(b) => b.to_string(),
                                                            _ => v.to_string(),
                                                        })
                                                        .unwrap_or_default();

                                                    let opt_type = opt_val
                                                        .get("Type")
                                                        .and_then(|t| t.as_str())
                                                        .unwrap_or("");
                                                    let mut choices = Vec::new();
                                                    if opt_type == "bool" {
                                                        choices.push("true".to_string());
                                                        choices.push("false".to_string());
                                                    }
                                                    if let Some(examples_arr) = opt_val
                                                        .get("Examples")
                                                        .and_then(|e| e.as_array())
                                                    {
                                                        for ex in examples_arr {
                                                            if let Some(val) = ex
                                                                .get("Value")
                                                                .and_then(|v| v.as_str())
                                                            {
                                                                choices.push(val.to_string());
                                                            }
                                                        }
                                                    }
                                                    let mut unique_choices = Vec::new();
                                                    for c in choices {
                                                        if !unique_choices.contains(&c) {
                                                            unique_choices.push(c);
                                                        }
                                                    }
                                                    let choices = unique_choices;

                                                    if opt_name != "type" {
                                                        fields.push((
                                                            opt_name,
                                                            opt_help,
                                                            opt_default,
                                                            choices,
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Sắp xếp: đưa client_id và client_secret lên đầu
                            fields.sort_by(|a, b| {
                                let a_is_oauth = a.0 == "client_id" || a.0 == "client_secret";
                                let b_is_oauth = b.0 == "client_id" || b.0 == "client_secret";
                                b_is_oauth.cmp(&a_is_oauth).then_with(|| a.0.cmp(&b.0))
                            });

                            self.connection_state.wizard =
                                ui::connection::WizardState::AdvancedSetup {
                                    provider,
                                    remote_name,
                                    fields,
                                    selected_field_idx: 0,
                                    scroll_offset: 0,
                                    is_editing: false,
                                    input_buffer: String::new(),
                                    selected_providers,
                                    active_tab: 0,
                                };
                        }
                    }
                    _ => {}
                }
            }
            ui::connection::WizardState::HeadlessOAuthInput {
                provider,
                remote_name,
                mut client_id,
                mut client_secret,
                mut token_input,
                mut focused_idx,
                selected_providers,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.connection_state.wizard = ui::connection::WizardState::None;
                    }
                    KeyCode::Tab => {
                        focused_idx = (focused_idx + 1) % 3;
                        self.connection_state.wizard = ui::connection::WizardState::HeadlessOAuthInput {
                            provider, remote_name, client_id, client_secret, token_input, focused_idx, selected_providers
                        };
                    }
                    KeyCode::Char(c) => {
                        if focused_idx == 0 {
                            client_id.push(c);
                        } else if focused_idx == 1 {
                            client_secret.push(c);
                        } else {
                            token_input.push(c);
                        }
                        self.connection_state.wizard = ui::connection::WizardState::HeadlessOAuthInput {
                            provider, remote_name, client_id, client_secret, token_input, focused_idx, selected_providers
                        };
                    }
                    KeyCode::Backspace => {
                        if focused_idx == 0 {
                            client_id.pop();
                        } else if focused_idx == 1 {
                            client_secret.pop();
                        } else {
                            token_input.pop();
                        }
                        self.connection_state.wizard = ui::connection::WizardState::HeadlessOAuthInput {
                            provider, remote_name, client_id, client_secret, token_input, focused_idx, selected_providers
                        };
                    }
                    KeyCode::Enter => {
                        let token_trimmed = token_input.trim().to_string();
                        if token_trimmed.is_empty() {
                            self.connection_state.error_message = Some("Token không được để trống!".to_string());
                            return;
                        }

                        let mut params = serde_json::Map::new();
                        params.insert("token".to_string(), serde_json::Value::String(token_trimmed));
                        if !client_id.trim().is_empty() {
                            params.insert("client_id".to_string(), serde_json::Value::String(client_id.trim().to_string()));
                        }
                        if !client_secret.trim().is_empty() {
                            params.insert("client_secret".to_string(), serde_json::Value::String(client_secret.trim().to_string()));
                        }

                        let rclone_param = json!({
                            "name": remote_name,
                            "type": provider,
                            "parameters": params,
                        })
                        .to_string();

                        let res = rclone::rpc("config/create", &rclone_param);
                        match res {
                            Ok(rpc_res) if rpc_res.status == 200 => {
                                self.connection_state.info_message = Some(format!("Tạo kết nối '{}' thành công qua Headless OAuth!", remote_name));
                                self.connection_state.wizard = ui::connection::WizardState::None;
                                self.load_remotes(tx.clone()).await;
                            }
                            Ok(rpc_res) => {
                                self.connection_state.error_message = Some(format!("Mã lỗi RPC: {}. Chi tiết: {}", rpc_res.status, rpc_res.output));
                            }
                            Err(e) => {
                                self.connection_state.error_message = Some(format!("Lỗi gọi RPC: {}", e));
                            }
                        }
                    }
                    _ => {}
                }
            }
            ui::connection::WizardState::SimpleOAuthLoop { .. } => {
                if key.code == KeyCode::Esc {
                    // Hủy OAuth
                    self.connection_state.wizard = ui::connection::WizardState::None;
                    tokio::spawn(async move {
                        let _ = rclone::rpc_async("config/oauthstop".to_string(), "{}".to_string()).await;
                    });
                }
            }
            ui::connection::WizardState::AdvancedSetup {
                provider,
                remote_name,
                mut fields,
                mut selected_field_idx,
                mut scroll_offset,
                mut is_editing,
                mut input_buffer,
                selected_providers,
                active_tab,
            } => {
                // Lọc danh sách fields theo tab
                let filtered_fields: Vec<(String, String, String, Vec<String>)> = fields
                    .iter()
                    .filter(|(name, _, _, _)| {
                        if active_tab == 0 {
                            ui::connection::is_basic_field(name)
                        } else {
                            !ui::connection::is_basic_field(name)
                        }
                    })
                    .cloned()
                    .collect();

                let save_idx = filtered_fields.len();
                let cancel_idx = filtered_fields.len() + 1;
                let total_items = filtered_fields.len() + 2;

                if is_editing {
                    let is_remote_field =
                        filtered_fields.get(selected_field_idx).map(|f| f.0.as_str()) == Some("remote");
                    let field_choices = filtered_fields.get(selected_field_idx).map(|f| &f.3);
                    if is_remote_field && (key.code == KeyCode::Up || key.code == KeyCode::Down) {
                        let remote_list = &self.connection_state.remotes;
                        if !remote_list.is_empty() {
                            let current_val = input_buffer.trim_end_matches(':');
                            let current_idx = remote_list.iter().position(|r| r == current_val);
                            let next_idx = match current_idx {
                                Some(idx) => {
                                    if key.code == KeyCode::Up {
                                        if idx == 0 {
                                            remote_list.len() - 1
                                        } else {
                                            idx - 1
                                        }
                                    } else {
                                        (idx + 1) % remote_list.len()
                                    }
                                }
                                None => 0,
                            };
                            input_buffer = format!("{}:", remote_list[next_idx]);
                        }
                        self.connection_state.wizard = ui::connection::WizardState::AdvancedSetup {
                            provider,
                            remote_name,
                            fields,
                            selected_field_idx,
                            scroll_offset,
                            is_editing,
                            input_buffer,
                            selected_providers,
                            active_tab,
                        };
                    } else if let Some(choices) = field_choices {
                        if !choices.is_empty()
                            && (key.code == KeyCode::Up || key.code == KeyCode::Down)
                        {
                            let current_idx = choices.iter().position(|c| c == &input_buffer);
                            let next_idx = match current_idx {
                                Some(idx) => {
                                    if key.code == KeyCode::Up {
                                        if idx == 0 { choices.len() - 1 } else { idx - 1 }
                                    } else {
                                        (idx + 1) % choices.len()
                                    }
                                }
                                None => 0,
                            };
                            input_buffer = choices[next_idx].clone();
                            self.connection_state.wizard =
                                ui::connection::WizardState::AdvancedSetup {
                                    provider,
                                    remote_name,
                                    fields,
                                    selected_field_idx,
                                    scroll_offset,
                                    is_editing,
                                    input_buffer,
                                    selected_providers,
                                    active_tab,
                                };
                        } else {
                            let mut cursor = self.connection_state.edit_cursor_idx;
                            if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                                self.connection_state.edit_cursor_idx = cursor;
                                self.connection_state.wizard =
                                    ui::connection::WizardState::AdvancedSetup {
                                        provider,
                                        remote_name,
                                        fields,
                                        selected_field_idx,
                                        scroll_offset,
                                        is_editing,
                                        input_buffer,
                                        selected_providers,
                                        active_tab,
                                    };
                            } else {
                                match key.code {
                                    KeyCode::Esc => {
                                        is_editing = false;
                                        self.connection_state.wizard =
                                            ui::connection::WizardState::AdvancedSetup {
                                                provider,
                                                remote_name,
                                                fields,
                                                selected_field_idx,
                                                scroll_offset,
                                                is_editing,
                                                input_buffer,
                                                selected_providers,
                                                active_tab,
                                            };
                                    }
                                    KeyCode::Enter => {
                                        if let Some(f) = filtered_fields.get(selected_field_idx) {
                                            if let Some(real_idx) = fields.iter().position(|real_f| real_f.0 == f.0) {
                                                fields[real_idx].2 = input_buffer.clone();
                                            }
                                        }
                                        is_editing = false;
                                        self.connection_state.wizard =
                                            ui::connection::WizardState::AdvancedSetup {
                                                provider,
                                                remote_name,
                                                fields,
                                                selected_field_idx,
                                                scroll_offset,
                                                is_editing,
                                                input_buffer,
                                                selected_providers,
                                                active_tab,
                                            };
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                } else {
                    match key.code {
                        KeyCode::Esc => {
                            self.connection_state.wizard = ui::connection::WizardState::None;
                        }
                        KeyCode::Tab | KeyCode::Right | KeyCode::Left => {
                            let new_tab = if active_tab == 0 { 1 } else { 0 };
                            self.connection_state.wizard = ui::connection::WizardState::AdvancedSetup {
                                provider,
                                remote_name,
                                fields,
                                selected_field_idx: 0,
                                scroll_offset: 0,
                                is_editing: false,
                                input_buffer: String::new(),
                                selected_providers,
                                active_tab: new_tab,
                            };
                        }
                        KeyCode::Up => {
                            if selected_field_idx == 0 {
                                selected_field_idx = total_items - 1;
                            } else {
                                selected_field_idx -= 1;
                            }
                            let term_h =
                                crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                            let popup_h = term_h * 75 / 100;
                            let list_h = popup_h.saturating_sub(4);

                            if selected_field_idx < filtered_fields.len() {
                                scroll_offset = ui::update_scroll_offset(selected_field_idx, scroll_offset, list_h, filtered_fields.len());
                            } else {
                                scroll_offset = filtered_fields.len().saturating_sub(list_h);
                            }

                            self.connection_state.wizard =
                                ui::connection::WizardState::AdvancedSetup {
                                    provider,
                                    remote_name,
                                    fields,
                                    selected_field_idx,
                                    scroll_offset,
                                    is_editing,
                                    input_buffer,
                                    selected_providers,
                                    active_tab,
                                };
                        }
                        KeyCode::Down => {
                            selected_field_idx = (selected_field_idx + 1) % total_items;
                            let term_h =
                                crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                            let popup_h = term_h * 75 / 100;
                            let list_h = popup_h.saturating_sub(4);

                            if selected_field_idx < filtered_fields.len() {
                                scroll_offset = ui::update_scroll_offset(selected_field_idx, scroll_offset, list_h, filtered_fields.len());
                            } else {
                                scroll_offset = filtered_fields.len().saturating_sub(list_h);
                            }

                            self.connection_state.wizard =
                                ui::connection::WizardState::AdvancedSetup {
                                    provider,
                                    remote_name,
                                    fields,
                                    selected_field_idx,
                                    scroll_offset,
                                    is_editing,
                                    input_buffer,
                                    selected_providers,
                                    active_tab,
                                };
                        }
                        KeyCode::Enter => {
                            if selected_field_idx < filtered_fields.len() {
                                is_editing = true;
                                input_buffer = filtered_fields[selected_field_idx].2.clone();
                                self.connection_state.edit_cursor_idx = input_buffer.chars().count();
                                self.connection_state.wizard =
                                    ui::connection::WizardState::AdvancedSetup {
                                        provider,
                                        remote_name,
                                        fields,
                                        selected_field_idx,
                                        scroll_offset,
                                        is_editing,
                                        input_buffer,
                                        selected_providers,
                                        active_tab,
                                    };
                            } else if selected_field_idx == save_idx {
                                // Lưu cấu hình remote mới
                                let mut params = HashMap::new();
                                for (name, _, val, _) in fields.iter() {
                                    let val_trimmed = val.trim();
                                    let is_empty_password = (name.to_lowercase().contains("pass")
                                        || name.to_lowercase().contains("salt")
                                        || name.to_lowercase().contains("secret")
                                        || name.to_lowercase().contains("key")
                                        || name.to_lowercase().contains("token")
                                        || name == "password2")
                                        && val_trimmed.is_empty();
                                    if !is_empty_password {
                                        params.insert(name.clone(), val.clone());
                                    }
                                }
                                let rclone_param = json!({
                                    "name": remote_name,
                                    "type": provider,
                                    "parameters": params
                                })
                                .to_string();

                                let res = rclone::rpc("config/create", &rclone_param);
                                match res {
                                    Ok(_) => {
                                        self.connection_state.info_message = Some(format!(
                                            "Đã tạo remote '{}' thành công!",
                                            remote_name
                                        ));
                                        self.advance_connection_wizard(
                                            selected_providers,
                                            tx.clone(),
                                        )
                                        .await;
                                        self.load_remotes(tx.clone()).await;
                                    }
                                    Err(e) => {
                                        self.connection_state.error_message =
                                            Some(format!("Lỗi khi tạo remote: {}", e));
                                    }
                                }
                            } else if selected_field_idx == cancel_idx {
                                self.connection_state.wizard = ui::connection::WizardState::None;
                            }
                        }
                        KeyCode::Char('s') | KeyCode::Char('S') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            // Lưu cấu hình remote mới
                            let mut params = HashMap::new();
                            for (name, _, val, _) in fields.iter() {
                                let val_trimmed = val.trim();
                                let is_empty_password = (name.to_lowercase().contains("pass")
                                    || name.to_lowercase().contains("salt")
                                    || name.to_lowercase().contains("secret")
                                    || name.to_lowercase().contains("key")
                                    || name.to_lowercase().contains("token")
                                    || name == "password2")
                                    && val_trimmed.is_empty();
                                if !is_empty_password {
                                    params.insert(name.clone(), val.clone());
                                }
                            }
                            let rclone_param = json!({
                            "name": remote_name,
                            "type": provider,
                            "parameters": params
                            })
                            .to_string();

                            let res = rclone::rpc("config/create", &rclone_param);
                            match res {
                                Ok(_) => {
                                    self.connection_state.info_message = Some(format!(
                                        "Đã tạo remote '{}' thành công!",
                                        remote_name
                                    ));
                                    self.advance_connection_wizard(selected_providers, tx.clone())
                                        .await;
                                    self.load_remotes(tx.clone()).await;
                                }
                                Err(e) => {
                                    self.connection_state.error_message =
                                        Some(format!("Lỗi khi tạo remote: {}", e));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            ui::connection::WizardState::EditSetup {
                remote_name,
                provider,
                mut fields,
                mut selected_idx,
                mut scroll_offset,
                mut is_editing,
                mut input_buffer,
                adding_new_key: _,
                new_key_buffer: _,
                active_tab,
            } => {
                // Lọc danh sách fields theo tab
                let filtered_fields: Vec<(String, String, String, Vec<String>)> = fields
                    .iter()
                    .filter(|(name, _, _, _)| {
                        if active_tab == 0 {
                            ui::connection::is_basic_field(name)
                        } else {
                            !ui::connection::is_basic_field(name)
                        }
                    })
                    .cloned()
                    .collect();

                let save_idx = filtered_fields.len();
                let cancel_idx = filtered_fields.len() + 1;
                let total_items = filtered_fields.len() + 2;

                if is_editing {
                    let is_remote_field =
                        filtered_fields.get(selected_idx).map(|f| f.0.as_str()) == Some("remote");
                    let field_choices = filtered_fields.get(selected_idx).map(|f| &f.3);
                    if is_remote_field && (key.code == KeyCode::Up || key.code == KeyCode::Down) {
                        let remote_list = &self.connection_state.remotes;
                        if !remote_list.is_empty() {
                            let current_val = input_buffer.trim_end_matches(':');
                            let current_idx = remote_list.iter().position(|r| r == current_val);
                            let next_idx = match current_idx {
                                Some(idx) => {
                                    if key.code == KeyCode::Up {
                                        if idx == 0 {
                                            remote_list.len() - 1
                                        } else {
                                            idx - 1
                                        }
                                    } else {
                                        (idx + 1) % remote_list.len()
                                    }
                                }
                                None => 0,
                            };
                            input_buffer = format!("{}:", remote_list[next_idx]);
                        }
                        self.connection_state.wizard = ui::connection::WizardState::EditSetup {
                            remote_name,
                            provider,
                            fields,
                            selected_idx,
                            scroll_offset,
                            is_editing,
                            input_buffer,
                            adding_new_key: false,
                            new_key_buffer: String::new(),
                            active_tab,
                        };
                    } else if let Some(choices) = field_choices {
                        if !choices.is_empty()
                            && (key.code == KeyCode::Up || key.code == KeyCode::Down)
                        {
                            let current_idx = choices.iter().position(|c| c == &input_buffer);
                            let next_idx = match current_idx {
                                Some(idx) => {
                                    if key.code == KeyCode::Up {
                                        if idx == 0 { choices.len() - 1 } else { idx - 1 }
                                    } else {
                                        (idx + 1) % choices.len()
                                    }
                                }
                                None => 0,
                            };
                            input_buffer = choices[next_idx].clone();
                            self.connection_state.wizard = ui::connection::WizardState::EditSetup {
                                remote_name,
                                provider,
                                fields,
                                selected_idx,
                                scroll_offset,
                                is_editing,
                                input_buffer,
                                adding_new_key: false,
                                new_key_buffer: String::new(),
                                active_tab,
                            };
                        } else {
                            let mut cursor = self.connection_state.edit_cursor_idx;
                            if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                                self.connection_state.edit_cursor_idx = cursor;
                                self.connection_state.wizard = ui::connection::WizardState::EditSetup {
                                    remote_name,
                                    provider,
                                    fields,
                                    selected_idx,
                                    scroll_offset,
                                    is_editing,
                                    input_buffer,
                                    adding_new_key: false,
                                    new_key_buffer: String::new(),
                                    active_tab,
                                };
                            } else {
                                match key.code {
                                    KeyCode::Esc => {
                                        is_editing = false;
                                        self.connection_state.wizard =
                                            ui::connection::WizardState::EditSetup {
                                                remote_name,
                                                provider,
                                                fields,
                                                selected_idx,
                                                scroll_offset,
                                                is_editing,
                                                input_buffer,
                                                adding_new_key: false,
                                                new_key_buffer: String::new(),
                                                active_tab,
                                            };
                                    }
                                    KeyCode::Enter => {
                                        if let Some(f) = filtered_fields.get(selected_idx) {
                                            if let Some(real_idx) = fields.iter().position(|real_f| real_f.0 == f.0) {
                                                fields[real_idx].2 = input_buffer.clone();
                                            }
                                        }
                                        is_editing = false;
                                        self.connection_state.wizard =
                                            ui::connection::WizardState::EditSetup {
                                                remote_name,
                                                provider,
                                                fields,
                                                selected_idx,
                                                scroll_offset,
                                                is_editing,
                                                input_buffer,
                                                adding_new_key: false,
                                                new_key_buffer: String::new(),
                                                active_tab,
                                            };
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                } else {
                    match key.code {
                        KeyCode::Esc => {
                            self.connection_state.wizard = ui::connection::WizardState::None;
                        }
                        KeyCode::Tab | KeyCode::Right | KeyCode::Left => {
                            let new_tab = if active_tab == 0 { 1 } else { 0 };
                            self.connection_state.wizard = ui::connection::WizardState::EditSetup {
                                remote_name,
                                provider,
                                fields,
                                selected_idx: 0,
                                scroll_offset: 0,
                                is_editing: false,
                                input_buffer: String::new(),
                                adding_new_key: false,
                                new_key_buffer: String::new(),
                                active_tab: new_tab,
                            };
                        }
                        KeyCode::Up => {
                            if selected_idx == 0 {
                                selected_idx = total_items - 1;
                            } else {
                                selected_idx -= 1;
                            }
                            let term_h =
                                crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                            let popup_h = term_h * 75 / 100;
                            let list_h = popup_h.saturating_sub(4);

                            if selected_idx < filtered_fields.len() {
                                scroll_offset = ui::update_scroll_offset(selected_idx, scroll_offset, list_h, filtered_fields.len());
                            } else {
                                scroll_offset = filtered_fields.len().saturating_sub(list_h);
                            }

                            self.connection_state.wizard = ui::connection::WizardState::EditSetup {
                                remote_name,
                                provider,
                                fields,
                                selected_idx,
                                scroll_offset,
                                is_editing,
                                input_buffer,
                                adding_new_key: false,
                                new_key_buffer: String::new(),
                                active_tab,
                            };
                        }
                        KeyCode::Down => {
                            selected_idx = (selected_idx + 1) % total_items;
                            let term_h =
                                crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                            let popup_h = term_h * 75 / 100;
                            let list_h = popup_h.saturating_sub(4);

                            if selected_idx < filtered_fields.len() {
                                scroll_offset = ui::update_scroll_offset(selected_idx, scroll_offset, list_h, filtered_fields.len());
                            } else {
                                scroll_offset = filtered_fields.len().saturating_sub(list_h);
                            }

                            self.connection_state.wizard = ui::connection::WizardState::EditSetup {
                                remote_name,
                                provider,
                                fields,
                                selected_idx,
                                scroll_offset,
                                is_editing,
                                input_buffer,
                                adding_new_key: false,
                                new_key_buffer: String::new(),
                                active_tab,
                            };
                        }
                        KeyCode::Enter => {
                            if selected_idx < filtered_fields.len() {
                                is_editing = true;
                                input_buffer = filtered_fields[selected_idx].2.clone();
                                self.connection_state.edit_cursor_idx = input_buffer.chars().count();
                                self.connection_state.wizard =
                                    ui::connection::WizardState::EditSetup {
                                        remote_name,
                                        provider,
                                        fields,
                                        selected_idx,
                                        scroll_offset,
                                        is_editing,
                                        input_buffer,
                                        adding_new_key: false,
                                        new_key_buffer: String::new(),
                                        active_tab,
                                    };
                            } else if selected_idx == save_idx {
                                let mut params = HashMap::new();
                                let mut new_remote_name = remote_name.clone();
                                for (name, _, val, _) in fields.iter() {
                                    if name == "_remote_name" {
                                        new_remote_name = val.trim().to_string();
                                    } else {
                                        let val_trimmed = val.trim();
                                        let is_empty_password = (name.to_lowercase().contains("pass")
                                            || name.to_lowercase().contains("salt")
                                            || name.to_lowercase().contains("secret")
                                            || name.to_lowercase().contains("key")
                                            || name.to_lowercase().contains("token")
                                            || name == "password2")
                                            && val_trimmed.is_empty();
                                        if !is_empty_password {
                                            params.insert(name.clone(), val.clone());
                                        }
                                    }
                                }

                                if new_remote_name.is_empty() {
                                    self.connection_state.error_message =
                                        Some("Tên remote không được để trống!".to_string());
                                } else if new_remote_name != remote_name {
                                    let rclone_param = json!({
                                        "name": new_remote_name,
                                        "type": provider,
                                        "parameters": params
                                    })
                                    .to_string();

                                    let create_res = rclone::rpc("config/create", &rclone_param);
                                    match create_res {
                                        Ok(_) => {
                                            let delete_param = json!({
                                                "name": remote_name
                                            })
                                            .to_string();
                                            let _ = rclone::rpc("config/delete", &delete_param);

                                            self.connection_state.info_message = Some(format!(
                                                "Đã đổi tên remote thành '{}' thành công!",
                                                new_remote_name
                                            ));
                                            self.connection_state.wizard =
                                                ui::connection::WizardState::None;
                                            self.load_remotes(tx.clone()).await;
                                        }
                                        Err(e) => {
                                            self.connection_state.error_message =
                                                Some(format!("Lỗi khi đổi tên remote: {}", e));
                                        }
                                    }
                                } else {
                                    let rclone_param = json!({
                                        "name": remote_name,
                                        "parameters": params
                                    })
                                    .to_string();

                                    let rpc_res = rclone::rpc("config/update", &rclone_param);
                                    match rpc_res {
                                        Ok(_) => {
                                            self.connection_state.info_message = Some(format!(
                                                "Đã cập nhật remote '{}' thành công!",
                                                remote_name
                                            ));
                                            self.connection_state.wizard =
                                                ui::connection::WizardState::None;
                                            self.load_remotes(tx.clone()).await;
                                        }
                                        Err(e) => {
                                            self.connection_state.error_message =
                                                Some(format!("Lỗi khi cập nhật remote: {}", e));
                                        }
                                    }
                                }
                            } else if selected_idx == cancel_idx {
                                self.connection_state.wizard = ui::connection::WizardState::None;
                            }
                        }
                        KeyCode::Char('s') | KeyCode::Char('S') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            let mut params = HashMap::new();
                            let mut new_remote_name = remote_name.clone();
                            for (name, _, val, _) in fields.iter() {
                                if name == "_remote_name" {
                                    new_remote_name = val.trim().to_string();
                                } else {
                                    let val_trimmed = val.trim();
                                    let is_empty_password = (name.to_lowercase().contains("pass")
                                        || name.to_lowercase().contains("salt")
                                        || name.to_lowercase().contains("secret")
                                        || name.to_lowercase().contains("key")
                                        || name.to_lowercase().contains("token")
                                        || name == "password2")
                                        && val_trimmed.is_empty();
                                    if !is_empty_password {
                                        params.insert(name.clone(), val.clone());
                                    }
                                }
                            }

                            if new_remote_name.is_empty() {
                                self.connection_state.error_message =
                                    Some("Tên remote không được để trống!".to_string());
                            } else if new_remote_name != remote_name {
                                let rclone_param = json!({
                                    "name": new_remote_name,
                                    "type": provider,
                                    "parameters": params
                                })
                                .to_string();

                                let create_res = rclone::rpc("config/create", &rclone_param);
                                match create_res {
                                    Ok(_) => {
                                        let delete_param = json!({
                                            "name": remote_name
                                        })
                                        .to_string();
                                        let _ = rclone::rpc("config/delete", &delete_param);

                                        self.connection_state.info_message = Some(format!(
                                            "Đã đổi tên remote thành '{}' thành công!",
                                            new_remote_name
                                        ));
                                        self.connection_state.wizard =
                                            ui::connection::WizardState::None;
                                        self.load_remotes(tx.clone()).await;
                                    }
                                    Err(e) => {
                                        self.connection_state.error_message =
                                            Some(format!("Lỗi khi đổi tên remote: {}", e));
                                    }
                                }
                            } else {
                                let rclone_param = json!({
                                    "name": remote_name,
                                    "parameters": params
                                })
                                .to_string();

                                let rpc_res = rclone::rpc("config/update", &rclone_param);
                                match rpc_res {
                                    Ok(_) => {
                                        self.connection_state.info_message = Some(format!(
                                            "Đã cập nhật remote '{}' thành công!",
                                            remote_name
                                        ));
                                        self.connection_state.wizard =
                                            ui::connection::WizardState::None;
                                        self.load_remotes(tx.clone()).await;
                                    }
                                    Err(e) => {
                                        self.connection_state.error_message =
                                            Some(format!("Lỗi khi cập nhật remote: {}", e));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            ui::connection::WizardState::ShowFeatures { .. } => {
                match key.code {
                    KeyCode::Esc | KeyCode::Enter => {
                        self.connection_state.wizard = ui::connection::WizardState::None;
                    }
                    _ => {}
                }
            }
        }
    }

    async fn handle_explorer_key(
        &mut self,
        key: KeyEvent,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        let popup = self.explorer_state.popup.clone();
        if popup != ui::explorer::ExplorerPopup::None {
            match popup {

                ui::explorer::ExplorerPopup::InputRename {
                    old_name,
                    mut input_buffer,
                    is_dir,
                } => {
                    let mut cursor = self.explorer_state.edit_cursor_idx;
                    if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                        self.explorer_state.edit_cursor_idx = cursor;
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::InputRename {
                            old_name,
                            input_buffer,
                            is_dir,
                        };
                    } else {
                        match key.code {
                            KeyCode::Esc => {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            }
                            KeyCode::Enter => {
                                let new_name = input_buffer.trim().to_string();
                                if !new_name.is_empty() && new_name != old_name {
                                    let pane = self.explorer_state.get_active_pane();
                                    let remote = pane.remote.clone();
                                    let parent_path = pane.path.clone();
                                    
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::None;

                                    let src = if remote.is_empty() {
                                        PathBuf::from(&parent_path).join(&old_name).to_string_lossy().to_string()
                                    } else {
                                        let clean_path = parent_path.trim_start_matches('/').trim_end_matches('/');
                                        if clean_path.is_empty() {
                                            format!("{}:/{}", remote.trim_end_matches(':'), old_name)
                                        } else {
                                            format!("{}:/{}/{}", remote.trim_end_matches(':'), clean_path, old_name)
                                        }
                                    };

                                    let dest = if remote.is_empty() {
                                        PathBuf::from(&parent_path).join(&new_name).to_string_lossy().to_string()
                                    } else {
                                        let clean_path = parent_path.trim_start_matches('/').trim_end_matches('/');
                                        if clean_path.is_empty() {
                                            format!("{}:/{}", remote.trim_end_matches(':'), new_name)
                                        } else {
                                            format!("{}:/{}/{}", remote.trim_end_matches(':'), clean_path, new_name)
                                        }
                                    };

                                    self.check_features_and_execute("rename", src, dest, is_dir, tx.clone());
                                } else {
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                                }
                            }
                            _ => {}
                        }
                    }
                }

                ui::explorer::ExplorerPopup::InputNewFolder { mut input_buffer } => {
                    let mut cursor = self.explorer_state.edit_cursor_idx;
                    if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                        self.explorer_state.edit_cursor_idx = cursor;
                        self.explorer_state.popup =
                            ui::explorer::ExplorerPopup::InputNewFolder { input_buffer };
                    } else {
                        match key.code {
                            KeyCode::Esc => {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            }
                            KeyCode::Enter => {
                                let folder_name = input_buffer.trim().to_string();
                                if !folder_name.is_empty() {
                                let pane = self.explorer_state.get_active_pane_mut();
                                let is_local = pane.remote.is_empty();
                                let target = if is_local {
                                    PathBuf::from(&pane.path)
                                        .join(&folder_name)
                                        .to_string_lossy()
                                        .to_string()
                                } else {
                                    String::new()
                                };

                                // Chèn thư mục mới một cách lạc quan vào danh sách UI
                                if !pane.items.iter().any(|item| item.name == folder_name) {
                                    pane.items.push(ui::explorer::FileItem {
                                        name: folder_name.clone(),
                                        size: 0,
                                        is_dir: true,
                                        mod_time: crate::lang::translate("exp_creating_placeholder"),
                                    });
                                    pane.items.sort_by(|a, b| {
                                        b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name))
                                    });
                                    if let Some(pos) = pane.items.iter().position(|item| item.name == folder_name) {
                                        pane.selected_idx = pos;
                                    }
                                }

                                let param = if is_local {
                                    json!({
                                        "fs": target,
                                        "remote": "",
                                    })
                                } else {
                                    let clean_path = pane.path.trim_start_matches('/').trim_end_matches('/');
                                    let parent_fs = if clean_path.is_empty() {
                                        format!("{}:", pane.remote.trim_end_matches(':'))
                                    } else {
                                        format!("{}:/{}", pane.remote.trim_end_matches(':'), clean_path)
                                    };
                                    json!({
                                        "fs": parent_fs,
                                        "remote": folder_name.clone(),
                                    })
                                }
                                .to_string();

                                let tx_op = tx.clone();
                                let pane_type = self.explorer_state.active_pane.clone();
                                tokio::spawn(async move {
                                    let res = if is_local {
                                        if std::fs::create_dir_all(&target).is_ok() {
                                            Ok(())
                                        } else {
                                            match std::process::Command::new("pkexec")
                                                .args(&["mkdir", "-p", &target])
                                                .status()
                                            {
                                                Ok(s) if s.success() => Ok(()),
                                                Ok(s) => Err(format!("Quyền root bị từ chối hoặc thất bại (exit: {})", s)),
                                                Err(e) => Err(format!("Lỗi chạy pkexec: {}", e)),
                                            }
                                        }
                                    } else {
                                        let op_res = rclone::rpc_async("operations/mkdir".to_string(), param).await;
                                        match op_res {
                                            Ok(r) if r.status == 200 => Ok(()),
                                            Ok(r) => Err(format!("Mã lỗi: {}", r.status)),
                                            Err(e) => Err(e),
                                        }
                                    };
                                    let _ = tx_op.send(AppEvent::ExplorerOperationFinished {
                                        pane: pane_type,
                                        op_name: "tạo thư mục (mkdir)".to_string(),
                                        result: res,
                                    });
                                });
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            }
                        }
                        _ => {}
                    }
                }
              }
                ui::explorer::ExplorerPopup::SyncConfirm => {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                        }
                        KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                            // Lấy nguồn và đích
                            let src_pane = match self.explorer_state.active_pane {
                                ui::explorer::ActivePane::Left => &self.explorer_state.left_pane,
                                ui::explorer::ActivePane::Right => &self.explorer_state.right_pane,
                            };
                            let dest_pane = match self.explorer_state.active_pane {
                                ui::explorer::ActivePane::Left => &self.explorer_state.right_pane,
                                ui::explorer::ActivePane::Right => &self.explorer_state.left_pane,
                            };

                            let src_fs = if src_pane.remote.is_empty() {
                                src_pane.path.clone()
                            } else {
                                format!("{}:{}", src_pane.remote.trim_end_matches(':'), src_pane.path)
                            };
                            let dest_fs = if dest_pane.remote.is_empty() {
                                dest_pane.path.clone()
                            } else {
                                format!("{}:{}", dest_pane.remote.trim_end_matches(':'), dest_pane.path)
                            };

                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;

                            // Spawn job đồng bộ bất đồng bộ
                            tokio::spawn(async move {
                                let _ = run_rpc_job_async(
                                    "sync/sync".to_string(),
                                    json!({
                                        "srcFs": src_fs,
                                        "dstFs": dest_fs,
                                    }),
                                )
                                .await;
                            });
                        }
                        _ => {}
                    }
                }
                ui::explorer::ExplorerPopup::SelectRemote {
                    remotes,
                    mut selected_idx,
                } => match key.code {
                    KeyCode::Esc => {
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = remotes.len() - 1;
                        } else {
                            selected_idx -= 1;
                        }
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::SelectRemote {
                            remotes,
                            selected_idx,
                        };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % remotes.len();
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::SelectRemote {
                            remotes,
                            selected_idx,
                        };
                    }
                    KeyCode::Enter => {
                        let chosen = remotes[selected_idx].clone();
                        let active_pane = self.explorer_state.get_active_pane_mut();
                        if chosen == "[Local System]" {
                            active_pane.remote = String::new();
                            active_pane.path = crate::app_config::get_home_dir();
                        } else {
                            active_pane.remote = chosen;
                            active_pane.path = String::new();
                        }
                        active_pane.items.clear();
                        active_pane.selected_idx = 0;
                        active_pane.scroll_offset = 0;
                        active_pane.selected_names.clear();
                        active_pane.shift_anchor = None;
                        active_pane.shift_active = false;
                        active_pane.alt_anchor = None;
                        active_pane.alt_active = false;
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;

                        let p_type = self.explorer_state.active_pane.clone();
                        self.refresh_explorer_pane(p_type, tx.clone()).await;
                    }
                    _ => {}
                },
                ui::explorer::ExplorerPopup::ConfirmFallback {
                    title,
                    options,
                    mut selected_idx,
                    actions,
                } => {
                    if (key.code == KeyCode::Char('c') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL))
                        || key.code == KeyCode::Esc
                    {
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                    } else {
                        match key.code {
                            KeyCode::Up => {
                                if selected_idx == 0 {
                                    selected_idx = options.len() - 1;
                                } else {
                                    selected_idx -= 1;
                                }
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::ConfirmFallback {
                                    title,
                                    options,
                                    selected_idx,
                                    actions,
                                };
                            }
                            KeyCode::Down => {
                                selected_idx = (selected_idx + 1) % options.len();
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::ConfirmFallback {
                                    title,
                                    options,
                                    selected_idx,
                                    actions,
                                };
                            }
                            KeyCode::Enter => {
                                let selected_action = actions[selected_idx].clone();
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                                self.execute_fallback_action(selected_action, tx.clone()).await;
                            }
                            _ => {}
                        }
                    }
                },
                ui::explorer::ExplorerPopup::CopyProgress { job_id, .. }
                | ui::explorer::ExplorerPopup::MoveProgress { job_id, .. } => {
                    if (key.code == KeyCode::Char('c') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL))
                        || key.code == KeyCode::Esc
                    {
                        if let Some(id) = job_id {
                            let param = json!({ "jobid": id }).to_string();
                            let _ = rclone::rpc("job/stop", &param);
                        }
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                    }
                }
                ui::explorer::ExplorerPopup::SpecialActionsMenu { mut selected_idx } => {
                    match key.code {
                        KeyCode::Esc => {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                        }
                        KeyCode::Up => {
                            selected_idx = if selected_idx == 0 { 7 } else { selected_idx - 1 };
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::SpecialActionsMenu { selected_idx };
                        }
                        KeyCode::Down => {
                            selected_idx = (selected_idx + 1) % 8;
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::SpecialActionsMenu { selected_idx };
                        }
                        KeyCode::Enter => {
                            if selected_idx == 7 {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            } else {
                                self.handle_special_action_selected(selected_idx, tx.clone()).await;
                            }
                        }
                        _ => {}
                    }
                }
                ui::explorer::ExplorerPopup::ViewFile { file_name, content, mut scroll_offset } => {
                    match key.code {
                        KeyCode::Esc => {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                        }
                        KeyCode::Up => {
                            if scroll_offset > 0 {
                                scroll_offset -= 1;
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::ViewFile { file_name, content, scroll_offset };
                            }
                        }
                        KeyCode::Down => {
                            if scroll_offset + 1 < content.len() {
                                scroll_offset += 1;
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::ViewFile { file_name, content, scroll_offset };
                            }
                        }
                        _ => {}
                    }
                }
                ui::explorer::ExplorerPopup::ChecksumTypeSelect { mut selected_idx } => {
                    let hash_types = vec!["md5".to_string(), "sha1".to_string(), "sha256".to_string(), "crc32".to_string(), "xxhash".to_string()];
                    match key.code {
                        KeyCode::Esc => {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                        }
                        KeyCode::Up => {
                            selected_idx = if selected_idx == 0 { hash_types.len() - 1 } else { selected_idx - 1 };
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::ChecksumTypeSelect { selected_idx };
                        }
                        KeyCode::Down => {
                            selected_idx = (selected_idx + 1) % hash_types.len();
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::ChecksumTypeSelect { selected_idx };
                        }
                        KeyCode::Enter => {
                            let hash_type = hash_types[selected_idx].clone();
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            self.execute_hashsum_file(hash_type, tx.clone()).await;
                        }
                        _ => {}
                    }
                }
                ui::explorer::ExplorerPopup::CryptdecodeForm { mut remote_input, mut encrypted_input, mut is_remote_focused, output_result } => {
                    let mut cursor = self.explorer_state.edit_cursor_idx;
                    let handled = if is_remote_focused {
                        handle_input_key(&key, &mut remote_input, &mut cursor)
                    } else {
                        handle_input_key(&key, &mut encrypted_input, &mut cursor)
                    };
                    if handled {
                        self.explorer_state.edit_cursor_idx = cursor;
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::CryptdecodeForm { remote_input, encrypted_input, is_remote_focused, output_result };
                    } else {
                        match key.code {
                            KeyCode::Esc => {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            }
                            KeyCode::Tab => {
                                is_remote_focused = !is_remote_focused;
                                self.explorer_state.edit_cursor_idx = if is_remote_focused {
                                    remote_input.chars().count()
                                } else {
                                    encrypted_input.chars().count()
                                };
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::CryptdecodeForm { remote_input, encrypted_input, is_remote_focused, output_result };
                            }
                            KeyCode::Enter => {
                                self.execute_cryptdecode(remote_input.clone(), encrypted_input.clone(), tx.clone()).await;
                            }
                            _ => {}
                        }
                    }
                }
                ui::explorer::ExplorerPopup::DecompressModeSelect { archive_path, mut selected_idx } => {
                    match key.code {
                        KeyCode::Esc => {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                        }
                        KeyCode::Up => {
                            selected_idx = if selected_idx == 0 { 2 } else { selected_idx - 1 };
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::DecompressModeSelect { archive_path, selected_idx };
                        }
                        KeyCode::Down => {
                            selected_idx = (selected_idx + 1) % 3;
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::DecompressModeSelect { archive_path, selected_idx };
                        }
                        KeyCode::Enter => {
                            self.handle_decompress_mode_selected(archive_path, selected_idx, tx.clone()).await;
                        }
                        _ => {}
                    }
                }
                ui::explorer::ExplorerPopup::DecompressPathInput { archive_path, mut selected_idx } => {
                    match key.code {
                        KeyCode::Esc => {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                        }
                        KeyCode::Up | KeyCode::Down => {
                            selected_idx = (selected_idx + 1) % 2;
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::DecompressPathInput { archive_path, selected_idx };
                        }
                        KeyCode::Enter => {
                            if selected_idx == 0 {
                                let active_pane = self.explorer_state.get_active_pane();
                                let initial_path = if active_pane.remote.is_empty() {
                                    active_pane.path.clone()
                                } else {
                                    format!("{}:{}", active_pane.remote.trim_end_matches(':'), active_pane.path)
                                };
                                self.explorer_state.edit_cursor_idx = initial_path.chars().count();
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::DecompressPathManualInput {
                                    archive_path,
                                    input_buffer: initial_path,
                                };
                            } else {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::TuiExplorerSelector {
                                    archive_path,
                                    remote: String::new(),
                                    path: String::new(),
                                    items: Vec::new(),
                                    selected_idx: 0,
                                    scroll_offset: 0,
                                    loading: true,
                                };
                                self.refresh_tui_selector_list(tx.clone());
                            }
                        }
                        _ => {}
                    }
                }
                ui::explorer::ExplorerPopup::DecompressPathManualInput { archive_path, mut input_buffer } => {
                    let mut cursor = self.explorer_state.edit_cursor_idx;
                    if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                        self.explorer_state.edit_cursor_idx = cursor;
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::DecompressPathManualInput { archive_path, input_buffer };
                    } else {
                        match key.code {
                            KeyCode::Esc => {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            }
                            KeyCode::Enter => {
                                let dest_path = input_buffer.trim().to_string();
                                if !dest_path.is_empty() {
                                    self.execute_archive_decompress(archive_path, dest_path, tx.clone()).await;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                ui::explorer::ExplorerPopup::TuiExplorerSelector {
                    archive_path,
                    mut remote,
                    mut path,
                    items,
                    mut selected_idx,
                    mut scroll_offset,
                    loading,
                } => {
                    if loading {
                        if key.code == KeyCode::Esc {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                        }
                        return;
                    }
                    match key.code {
                        KeyCode::Esc => {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                        }
                        KeyCode::Up => {
                            if !items.is_empty() {
                                if selected_idx == 0 {
                                    selected_idx = items.len() - 1;
                                } else {
                                    selected_idx -= 1;
                                }
                                let term_h = crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                                let popup_h = term_h * 70 / 100;
                                let list_h = popup_h.saturating_sub(4);
                                scroll_offset = ui::update_scroll_offset(selected_idx, scroll_offset, list_h, items.len());
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::TuiExplorerSelector {
                                    archive_path, remote, path, items, selected_idx, scroll_offset, loading
                                };
                            }
                        }
                        KeyCode::Down => {
                            if !items.is_empty() {
                                selected_idx = (selected_idx + 1) % items.len();
                                let term_h = crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                                let popup_h = term_h * 70 / 100;
                                let list_h = popup_h.saturating_sub(4);
                                scroll_offset = ui::update_scroll_offset(selected_idx, scroll_offset, list_h, items.len());
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::TuiExplorerSelector {
                                    archive_path, remote, path, items, selected_idx, scroll_offset, loading
                                };
                            }
                        }
                        KeyCode::Enter => {
                            if !items.is_empty() {
                                let selected = items[selected_idx].clone();
                                if selected.name == "[Local System]" {
                                    remote = String::new();
                                    path = "/".to_string();
                                    selected_idx = 0;
                                    scroll_offset = 0;
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::TuiExplorerSelector {
                                        archive_path, remote, path, items: Vec::new(), selected_idx, scroll_offset, loading: true
                                    };
                                    self.refresh_tui_selector_list(tx.clone());
                                } else if selected.name.ends_with(':') {
                                    remote = selected.name.clone();
                                    path = String::new();
                                    selected_idx = 0;
                                    scroll_offset = 0;
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::TuiExplorerSelector {
                                        archive_path, remote, path, items: Vec::new(), selected_idx, scroll_offset, loading: true
                                    };
                                    self.refresh_tui_selector_list(tx.clone());
                                } else if selected.name == ".." {
                                    if !path.is_empty() && path != "/" {
                                        if let Some(idx) = path.rfind('/') {
                                            path = path[..idx].to_string();
                                        } else {
                                            path = String::new();
                                        }
                                        if path.is_empty() && remote.is_empty() {
                                            path = String::new();
                                            remote = String::new();
                                        }
                                    } else {
                                        path = String::new();
                                        remote = String::new();
                                    }
                                    selected_idx = 0;
                                    scroll_offset = 0;
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::TuiExplorerSelector {
                                        archive_path, remote, path, items: Vec::new(), selected_idx, scroll_offset, loading: true
                                    };
                                    self.refresh_tui_selector_list(tx.clone());
                                } else if selected.is_dir {
                                    if path == "/" {
                                        if remote.is_empty() {
                                            path = format!("/{}", selected.name);
                                        } else {
                                            path = selected.name;
                                        }
                                    } else if path.is_empty() {
                                        path = selected.name;
                                    } else {
                                        path = format!("{}/{}", path, selected.name);
                                    }
                                    selected_idx = 0;
                                    scroll_offset = 0;
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::TuiExplorerSelector {
                                        archive_path, remote, path, items: Vec::new(), selected_idx, scroll_offset, loading: true
                                    };
                                    self.refresh_tui_selector_list(tx.clone());
                                }
                            }
                        }
                        KeyCode::Insert => {
                            let dest_path = if remote.is_empty() {
                                path.clone()
                            } else {
                                let clean_remote = remote.trim_end_matches(':');
                                let clean_path = if path.starts_with('/') {
                                    path.clone()
                                } else {
                                    format!("/{}", path)
                                };
                                format!("{}:{}", clean_remote, clean_path)
                            };
                            if !dest_path.is_empty() {
                                self.execute_archive_decompress(archive_path, dest_path, tx.clone()).await;
                            }
                        }
                        _ => {}
                    }
                }
                ui::explorer::ExplorerPopup::SpecialActionMessage { .. } => {
                    if key.code == KeyCode::Esc || key.code == KeyCode::Enter {
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                    }
                }
                ui::explorer::ExplorerPopup::InputPasteRename { mut input_buffer } => {
                    let mut cursor = self.explorer_state.edit_cursor_idx;
                    if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                        self.explorer_state.edit_cursor_idx = cursor;
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::InputPasteRename { input_buffer };
                    } else {
                        match key.code {
                            KeyCode::Esc => {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            }
                            KeyCode::Enter => {
                                let new_name = input_buffer.trim().to_string();
                                if !new_name.is_empty() {
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                                    if let Some(ref clipboard_item) = self.explorer_state.clipboard {
                                        let src = if clipboard_item.remote.is_empty() {
                                            PathBuf::from(&clipboard_item.path)
                                                .join(&clipboard_item.name)
                                                .to_string_lossy()
                                                .to_string()
                                        } else {
                                            format!("{}:{}/{}", clipboard_item.remote.trim_end_matches(':'), clipboard_item.path.trim_start_matches('/'), clipboard_item.name)
                                        };

                                        let dest_pane = self.explorer_state.get_active_pane();
                                        let dest = if dest_pane.remote.is_empty() {
                                            PathBuf::from(&dest_pane.path)
                                                .join(&new_name)
                                                .to_string_lossy()
                                                .to_string()
                                        } else {
                                            format!("{}:{}/{}", dest_pane.remote.trim_end_matches(':'), dest_pane.path.trim_start_matches('/'), new_name)
                                        };

                                        let is_dir = clipboard_item.is_dir;
                                        self.check_features_and_execute("copy", src, dest, is_dir, tx.clone());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Esc => {
                self.screen = Screen::MainMenu;
            }
            KeyCode::Tab => {
                self.explorer_state.toggle_pane();
            }
            KeyCode::Up => {
                self.explorer_state.get_active_pane_mut().prev();
            }
            KeyCode::Down => {
                self.explorer_state.get_active_pane_mut().next();
            }
            KeyCode::Enter => {
                // Vào thư mục con hoặc trở về thư mục cha nếu là ".."
                let active_pane = self.explorer_state.get_active_pane_mut();
                if !active_pane.items.is_empty() {
                    let selected = active_pane.items[active_pane.selected_idx].clone();
                    if selected.name == ".." {
                        let current_path = &active_pane.path;
                        if !current_path.is_empty() && current_path != "/" {
                            let parent = if let Some(idx) = current_path.rfind('/') {
                                current_path[..idx].to_string()
                            } else {
                                String::new()
                            };

                            active_pane.path = if parent.is_empty() && active_pane.remote.is_empty()
                            {
                                "/".to_string()
                            } else {
                                parent
                            };

                            active_pane.items.clear();
                            active_pane.selected_idx = 0;
                            active_pane.scroll_offset = 0;
                            active_pane.selected_names.clear();
                            active_pane.shift_anchor = None;
                            active_pane.shift_active = false;
                            active_pane.alt_anchor = None;
                            active_pane.alt_active = false;
                            let p_type = self.explorer_state.active_pane.clone();
                            self.refresh_explorer_pane(p_type, tx.clone()).await;
                        }
                    } else if selected.is_dir {
                        if active_pane.path == "/" {
                            if active_pane.remote.is_empty() {
                                active_pane.path = format!("/{}", selected.name);
                            } else {
                                active_pane.path = selected.name;
                            }
                        } else if active_pane.path.is_empty() {
                            active_pane.path = selected.name;
                        } else {
                            active_pane.path = format!("{}/{}", active_pane.path, selected.name);
                        }
                        active_pane.items.clear();
                        active_pane.selected_idx = 0;
                        active_pane.scroll_offset = 0;
                        active_pane.selected_names.clear();
                        active_pane.shift_anchor = None;
                        active_pane.shift_active = false;
                        active_pane.alt_anchor = None;
                        active_pane.alt_active = false;
                        let p_type = self.explorer_state.active_pane.clone();
                        self.refresh_explorer_pane(p_type, tx.clone()).await;
                    } else {
                        // Quick file preview (cat)
                        let name_lower = selected.name.to_lowercase();
                        let is_text = name_lower.ends_with(".txt")
                            || name_lower.ends_with(".log")
                            || name_lower.ends_with(".conf")
                            || name_lower.ends_with(".config")
                            || name_lower.ends_with(".json")
                            || name_lower.ends_with(".yaml")
                            || name_lower.ends_with(".yml")
                            || name_lower.ends_with(".toml")
                            || name_lower.ends_with(".sh")
                            || name_lower.ends_with(".py")
                            || name_lower.ends_with(".rs")
                            || name_lower.ends_with(".js")
                            || name_lower.ends_with(".ts")
                            || name_lower.ends_with(".md")
                            || name_lower.ends_with(".html")
                            || name_lower.ends_with(".css")
                            || name_lower.ends_with(".go")
                            || name_lower.ends_with(".c")
                            || name_lower.ends_with(".h")
                            || name_lower.ends_with(".cpp")
                            || name_lower.ends_with(".java")
                            || name_lower.ends_with(".properties")
                            || name_lower.ends_with(".env")
                            || name_lower.ends_with(".xml");
                        
                        if is_text && selected.size <= 2_000_000 {
                            let tx_clone = tx.clone();
                            let remote = active_pane.remote.clone();
                            let path = active_pane.path.clone();
                            let file_name = selected.name.clone();
                            
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::SpecialActionMessage {
                                title: "Đang tải file...".to_string(),
                                message: "Vui lòng chờ...".to_string(),
                            };

                            tokio::spawn(async move {
                                let (parent_fs, filename) = if remote.is_empty() {
                                    (path.clone(), file_name.clone())
                                } else {
                                    let clean_remote = remote.trim_end_matches(':');
                                    let clean_path = if path.starts_with('/') {
                                        path.clone()
                                    } else {
                                        format!("/{}", path)
                                    };
                                    (format!("{}:{}", clean_remote, clean_path), file_name.clone())
                                };
                                let param = json!({
                                    "fs": parent_fs,
                                    "remote": filename,
                                }).to_string();
                                
                                let res = rclone::rpc_async("operations/cat".to_string(), param).await;
                                match res {
                                    Ok(rpc_res) if rpc_res.status == 200 => {
                                        let lines: Vec<String> = rpc_res.output.lines().map(|s| s.to_string()).collect();
                                        let _ = tx_clone.send(AppEvent::FileViewLoaded {
                                            file_name,
                                            result: Ok(lines),
                                        });
                                    }
                                    Ok(rpc_res) => {
                                        let _ = tx_clone.send(AppEvent::FileViewLoaded {
                                            file_name,
                                            result: Err(format!("Status {}: {}", rpc_res.status, rpc_res.output)),
                                        });
                                    }
                                    Err(e) => {
                                        let _ = tx_clone.send(AppEvent::FileViewLoaded {
                                            file_name,
                                            result: Err(e),
                                        });
                                    }
                                }
                            });
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                // Quay lại thư mục cha
                let active_pane = self.explorer_state.get_active_pane_mut();
                let current_path = &active_pane.path;
                if !current_path.is_empty() && current_path != "/" {
                    let parent = if let Some(idx) = current_path.rfind('/') {
                        current_path[..idx].to_string()
                    } else {
                        String::new()
                    };

                    active_pane.path = if parent.is_empty() && active_pane.remote.is_empty() {
                        "/".to_string()
                    } else {
                        parent
                    };

                    active_pane.items.clear();
                    active_pane.selected_idx = 0;
                    active_pane.scroll_offset = 0;
                    active_pane.selected_names.clear();
                    active_pane.shift_anchor = None;
                    active_pane.shift_active = false;
                    active_pane.alt_anchor = None;
                    active_pane.alt_active = false;
                    let p_type = self.explorer_state.active_pane.clone();
                    self.refresh_explorer_pane(p_type, tx.clone()).await;
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') if key.modifiers.contains(KeyModifiers::ALT) || (cfg!(target_os = "macos") && key.modifiers.contains(KeyModifiers::CONTROL)) => {
                // Ctrl+R: Mở danh sách remote để lựa chọn cho pane hiện tại
                let mut remotes = vec!["[Local System]".to_string()];
                remotes.extend(self.connection_state.remotes.clone());
                self.explorer_state.popup = ui::explorer::ExplorerPopup::SelectRemote {
                    remotes,
                    selected_idx: 0,
                };
            }
            KeyCode::Char('n') | KeyCode::Char('N') if key.modifiers.contains(KeyModifiers::ALT) || (cfg!(target_os = "macos") && key.modifiers.contains(KeyModifiers::CONTROL)) => {
                self.explorer_state.edit_cursor_idx = 0;
                self.explorer_state.popup = ui::explorer::ExplorerPopup::InputNewFolder {
                    input_buffer: String::new(),
                };
            }
            KeyCode::Char('y') | KeyCode::Char('Y') if key.modifiers.contains(KeyModifiers::ALT) || (cfg!(target_os = "macos") && key.modifiers.contains(KeyModifiers::CONTROL)) => {
                // Ctrl+Y: Đổi tên tệp/thư mục
                let (name, is_dir) = {
                    let pane = self.explorer_state.get_active_pane();
                    if !pane.items.is_empty() {
                        let item = &pane.items[pane.selected_idx];
                        if item.name != ".." {
                            (Some(item.name.clone()), Some(item.is_dir))
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    }
                };
                if let (Some(name), Some(is_dir)) = (name, is_dir) {
                    self.explorer_state.edit_cursor_idx = name.chars().count();
                    self.explorer_state.popup = ui::explorer::ExplorerPopup::InputRename {
                        old_name: name.clone(),
                        input_buffer: name,
                        is_dir,
                    };
                }
            }
            KeyCode::Delete => {
                let pane = self.explorer_state.get_active_pane();
                if !pane.items.is_empty() {
                    // Nếu có nhiều mục được chọn, xóa tất cả
                    if !pane.selected_names.is_empty() {
                        let names: Vec<String> = pane.selected_names.iter().cloned().collect();
                        self.delete_confirm = Some(DeleteTarget::FileExplorerMultiple(names));
                    } else {
                        let item_name = pane.items[pane.selected_idx].name.clone();
                        if item_name != ".." {
                            self.delete_confirm = Some(DeleteTarget::FileExplorer(item_name));
                        }
                    }
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+A: Chọn tất cả (ngoại trừ "..")
                let pane = self.explorer_state.get_active_pane_mut();
                if pane.selected_names.len() == pane.items.iter().filter(|i| i.name != "..").count() {
                    // Nếu đã chọn hết thì bỏ chọn tất cả
                    pane.selected_names.clear();
                } else {
                    pane.selected_names.clear();
                    for item in &pane.items {
                        if item.name != ".." {
                            pane.selected_names.insert(item.name.clone());
                        }
                    }
                }
            }

            KeyCode::Char('V') | KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::SHIFT) && !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) => {
                // Shift+V: Chọn theo vùng dạng bôi đen/loại bỏ (range selection toggle)
                let pane = self.explorer_state.get_active_pane_mut();
                if let Some(anchor) = pane.shift_anchor {
                    if anchor == pane.selected_idx {
                        // Lần nhấn thứ 3 hoặc nhấn tại chỗ: Hủy bỏ neo
                        pane.shift_anchor = None;
                        pane.shift_active = false;
                    } else if !pane.shift_active {
                        // Lần nhấn thứ 2: Toggle từ anchor đến vị trí hiện tại
                        let start = anchor.min(pane.selected_idx);
                        let end = anchor.max(pane.selected_idx);
                        for i in start..=end {
                            if i < pane.items.len() && pane.items[i].name != ".." {
                                let name = pane.items[i].name.clone();
                                if pane.selected_names.contains(&name) {
                                    pane.selected_names.remove(&name);
                                } else {
                                    pane.selected_names.insert(name);
                                }
                            }
                        }
                        pane.shift_active = true;
                    } else {
                        // Lần nhấn thứ 3: Hủy bỏ neo
                        pane.shift_anchor = None;
                        pane.shift_active = false;
                    }
                } else {
                    // Lần nhấn thứ 1: Đặt anchor
                    pane.shift_anchor = Some(pane.selected_idx);
                    pane.shift_active = false;
                }
            }
            KeyCode::Char('V') | KeyCode::Char('v') | KeyCode::Char('d') | KeyCode::Char('D')
                if ((key.code == KeyCode::Char('V') || key.code == KeyCode::Char('v'))
                    && key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::SHIFT))
                    || ((key.code == KeyCode::Char('d') || key.code == KeyCode::Char('D'))
                        && cfg!(target_os = "macos")
                        && key.modifiers.contains(KeyModifiers::CONTROL)) => {
                // Alt+V: Toggle chọn 1 mục đơn lẻ (không di chuyển dòng)
                let pane = self.explorer_state.get_active_pane_mut();
                if !pane.items.is_empty() {
                    let idx = pane.selected_idx;
                    let name = pane.items[idx].name.clone();
                    if name != ".." {
                        if pane.selected_names.contains(&name) {
                            pane.selected_names.remove(&name);
                        } else {
                            pane.selected_names.insert(name);
                        }
                    }
                }
            }
            KeyCode::Char('c') | KeyCode::Char('C') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+C: Sao chép vào clipboard nội bộ (hỗ trợ multi-select)
                let pane = self.explorer_state.get_active_pane();
                if !pane.items.is_empty() {
                    if !pane.selected_names.is_empty() {
                        // Multi-select: sao chép tất cả các mục đã chọn
                        let items: Vec<ui::explorer::ClipboardItem> = pane.items.iter()
                            .filter(|item| pane.selected_names.contains(&item.name))
                            .map(|item| ui::explorer::ClipboardItem {
                                remote: pane.remote.clone(),
                                path: pane.path.clone(),
                                name: item.name.clone(),
                                is_dir: item.is_dir,
                            })
                            .collect();
                        self.explorer_state.clipboard_items = Some(items);
                        self.explorer_state.clipboard = None;
                    } else {
                        // Single select
                        let item = &pane.items[pane.selected_idx];
                        if item.name != ".." {
                            self.explorer_state.clipboard = Some(ui::explorer::ClipboardItem {
                                remote: pane.remote.clone(),
                                path: pane.path.clone(),
                                name: item.name.clone(),
                                is_dir: item.is_dir,
                            });
                            self.explorer_state.clipboard_items = None;
                        }
                    }
                }
            }
            KeyCode::Char('o') | KeyCode::Char('O') if key.modifiers.contains(KeyModifiers::ALT) || (cfg!(target_os = "macos") && key.modifiers.contains(KeyModifiers::CONTROL)) => {
                let pane = self.explorer_state.get_active_pane();
                if !pane.items.is_empty() {
                    let selected = &pane.items[pane.selected_idx];
                    if selected.name != ".." {
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::SpecialActionsMenu {
                            selected_idx: 0,
                        };
                    }
                }
            }
            KeyCode::Char('v') | KeyCode::Char('V') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+V: Dán (hỗ trợ multi-select)
                if let Some(ref items) = self.explorer_state.clipboard_items {
                    if !items.is_empty() {
                        // Multi-paste: Sao chép tuần tự tất cả các mục, bỏ qua popup đổi tên
                        let dest_pane = self.explorer_state.get_active_pane();
                        let dest_remote = dest_pane.remote.clone();
                        let dest_path = dest_pane.path.clone();
                        let items_clone = items.clone();
                        let tx_op = tx.clone();
                        let pane_type = self.explorer_state.active_pane.clone();

                        self.explorer_state.popup = ui::explorer::ExplorerPopup::CopyProgress {
                            src: format!("({} mục)", items_clone.len()),
                            dest: if dest_remote.is_empty() { dest_path.clone() } else { format!("{}:{}", dest_remote, dest_path) },
                            pct: 0.0,
                            job_id: None,
                        };

                        tokio::spawn(async move {
                            let total = items_clone.len();
                            let mut last_err = None;
                            for (idx, clip_item) in items_clone.iter().enumerate() {
                                let src = if clip_item.remote.is_empty() {
                                    PathBuf::from(&clip_item.path)
                                        .join(&clip_item.name)
                                        .to_string_lossy()
                                        .to_string()
                                } else {
                                    format!("{}:{}/{}", clip_item.remote.trim_end_matches(':'), clip_item.path.trim_start_matches('/'), clip_item.name)
                                };
                                let dest = if dest_remote.is_empty() {
                                    PathBuf::from(&dest_path)
                                        .join(&clip_item.name)
                                        .to_string_lossy()
                                        .to_string()
                                } else {
                                    format!("{}:{}/{}", dest_remote.trim_end_matches(':'), dest_path.trim_start_matches('/'), clip_item.name)
                                };

                                let pct = ((idx as f64) / total as f64) * 100.0;
                                let _ = tx_op.send(AppEvent::CopyProgress {
                                    src: format!("({}/{}) {}", idx + 1, total, clip_item.name),
                                    dest: dest.clone(),
                                    pct,
                                    job_id: None,
                                });

                                let method = if clip_item.is_dir {
                                    "sync/copy"
                                } else {
                                    "operations/copyfile"
                                };
                                let param = if clip_item.is_dir {
                                    json!({ "srcFs": src, "dstFs": dest })
                                } else {
                                    json!({ "srcFs": src.rsplit_once('/').map(|(p,_)| p).unwrap_or(&src), "srcRemote": clip_item.name, "dstFs": dest.rsplit_once('/').map(|(p,_)| p).unwrap_or(&dest), "dstRemote": clip_item.name })
                                };

                                let res = run_rpc_job_async(method.to_string(), param).await;
                                if let Err(e) = res {
                                    last_err = Some(e);
                                }
                            }
                            let _ = tx_op.send(AppEvent::CopyProgress {
                                src: format!("({} mục)", total),
                                dest: String::new(),
                                pct: 100.0,
                                job_id: None,
                            });
                            let result = match last_err {
                                None => Ok(()),
                                Some(e) => Err(e),
                            };
                            let _ = tx_op.send(AppEvent::ExplorerOperationFinished {
                                pane: pane_type,
                                op_name: "sao chép nhiều mục".to_string(),
                                result,
                            });
                        });
                    }
                } else if let Some(ref clipboard_item) = self.explorer_state.clipboard {
                    self.explorer_state.edit_cursor_idx = clipboard_item.name.chars().count();
                    self.explorer_state.popup = ui::explorer::ExplorerPopup::InputPasteRename {
                        input_buffer: clipboard_item.name.clone(),
                    };
                }
            }
            KeyCode::Char('x') | KeyCode::Char('X') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+X: Di chuyển sang pane đối diện (hỗ trợ multi-select)
                let src_pane = self.explorer_state.get_active_pane();
                if !src_pane.items.is_empty() {
                    if !src_pane.selected_names.is_empty() {
                        // Multi-move: Di chuyển tất cả mục đã chọn
                        let items: Vec<(String, bool)> = src_pane.items.iter()
                            .filter(|item| src_pane.selected_names.contains(&item.name))
                            .map(|item| (item.name.clone(), item.is_dir))
                            .collect();
                        let src_remote = src_pane.remote.clone();
                        let src_path = src_pane.path.clone();

                        let dest_pane = match self.explorer_state.active_pane {
                            ui::explorer::ActivePane::Left => &self.explorer_state.right_pane,
                            ui::explorer::ActivePane::Right => &self.explorer_state.left_pane,
                        };
                        let dest_remote = dest_pane.remote.clone();
                        let dest_path = dest_pane.path.clone();
                        let pane_type = self.explorer_state.active_pane.clone();
                        let tx_op = tx.clone();

                        self.explorer_state.popup = ui::explorer::ExplorerPopup::MoveProgress {
                            src: format!("({} mục)", items.len()),
                            dest: if dest_remote.is_empty() { dest_path.clone() } else { format!("{}:{}", dest_remote, dest_path) },
                            pct: 0.0,
                            job_id: None,
                        };

                        // Xoá selection sau khi bắt đầu di chuyển
                        self.explorer_state.get_active_pane_mut().selected_names.clear();

                        tokio::spawn(async move {
                            let total = items.len();
                            let mut last_err = None;
                            for (idx, (item_name, is_dir)) in items.iter().enumerate() {
                                let src = if src_remote.is_empty() {
                                    PathBuf::from(&src_path)
                                        .join(item_name)
                                        .to_string_lossy()
                                        .to_string()
                                } else {
                                    format!("{}:{}/{}", src_remote.trim_end_matches(':'), src_path.trim_start_matches('/'), item_name)
                                };
                                let dest = if dest_remote.is_empty() {
                                    PathBuf::from(&dest_path)
                                        .join(item_name)
                                        .to_string_lossy()
                                        .to_string()
                                } else {
                                    format!("{}:{}/{}", dest_remote.trim_end_matches(':'), dest_path.trim_start_matches('/'), item_name)
                                };

                                let pct = ((idx as f64) / total as f64) * 100.0;
                                let _ = tx_op.send(AppEvent::MoveProgress {
                                    src: format!("({}/{}) {}", idx + 1, total, item_name),
                                    dest: dest.clone(),
                                    pct,
                                    job_id: None,
                                });

                                let method = if *is_dir {
                                    "sync/move"
                                } else {
                                    "operations/movefile"
                                };
                                let param = if *is_dir {
                                    json!({ "srcFs": src, "dstFs": dest })
                                } else {
                                    json!({ "srcFs": src.rsplit_once('/').map(|(p,_)| p).unwrap_or(&src), "srcRemote": item_name, "dstFs": dest.rsplit_once('/').map(|(p,_)| p).unwrap_or(&dest), "dstRemote": item_name })
                                };

                                let res = run_rpc_job_async(method.to_string(), param).await;
                                if let Err(e) = res {
                                    last_err = Some(e);
                                }
                            }
                            let _ = tx_op.send(AppEvent::MoveProgress {
                                src: format!("({} mục)", total),
                                dest: String::new(),
                                pct: 100.0,
                                job_id: None,
                            });
                            let result = match last_err {
                                None => Ok(()),
                                Some(e) => Err(e),
                            };
                            let _ = tx_op.send(AppEvent::ExplorerOperationFinished {
                                pane: pane_type,
                                op_name: "di chuyển nhiều mục".to_string(),
                                result,
                            });
                        });
                    } else {
                        let item = &src_pane.items[src_pane.selected_idx];
                        if item.name == ".." {
                            return;
                        }
                        let is_dir = item.is_dir;
                        let src = if src_pane.remote.is_empty() {
                            PathBuf::from(&src_pane.path)
                                .join(&item.name)
                                .to_string_lossy()
                                .to_string()
                        } else {
                            format!("{}:{}/{}", src_pane.remote.trim_end_matches(':'), src_pane.path.trim_start_matches('/'), item.name)
                        };

                        let dest_pane = match self.explorer_state.active_pane {
                            ui::explorer::ActivePane::Left => &self.explorer_state.right_pane,
                            ui::explorer::ActivePane::Right => &self.explorer_state.left_pane,
                        };
                        let dest = if dest_pane.remote.is_empty() {
                            PathBuf::from(&dest_pane.path)
                                .join(&item.name)
                                .to_string_lossy()
                                .to_string()
                        } else {
                            format!("{}:{}/{}", dest_pane.remote.trim_end_matches(':'), dest_pane.path.trim_start_matches('/'), item.name)
                        };

                        self.check_features_and_execute("move", src, dest, is_dir, tx.clone());
                    }
                }
            }
            KeyCode::Char('t') | KeyCode::Char('T') if key.modifiers.contains(KeyModifiers::ALT) || (cfg!(target_os = "macos") && key.modifiers.contains(KeyModifiers::CONTROL)) => {
                // Ctrl+T: Đồng bộ
                self.explorer_state.popup = ui::explorer::ExplorerPopup::SyncConfirm;
            }
            KeyCode::Char(' ') => {
                // Space: Xoá clipboard và selection
                self.explorer_state.clipboard = None;
                self.explorer_state.clipboard_items = None;
                let pane = self.explorer_state.get_active_pane_mut();
                pane.selected_names.clear();
                pane.shift_anchor = None;
                pane.shift_active = false;
                pane.alt_anchor = None;
                pane.alt_active = false;
            }
            _ => {}
        }
    }

    async fn handle_monitor_key(
        &mut self,
        key: KeyEvent,
        _tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        if self.monitor_state.confirm_stop_job.is_some() {
            match key.code {
                KeyCode::Esc => {
                    self.monitor_state.confirm_stop_job = None;
                }
                KeyCode::Enter => {
                    if let Some(job) = self.monitor_state.confirm_stop_job.take() {
                        let op_res = if let Some(id) = job.job_id {
                            let param = json!({ "jobid": id }).to_string();
                            rclone::rpc("job/stop", &param)
                        } else {
                            let param = json!({ "group": job.name }).to_string();
                            rclone::rpc("job/stopgroup", &param)
                        };
                        let msg = match op_res {
                            Ok(_) => format!("Đã yêu cầu hủy bỏ tác vụ: {}", job.name),
                            Err(e) => format!("Lỗi khi hủy tác vụ: {}", e),
                        };
                        self.monitor_state.history.push(msg);
                    }
                }
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Esc => {
                    self.screen = Screen::MainMenu;
                }
                KeyCode::Up => self.monitor_state.prev(),
                KeyCode::Down => self.monitor_state.next(),
                KeyCode::Delete => {
                    if !self.monitor_state.active_jobs.is_empty() {
                        if self.monitor_state.selected_job_idx < self.monitor_state.active_jobs.len() {
                            let job = self.monitor_state.active_jobs[self.monitor_state.selected_job_idx].clone();
                            self.monitor_state.confirm_stop_job = Some(job);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn load_profile_list(&mut self) {
        let mut list = Vec::new();
        for (name, path) in &self.config.profiles {
            list.push((name.clone(), path.clone()));
        }
        list.sort_by(|a, b| a.0.cmp(&b.0));
        self.profile_state.profiles = list;
    }

    async fn handle_profile_key(
        &mut self,
        key: KeyEvent,
        _tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        if self.profile_state.export_popup != ui::profile::ExportPopupState::None {
            if key.code == KeyCode::Enter || key.code == KeyCode::Esc {
                if let ui::profile::ExportPopupState::ConfirmOverwrite { profile_name } =
                    &self.profile_state.export_popup
                {
                    if key.code == KeyCode::Enter {
                        // Xác nhận ghi đè export
                        let res = self.config.export_profile(profile_name, true);
                        if let ExportResult::Success(path) = res {
                            self.profile_state.export_popup =
                                ui::profile::ExportPopupState::Success {
                                    path: path.to_string_lossy().to_string(),
                                };
                        }
                    } else {
                        self.profile_state.export_popup = ui::profile::ExportPopupState::None;
                    }
                } else {
                    self.profile_state.export_popup = ui::profile::ExportPopupState::None;
                }
            }
            return;
        }

        let wizard = self.profile_state.wizard.clone();
        match wizard {
            ui::profile::ImportWizardState::None => {
                match key.code {
                    KeyCode::Esc => {
                        self.screen = Screen::MainMenu;
                    }
                    KeyCode::Up => self.profile_state.prev(),
                    KeyCode::Down => self.profile_state.next(),
                    KeyCode::Enter => {
                        // Chọn kích hoạt profile cấu hình nóng
                        if !self.profile_state.profiles.is_empty() {
                            let name = self.profile_state.profiles[self.profile_state.selected_idx]
                                .0
                                .clone();
                            self.config.active_profile = name;
                            let _ = self.config.save();

                            // Cập nhật biến môi trường cho Go core
                            let path = self.config.get_active_profile_path();
                            unsafe {
                                std::env::set_var("RCLONE_CONFIG", &path);
                            }
                            let _ =
                                rclone::rpc("config/setpath", &json!({"path": path}).to_string());
                        }
                    }
                    KeyCode::Char('x') | KeyCode::Char('X') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // Xuất Profile
                        if !self.profile_state.profiles.is_empty() {
                            let name = self.profile_state.profiles[self.profile_state.selected_idx]
                                .0
                                .clone();
                            let res = self.config.export_profile(&name, false);
                            match res {
                                ExportResult::Success(path) => {
                                    self.profile_state.export_popup =
                                        ui::profile::ExportPopupState::Success {
                                            path: path.to_string_lossy().to_string(),
                                        };
                                }
                                ExportResult::AlreadyExists(_) => {
                                    self.profile_state.export_popup =
                                        ui::profile::ExportPopupState::ConfirmOverwrite {
                                            profile_name: name,
                                        };
                                }
                                ExportResult::SourceNotFound => {
                                    self.profile_state.error_message =
                                        Some("Không tìm thấy tệp cấu hình nguồn.".to_string());
                                }
                                ExportResult::Error(e) => {
                                    self.profile_state.error_message = Some(e);
                                }
                            }
                        }
                    }
                    KeyCode::Insert => {
                        // Thêm Profile mới
                        self.profile_state.wizard =
                            ui::profile::ImportWizardState::InputProfileName {
                                input_buffer: String::new(),
                            };
                    }
                    _ => {}
                }
            }
            ui::profile::ImportWizardState::InputProfileName { mut input_buffer } => {
                match key.code {
                    KeyCode::Esc => {
                        self.profile_state.wizard = ui::profile::ImportWizardState::None;
                    }
                    KeyCode::Char(c) => {
                        input_buffer.push(c);
                        self.profile_state.wizard =
                            ui::profile::ImportWizardState::InputProfileName { input_buffer };
                    }
                    KeyCode::Backspace => {
                        input_buffer.pop();
                        self.profile_state.wizard =
                            ui::profile::ImportWizardState::InputProfileName { input_buffer };
                    }
                    KeyCode::Enter => {
                        let name = input_buffer.trim().to_string();
                        if !name.is_empty() {
                            self.profile_state.wizard =
                                ui::profile::ImportWizardState::SelectImportType {
                                    profile_name: name,
                                    selected_idx: 0,
                                };
                        }
                    }
                    _ => {}
                }
            }
            ui::profile::ImportWizardState::SelectImportType {
                profile_name,
                mut selected_idx,
            } => match key.code {
                KeyCode::Esc => {
                    self.profile_state.wizard = ui::profile::ImportWizardState::None;
                }
                KeyCode::Up | KeyCode::Down | KeyCode::Tab => {
                    selected_idx = if selected_idx == 0 { 1 } else { 0 };
                    self.profile_state.wizard = ui::profile::ImportWizardState::SelectImportType {
                        profile_name,
                        selected_idx,
                    };
                }
                KeyCode::Enter => {
                    self.profile_state.wizard = ui::profile::ImportWizardState::InputSource {
                        profile_name: profile_name.clone(),
                        import_type: selected_idx,
                        input_buffer: String::new(),
                    };
                }
                _ => {}
            },
            ui::profile::ImportWizardState::InputSource {
                profile_name,
                import_type,
                mut input_buffer,
            } => match key.code {
                KeyCode::Esc => {
                    self.profile_state.wizard = ui::profile::ImportWizardState::None;
                }
                KeyCode::Char(c) => {
                    input_buffer.push(c);
                    self.profile_state.wizard = ui::profile::ImportWizardState::InputSource {
                        profile_name,
                        import_type,
                        input_buffer,
                    };
                }
                KeyCode::Backspace => {
                    input_buffer.pop();
                    self.profile_state.wizard = ui::profile::ImportWizardState::InputSource {
                        profile_name,
                        import_type,
                        input_buffer,
                    };
                }
                KeyCode::Enter => {
                    let src = input_buffer.trim().to_string();
                    if !src.is_empty() {
                        let already_exists = self.config.profiles.contains_key(&profile_name);
                        if already_exists {
                            self.profile_state.wizard =
                                ui::profile::ImportWizardState::ConfirmImportOverwrite {
                                    profile_name: profile_name.clone(),
                                    source_path_or_url: src,
                                    import_type,
                                };
                        } else {
                            self.execute_import_profile(profile_name.clone(), src, import_type);
                        }
                    }
                }
                _ => {}
            },
            ui::profile::ImportWizardState::ConfirmImportOverwrite {
                profile_name,
                source_path_or_url,
                import_type,
            } => match key.code {
                KeyCode::Esc => {
                    self.profile_state.wizard = ui::profile::ImportWizardState::None;
                }
                KeyCode::Enter => {
                    self.execute_import_profile(
                        profile_name.clone(),
                        source_path_or_url.clone(),
                        import_type,
                    );
                }
                _ => {}
            },
        }
    }

    fn execute_import_profile(&mut self, name: String, src: String, import_type: usize) {
        let dest_path = AppConfig::config_dir().join(format!("{}.config", name));

        if import_type == 1 {
            // Local path copy
            if Path::new(&src).exists() {
                if fs::copy(&src, &dest_path).is_ok() {
                    self.config
                        .profiles
                        .insert(name, dest_path.to_string_lossy().to_string());
                    let _ = self.config.save();
                    self.load_profile_list();
                    self.profile_state.wizard = ui::profile::ImportWizardState::None;
                } else {
                    self.profile_state.error_message =
                        Some("Lỗi sao chép tệp cấu hình.".to_string());
                }
            } else {
                self.profile_state.error_message =
                    Some("Đường dẫn local không tồn tại.".to_string());
            }
        } else {
            // URL Download (giả lập tải xuống nhanh bằng wget/curl)
            let output = Command::new("curl")
                .args(["-o", &dest_path.to_string_lossy(), &src])
                .output();

            if output.is_ok() && output.unwrap().status.success() {
                self.config
                    .profiles
                    .insert(name, dest_path.to_string_lossy().to_string());
                let _ = self.config.save();
                self.load_profile_list();
                self.profile_state.wizard = ui::profile::ImportWizardState::None;
            } else {
                self.profile_state.error_message =
                    Some("Tải cấu hình từ URL thất bại.".to_string());
            }
        }
    }

    async fn handle_services_key(
        &mut self,
        key: KeyEvent,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        let wizard = self.services_state.wizard.clone();
        match wizard {
            ui::services::ServicesWizardState::None => {
                match key.code {
                    KeyCode::Esc => {
                        self.screen = Screen::MainMenu;
                    }
                    KeyCode::Tab => {
                        let limit = if cfg!(target_os = "windows") { 2 } else { 3 };
                        self.services_state.active_focus = (self.services_state.active_focus + 1) % limit;
                    }
                    KeyCode::Up => {
                        match self.services_state.active_focus {
                            0 => self.services_state.prev_menu(),
                            1 => self.services_state.prev_active(),
                            _ => self.services_state.prev_systemd(),
                        }
                    }
                    KeyCode::Down => {
                        match self.services_state.active_focus {
                            0 => self.services_state.next_menu(),
                            1 => self.services_state.next_active(),
                            _ => self.services_state.next_systemd(),
                        }
                    }
                    KeyCode::Enter => {
                        if self.services_state.active_focus == 0 {
                            // Khởi động wizard tạo dịch vụ mới
                            let service_type = match self.services_state.selected_menu_idx {
                                0 => ui::services::ServiceType::Mount,
                                1 => ui::services::ServiceType::NfsMount,
                                2 => ui::services::ServiceType::WebGui,
                                3 => ui::services::ServiceType::Serve,
                                _ => return,
                            };

                            // Web GUI không cần chọn Remote, chuyển thẳng sang hỏi flags
                            if service_type == ui::services::ServiceType::WebGui {
                                self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
                                    service_type,
                                    remote: String::new(),
                                    path: String::new(),
                                    protocol: None,
                                    flags: vec![
                                        ("--rc-addr".to_string(), "Nhập địa chỉ cổng Web GUI (Mặc định: localhost:5572)".to_string(), "localhost:5572".to_string(), String::new()),
                                        ("--rc-no-auth".to_string(), "Bạn có muốn bỏ qua bảo mật đăng nhập Web GUI? (y/N)".to_string(), "n".to_string(), String::new()),
                                    ],
                                    current_flag_idx: 0,
                                    input_buffer: String::new(),
                                    is_simple_terminal: false,
                                    is_editing: false,
                                };
                            } else if service_type == ui::services::ServiceType::Mount || service_type == ui::services::ServiceType::NfsMount {
                                // Chọn chế độ Simple hay Advanced
                                self.services_state.wizard =
                                    ui::services::ServicesWizardState::AskMode {
                                        service_type,
                                        selected_idx: 0,
                                    };
                            } else {
                                // Cần chọn Remote cho Serve
                                self.services_state.wizard =
                                    ui::services::ServicesWizardState::SelectRemote {
                                        service_type,
                                        remotes: self.connection_state.remotes.clone(),
                                        selected_idx: 0,
                                        is_simple_terminal: false,
                                        is_simple_gui: false,
                                    };
                            }
                        } else if self.services_state.active_focus == 2 {
                            if !self.services_state.systemd_services.is_empty() {
                                let idx = self.services_state.selected_systemd_idx;
                                let svc = &self.services_state.systemd_services[idx];
                                match self.load_systemd_service_fields(&svc.file_path, svc.is_user) {
                                    Ok(fields) => {
                                        self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                            service_name: svc.name.clone(),
                                            file_path: svc.file_path.clone(),
                                            is_user: svc.is_user,
                                            fields,
                                            selected_idx: 0,
                                            scroll_offset: 0,
                                            is_editing: false,
                                            input_buffer: String::new(),
                                            active_tab: 0,
                                            adding_new_key: false,
                                            new_key_buffer: String::new(),
                                        };
                                    }
                                    Err(e) => {
                                        self.services_state.error_message = Some(format!(
                                            "Lỗi đọc file cấu hình: {}", e
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char(' ') => {
                        if self.services_state.active_focus == 2 {
                            if !self.services_state.systemd_services.is_empty() {
                                let idx = self.services_state.selected_systemd_idx;
                                let svc = &self.services_state.systemd_services[idx];
                                self.services_state.wizard = ui::services::ServicesWizardState::SelectSystemdAction {
                                    service_name: svc.name.clone(),
                                    file_path: svc.file_path.clone(),
                                    is_user: svc.is_user,
                                    selected_idx: 0,
                                };
                            }
                        }
                    }
                    KeyCode::Insert => {
                        if self.services_state.active_focus == 2 {
                            let fields = self.init_create_systemd_service_fields();
                            self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                fields,
                                selected_idx: 0,
                                scroll_offset: 0,
                                is_editing: false,
                                input_buffer: String::new(),
                                active_tab: 0,
                                adding_new_key: false,
                                new_key_buffer: String::new(),
                            };
                        }
                    }
                    KeyCode::Delete => {
                        if self.services_state.active_focus == 1 {
                            if !self.services_state.active_services.is_empty() {
                                let idx = self.services_state.selected_active_idx;
                                self.delete_confirm = Some(DeleteTarget::Service(idx));
                            }
                        } else if self.services_state.active_focus == 2 {
                            if !self.services_state.systemd_services.is_empty() {
                                let idx = self.services_state.selected_systemd_idx;
                                self.delete_confirm = Some(DeleteTarget::SystemdService(idx));
                            }
                        }
                    }
                    _ => {}
                }
            }
            ui::services::ServicesWizardState::AskMode {
                service_type,
                mut selected_idx,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.services_state.wizard = ui::services::ServicesWizardState::None;
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = 2;
                        } else {
                            selected_idx -= 1;
                        }
                        self.services_state.wizard =
                            ui::services::ServicesWizardState::AskMode {
                                service_type,
                                selected_idx,
                            };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % 3;
                        self.services_state.wizard =
                            ui::services::ServicesWizardState::AskMode {
                                service_type,
                                selected_idx,
                            };
                    }
                    KeyCode::Enter => {
                        let is_simple_terminal = selected_idx == 0;
                        let is_simple_gui = selected_idx == 1;

                        // Tìm remote Secret để preselect nếu có
                        let mut default_idx = 0;
                        for (i, r) in self.connection_state.remotes.iter().enumerate() {
                            if r.to_lowercase() == "secret" {
                                default_idx = i + 1;
                                break;
                            }
                        }

                        self.services_state.wizard =
                            ui::services::ServicesWizardState::SelectRemote {
                                service_type,
                                remotes: self.connection_state.remotes.clone(),
                                selected_idx: default_idx,
                                is_simple_terminal,
                                is_simple_gui,
                            };
                    }
                    _ => {}
                }
            }
            ui::services::ServicesWizardState::SelectRemote {
                service_type,
                remotes,
                mut selected_idx,
                is_simple_terminal,
                is_simple_gui,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.services_state.wizard = ui::services::ServicesWizardState::None;
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = remotes.len();
                        } else {
                            selected_idx -= 1;
                        }
                        self.services_state.wizard =
                            ui::services::ServicesWizardState::SelectRemote {
                                service_type,
                                remotes,
                                selected_idx,
                                is_simple_terminal,
                                is_simple_gui,
                            };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % (remotes.len() + 1);
                        self.services_state.wizard =
                            ui::services::ServicesWizardState::SelectRemote {
                                service_type,
                                remotes,
                                selected_idx,
                                is_simple_terminal,
                                is_simple_gui,
                            };
                    }
                    KeyCode::Enter => {
                        let remote = if selected_idx == 0 {
                            String::new() // Local
                        } else {
                            format!("{}:", remotes[selected_idx - 1])
                        };

                        if is_simple_gui {
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote: remote.clone(),
                                current_path: "/".to_string(),
                                items: Vec::new(),
                                selected_idx: 0,
                                loading: true,
                                error_msg: None,
                                creating_folder: None,
                            };
                            self.refresh_wizard_gui_list(tx).await;
                        } else {
                            let default_path = if remote.to_lowercase().starts_with("secret:") {
                                "Khobaomat".to_string()
                            } else {
                                String::new()
                            };
                            self.services_state.edit_cursor_idx = default_path.chars().count();
                            self.services_state.wizard = ui::services::ServicesWizardState::InputPath {
                                service_type,
                                remote,
                                input_buffer: default_path,
                                is_simple_terminal,
                            };
                        }
                    }
                    _ => {}
                }
            }
            ui::services::ServicesWizardState::InputPath {
                service_type,
                remote,
                mut input_buffer,
                is_simple_terminal,
            } => {
                let mut cursor = self.services_state.edit_cursor_idx;
                if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                    self.services_state.edit_cursor_idx = cursor;
                    self.services_state.wizard = ui::services::ServicesWizardState::InputPath {
                        service_type,
                        remote,
                        input_buffer,
                        is_simple_terminal,
                    };
                } else {
                    match key.code {
                        KeyCode::Esc => {
                            self.services_state.wizard = ui::services::ServicesWizardState::None;
                        }
                        KeyCode::Enter => {
                        let path = input_buffer.trim().to_string();

                        if service_type == ui::services::ServiceType::Mount || service_type == ui::services::ServiceType::NfsMount {
                            let flags = if cfg!(target_os = "windows") {
                                if is_simple_terminal {
                                    vec![
                                        ("mount_point".to_string(), "Nhập ổ đĩa mount cục bộ (Mặc định: X:)".to_string(), "X:".to_string(), String::new()),
                                    ]
                                } else {
                                    vec![
                                        ("mount_point".to_string(), "Nhập ổ đĩa mount cục bộ (Ví dụ: X: hoặc Y:)".to_string(), "X:".to_string(), String::new()),
                                        ("--vfs-cache-mode".to_string(), "Chế độ cache VFS (off, minimal, writes, full - Mặc định: writes)".to_string(), "writes".to_string(), String::new()),
                                        ("--dir-cache-time".to_string(), "Thời gian cache thư mục (Ví dụ: 5m, 1h - Mặc định: 5m)".to_string(), "5m".to_string(), String::new()),
                                        ("--read-only".to_string(), "Giới hạn ổ đĩa chỉ đọc (Read Only)? (y/N)".to_string(), "n".to_string(), String::new()),
                                    ]
                                }
                            } else {
                                if is_simple_terminal {
                                    vec![
                                        ("mount_point".to_string(), "Nhập thư mục mount cục bộ (Mặc định: /mnt/drive)".to_string(), "/mnt/drive".to_string(), String::new()),
                                    ]
                                } else {
                                    vec![
                                        ("mount_point".to_string(), "Nhập thư mục mount cục bộ (Ví dụ: /mnt/drive hoặc ~/Cloud)".to_string(), "/mnt/drive".to_string(), String::new()),
                                        ("--vfs-cache-mode".to_string(), "Chế độ cache VFS (off, minimal, writes, full - Mặc định: writes)".to_string(), "writes".to_string(), String::new()),
                                        ("--dir-cache-time".to_string(), "Thời gian cache thư mục (Ví dụ: 5m, 1h - Mặc định: 5m)".to_string(), "5m".to_string(), String::new()),
                                        ("--read-only".to_string(), "Giới hạn ổ đĩa chỉ đọc (Read Only)? (y/N)".to_string(), "n".to_string(), String::new()),
                                        ("--daemon".to_string(), "Chạy ngầm tiến trình mount dưới nền? (Y/n)".to_string(), "y".to_string(), String::new()),
                                    ]
                                }
                            };
                            // Chuyển sang hỏi Flags cho Mount
                            self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
                                service_type,
                                remote,
                                path,
                                protocol: None,
                                flags,
                                current_flag_idx: 0,
                                input_buffer: String::new(),
                                is_simple_terminal,
                                is_editing: false,
                            };
                        } else {
                            // Chia sẻ mạng: Chọn giao thức trước
                            self.services_state.wizard =
                                ui::services::ServicesWizardState::SelectProtocol {
                                    remote,
                                    path,
                                    selected_idx: 0,
                                };
                        }
                    }
                    _ => {}
                }
              }
            }
            ui::services::ServicesWizardState::GuiSelectPath {
                service_type,
                remote,
                mut current_path,
                items,
                mut selected_idx,
                loading,
                error_msg,
                creating_folder,
            } => {
                if let Some(mut input_buffer) = creating_folder {
                    match key.code {
                        KeyCode::Esc => {
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: None,
                            };
                        }
                        KeyCode::Char(c) => {
                            input_buffer.push(c);
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(input_buffer),
                            };
                        }
                        KeyCode::Backspace => {
                            input_buffer.pop();
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(input_buffer),
                            };
                        }
                        KeyCode::Enter => {
                            let folder_name = input_buffer.trim().to_string();
                            if !folder_name.is_empty() {
                                let is_local = remote.is_empty();
                                let parent_fs = if is_local {
                                    current_path.clone()
                                } else {
                                    format!("{}{}", remote, current_path)
                                };
                                let fs_target = if parent_fs.ends_with('/') {
                                    format!("{}{}", parent_fs, folder_name)
                                } else {
                                    format!("{}/{}", parent_fs, folder_name)
                                };

                                let param = if is_local {
                                    serde_json::json!({
                                        "fs": fs_target,
                                        "remote": "",
                                    })
                                } else {
                                    serde_json::json!({
                                        "fs": parent_fs,
                                        "remote": folder_name,
                                    })
                                }
                                .to_string();

                                let tx_clone = tx.clone();
                                tokio::spawn(async move {
                                    if is_local {
                                        if std::fs::create_dir_all(&fs_target).is_err() {
                                            let _ = std::process::Command::new("pkexec")
                                                .args(&["mkdir", "-p", &fs_target])
                                                .status();
                                        }
                                    } else {
                                        let _ = rclone::rpc_async("operations/mkdir".to_string(), param).await;
                                    }
                                    let _ = tx_clone.send(AppEvent::WizardGuiRefresh);
                                });
                            }
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items,
                                selected_idx,
                                loading: true,
                                error_msg,
                                creating_folder: None,
                            };
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Esc => {
                            self.services_state.wizard = ui::services::ServicesWizardState::None;
                        }
                        KeyCode::Up => {
                            if !items.is_empty() {
                                if selected_idx == 0 {
                                    selected_idx = items.len() - 1;
                                } else {
                                    selected_idx -= 1;
                                }
                                self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                    service_type,
                                    remote,
                                    current_path,
                                    items,
                                    selected_idx,
                                    loading,
                                    error_msg,
                                    creating_folder: None,
                                };
                            }
                        }
                        KeyCode::Down => {
                            if !items.is_empty() {
                                selected_idx = (selected_idx + 1) % items.len();
                                self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                    service_type,
                                    remote,
                                    current_path,
                                    items,
                                    selected_idx,
                                    loading,
                                    error_msg,
                                    creating_folder: None,
                                };
                            }
                        }
                        KeyCode::Enter => {
                            if !items.is_empty() && selected_idx < items.len() {
                                let name = &items[selected_idx].name;
                                if name == ".." {
                                    if current_path != "/" && !current_path.is_empty() {
                                        let mut parts: Vec<&str> = current_path.split('/').filter(|s| !s.is_empty()).collect();
                                        if !parts.is_empty() {
                                            parts.pop();
                                        }
                                        current_path = if parts.is_empty() {
                                            "/".to_string()
                                        } else {
                                            format!("/{}", parts.join("/"))
                                        };
                                    }
                                } else {
                                    current_path = if current_path == "/" {
                                        format!("/{}", name)
                                    } else {
                                        format!("{}/{}", current_path, name)
                                    };
                                }
                                self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                    service_type,
                                    remote,
                                    current_path,
                                    items: Vec::new(),
                                    selected_idx: 0,
                                    loading: true,
                                    error_msg: None,
                                    creating_folder: None,
                                };
                                self.refresh_wizard_gui_list(tx.clone()).await;
                            }
                        }
                        KeyCode::Insert => {
                            let clean_path = if remote.is_empty() {
                                current_path.clone()
                            } else if current_path == "/" {
                                "/".to_string()
                            } else {
                                current_path.trim_start_matches('/').to_string()
                            };

                            if service_type == ui::services::ServiceType::Mount || service_type == ui::services::ServiceType::NfsMount {
                                self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
                                    service_type,
                                    remote: remote.clone(),
                                    remote_path: clean_path,
                                    current_path: "/".to_string(),
                                    items: Vec::new(),
                                    selected_idx: 0,
                                    loading: true,
                                    error_msg: None,
                                    creating_folder: None,
                                };
                                self.refresh_wizard_gui_list(tx.clone()).await;
                            } else {
                                let flags = vec![
                                    ("mount_point".to_string(), "Nhập thư mục mount cục bộ (Mặc định: /mnt/drive)".to_string(), "/mnt/drive".to_string(), String::new()),
                                ];
                                self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
                                    service_type,
                                    remote,
                                    path: clean_path,
                                    protocol: None,
                                    flags,
                                    current_flag_idx: 0,
                                    input_buffer: String::new(),
                                    is_simple_terminal: true,
                                    is_editing: false,
                                };
                            }
                        }
                        KeyCode::F(7) => {
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        KeyCode::Char('n') if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) => {
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        KeyCode::Char('N') if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) => {
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        KeyCode::Char('N') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        KeyCode::Char('n') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT) => {
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        _ => {}
                    }
                }
            }
            ui::services::ServicesWizardState::GuiSelectLocalPath {
                service_type,
                remote,
                remote_path,
                mut current_path,
                items,
                mut selected_idx,
                loading,
                error_msg,
                creating_folder,
            } => {
                let tx_clone = tx.clone();
                if let Some(mut buf) = creating_folder {
                    match key.code {
                        KeyCode::Esc => {
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
                                service_type,
                                remote,
                                remote_path,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: None,
                            };
                        }
                        KeyCode::Char(c) => {
                            buf.push(c);
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
                                service_type,
                                remote,
                                remote_path,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(buf),
                            };
                        }
                        KeyCode::Backspace => {
                            buf.pop();
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
                                service_type,
                                remote,
                                remote_path,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(buf),
                            };
                        }
                        KeyCode::Enter => {
                            if !buf.trim().is_empty() {
                                let folder_name = buf.trim().to_string();
                                let fs_target = if current_path == "/" {
                                    format!("/{}", folder_name)
                                } else {
                                    format!("{}/{}", current_path, folder_name)
                                };
                                tokio::spawn(async move {
                                    if std::fs::create_dir_all(&fs_target).is_err() {
                                        let _ = std::process::Command::new("pkexec")
                                            .args(&["mkdir", "-p", &fs_target])
                                            .status();
                                    }
                                    let _ = tx_clone.send(AppEvent::WizardGuiRefresh);
                                });
                            }
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
                                service_type,
                                remote,
                                remote_path,
                                current_path,
                                items,
                                selected_idx,
                                loading: true,
                                error_msg,
                                creating_folder: None,
                            };
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Esc => {
                            self.services_state.wizard = ui::services::ServicesWizardState::None;
                        }
                        KeyCode::Up => {
                            if !items.is_empty() {
                                if selected_idx == 0 {
                                    selected_idx = items.len() - 1;
                                } else {
                                    selected_idx -= 1;
                                }
                                self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
                                    service_type,
                                    remote,
                                    remote_path,
                                    current_path,
                                    items,
                                    selected_idx,
                                    loading,
                                    error_msg,
                                    creating_folder: None,
                                };
                            }
                        }
                        KeyCode::Down => {
                            if !items.is_empty() {
                                selected_idx = (selected_idx + 1) % items.len();
                                self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
                                    service_type,
                                    remote,
                                    remote_path,
                                    current_path,
                                    items,
                                    selected_idx,
                                    loading,
                                    error_msg,
                                    creating_folder: None,
                                };
                            }
                        }
                        KeyCode::Enter => {
                            if !items.is_empty() && selected_idx < items.len() {
                                let name = &items[selected_idx].name;
                                if name == ".." {
                                    if current_path != "/" && !current_path.is_empty() {
                                        let mut parts: Vec<&str> = current_path.split('/').filter(|s| !s.is_empty()).collect();
                                        if !parts.is_empty() {
                                            parts.pop();
                                        }
                                        current_path = if parts.is_empty() {
                                            "/".to_string()
                                        } else {
                                            format!("/{}", parts.join("/"))
                                        };
                                    }
                                } else {
                                    current_path = if current_path == "/" {
                                        format!("/{}", name)
                                    } else {
                                        format!("{}/{}", current_path, name)
                                    };
                                }
                                self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
                                    service_type,
                                    remote,
                                    remote_path,
                                    current_path,
                                    items: Vec::new(),
                                    selected_idx: 0,
                                    loading: true,
                                    error_msg: None,
                                    creating_folder: None,
                                };
                                self.refresh_wizard_gui_list(tx.clone()).await;
                            }
                        }
                        KeyCode::Insert => {
                            let local_mnt = current_path.clone();
                            let flags = vec![
                                ("mount_point".to_string(), String::new(), String::new(), local_mnt),
                            ];
                            self.execute_launch_service(
                                service_type,
                                remote,
                                remote_path,
                                None,
                                flags,
                            );
                        }
                        KeyCode::F(7) => {
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
                                service_type,
                                remote,
                                remote_path,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        KeyCode::Char('n') if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) => {
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
                                service_type,
                                remote,
                                remote_path,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        KeyCode::Char('N') if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) => {
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
                                service_type,
                                remote,
                                remote_path,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        KeyCode::Char('N') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
                                service_type,
                                remote,
                                remote_path,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        KeyCode::Char('n') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT) => {
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
                                service_type,
                                remote,
                                remote_path,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        _ => {}
                    }
                }
            }
            ui::services::ServicesWizardState::SelectProtocol {
                remote,
                path,
                mut selected_idx,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.services_state.wizard = ui::services::ServicesWizardState::None;
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = 3;
                        } else {
                            selected_idx -= 1;
                        }
                        self.services_state.wizard =
                            ui::services::ServicesWizardState::SelectProtocol {
                                remote,
                                path,
                                selected_idx,
                            };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % 4;
                        self.services_state.wizard =
                            ui::services::ServicesWizardState::SelectProtocol {
                                remote,
                                path,
                                selected_idx,
                            };
                    }
                    KeyCode::Enter => {
                        let proto = match selected_idx {
                            0 => "http",
                            1 => "ftp",
                            2 => "webdav",
                            3 => "sftp",
                            _ => "http",
                        }
                        .to_string();

                        // Chuyển sang hỏi cờ cho Serve
                        self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
                            service_type: ui::services::ServiceType::Serve,
                            remote,
                            path,
                            protocol: Some(proto),
                            flags: vec![
                                (
                                    "--addr".to_string(),
                                    "Địa chỉ IP và Cổng gán (Ví dụ :8080 hoặc 127.0.0.1:8080 - Mặc định: :8080)".to_string(),
                                    ":8080".to_string(),
                                    String::new(),
                                ),
                                (
                                    "--user".to_string(),
                                    "Tên đăng nhập bảo mật (Để trống nếu không dùng)".to_string(),
                                    "".to_string(),
                                    String::new(),
                                ),
                                (
                                    "--pass".to_string(),
                                    "Mật khẩu bảo mật (Để trống nếu không dùng)".to_string(),
                                    "".to_string(),
                                    String::new(),
                                ),
                                (
                                    "--read-only".to_string(),
                                    "Chỉ cho phép tải xuống (Read Only)? (y/N)".to_string(),
                                    "n".to_string(),
                                    String::new(),
                                ),
                            ],
                            current_flag_idx: 0,
                            input_buffer: String::new(),
                            is_simple_terminal: false,
                            is_editing: false,
                        };
                    }
                    _ => {}
                }
            }
            ui::services::ServicesWizardState::AskFlags {
                service_type,
                remote,
                path,
                protocol,
                mut flags,
                mut current_flag_idx,
                mut input_buffer,
                is_simple_terminal,
                is_editing,
            } => {
                let total_options = flags.len() + 2;

                if is_editing {
                    let mut cursor = self.services_state.edit_cursor_idx;
                    if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                        self.services_state.edit_cursor_idx = cursor;
                        self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
                            service_type,
                            remote,
                            path,
                            protocol,
                            flags,
                            current_flag_idx,
                            input_buffer,
                            is_simple_terminal,
                            is_editing: true,
                        };
                    } else {
                        match key.code {
                            KeyCode::Esc => {
                                self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
                                    service_type,
                                    remote,
                                    path,
                                    protocol,
                                    flags,
                                    current_flag_idx,
                                    input_buffer: String::new(),
                                    is_simple_terminal,
                                    is_editing: false,
                                };
                            }
                            KeyCode::Enter => {
                                flags[current_flag_idx].3 = input_buffer.trim().to_string();
                                self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
                                    service_type,
                                    remote,
                                    path,
                                    protocol,
                                    flags,
                                    current_flag_idx,
                                    input_buffer: String::new(),
                                    is_simple_terminal,
                                    is_editing: false,
                                };
                            }
                            _ => {}
                        }
                    }
                    return;
                }

                match key.code {
                    KeyCode::Esc => {
                        self.services_state.wizard = ui::services::ServicesWizardState::None;
                    }
                    KeyCode::Up => {
                        if current_flag_idx == 0 {
                            current_flag_idx = total_options - 1;
                        } else {
                            current_flag_idx -= 1;
                        }
                        self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
                            service_type,
                            remote,
                            path,
                            protocol,
                            flags,
                            current_flag_idx,
                            input_buffer: String::new(),
                            is_simple_terminal,
                            is_editing: false,
                        };
                    }
                    KeyCode::Down => {
                        current_flag_idx = (current_flag_idx + 1) % total_options;
                        self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
                            service_type,
                            remote,
                            path,
                            protocol,
                            flags,
                            current_flag_idx,
                            input_buffer: String::new(),
                            is_simple_terminal,
                            is_editing: false,
                        };
                    }
                    KeyCode::Enter => {
                        if current_flag_idx < flags.len() {
                            let val = flags[current_flag_idx].3.clone();
                            self.services_state.edit_cursor_idx = val.chars().count();
                            self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
                                service_type,
                                remote,
                                path,
                                protocol,
                                flags,
                                current_flag_idx,
                                input_buffer: val,
                                is_simple_terminal,
                                is_editing: true,
                            };
                        } else if current_flag_idx == flags.len() {
                            for item in flags.iter_mut() {
                                if item.3.is_empty() {
                                    if item.0 == "mount_point" || item.0 == "--addr" || item.0 == "--rc-addr" {
                                        item.3 = item.2.clone();
                                    }
                                }
                            }
                            self.execute_launch_service(
                                service_type,
                                remote,
                                path,
                                protocol,
                                flags,
                            );
                        } else {
                            self.services_state.wizard = ui::services::ServicesWizardState::None;
                        }
                    }
                    _ => {}
                }
            }
            ui::services::ServicesWizardState::SelectSystemdAction {
                service_name,
                file_path,
                is_user,
                mut selected_idx,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.services_state.wizard = ui::services::ServicesWizardState::None;
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = 5;
                        } else {
                            selected_idx -= 1;
                        }
                        self.services_state.wizard = ui::services::ServicesWizardState::SelectSystemdAction {
                            service_name,
                            file_path,
                            is_user,
                            selected_idx,
                        };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % 6;
                        self.services_state.wizard = ui::services::ServicesWizardState::SelectSystemdAction {
                            service_name,
                            file_path,
                            is_user,
                            selected_idx,
                        };
                    }
                    KeyCode::Enter => {
                        self.services_state.wizard = ui::services::ServicesWizardState::None;
                        let cmd_res = match selected_idx {
                            0 => {
                                self.ensure_mount_point_exists_from_service_file(&file_path);
                                if is_user {
                                    Command::new("systemctl").args(["--user", "start", &service_name]).status()
                                } else {
                                    Command::new("pkexec").args(["systemctl", "start", &service_name]).status()
                                }
                            }
                            1 => {
                                if is_user {
                                    Command::new("systemctl").args(["--user", "stop", &service_name]).status()
                                } else {
                                    Command::new("pkexec").args(["systemctl", "stop", &service_name]).status()
                                }
                            }
                            2 => {
                                self.ensure_mount_point_exists_from_service_file(&file_path);
                                if is_user {
                                    Command::new("systemctl").args(["--user", "restart", &service_name]).status()
                                } else {
                                    Command::new("pkexec").args(["systemctl", "restart", &service_name]).status()
                                }
                            }
                            3 => {
                                if is_user {
                                    Command::new("systemctl").args(["--user", "enable", &service_name]).status()
                                } else {
                                    Command::new("pkexec").args(["systemctl", "enable", &service_name]).status()
                                }
                            }
                            4 => {
                                if is_user {
                                    Command::new("systemctl").args(["--user", "disable", &service_name]).status()
                                } else {
                                    Command::new("pkexec").args(["systemctl", "disable", &service_name]).status()
                                }
                            }
                            5 => {
                                match self.load_systemd_service_fields(&file_path, is_user) {
                                    Ok(fields) => {
                                        self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                            service_name: service_name.clone(),
                                            file_path: file_path.clone(),
                                            is_user,
                                            fields,
                                            selected_idx: 0,
                                            scroll_offset: 0,
                                            is_editing: false,
                                            input_buffer: String::new(),
                                            active_tab: 0,
                                            adding_new_key: false,
                                            new_key_buffer: String::new(),
                                        };
                                        return;
                                    }
                                    Err(e) => {
                                        self.services_state.error_message = Some(format!(
                                            "Lỗi đọc file cấu hình: {}", e
                                        ));
                                        return;
                                    }
                                }
                            }
                            _ => return,
                        };
                        match cmd_res {
                            Ok(status) if status.success() => {
                                self.services_state.info_message = Some("Thực thi lệnh systemd thành công!".to_string());
                            }
                            Ok(status) => {
                                let is_eng = crate::lang::translate("srv_error_title").contains("SERVICE");
                                let mut err_msg = if is_eng {
                                    format!("Error executing systemd command: {}", status)
                                } else {
                                    format!("Lỗi thực thi lệnh systemd: {}", status)
                                };
                                if selected_idx == 0 || selected_idx == 2 {
                                    let logs = self.get_systemd_error_logs(&service_name, is_user);
                                    if !logs.is_empty() {
                                        let detail_hdr = if is_eng {
                                            "\n\nError details from system journal:\n"
                                        } else {
                                            "\n\nChi tiết lỗi từ nhật ký hệ thống:\n"
                                        };
                                        err_msg.push_str(&format!("{}{}", detail_hdr, logs));
                                    }
                                }
                                self.services_state.error_message = Some(err_msg);
                            }
                            Err(e) => {
                                self.services_state.error_message = Some(format!("Không thể chạy systemctl: {}", e));
                            }
                        }
                        self.scan_systemd_services();
                    }
                    _ => {}
                }
            }
            ui::services::ServicesWizardState::EditSystemdService {
                service_name,
                file_path,
                is_user,
                mut fields,
                mut selected_idx,
                mut scroll_offset,
                is_editing,
                mut input_buffer,
                mut active_tab,
                adding_new_key,
                mut new_key_buffer,
            } => {
                let filtered_fields: Vec<&(String, String, String, Vec<String>)> = fields
                    .iter()
                    .filter(|(name, _, _, _)| {
                        if active_tab == 0 {
                            name.starts_with('_')
                        } else {
                            !name.starts_with('_')
                        }
                    })
                    .collect();
                let total_fields_count = filtered_fields.len();
                let total_options = total_fields_count + 2;

                if adding_new_key {
                    match key.code {
                        KeyCode::Esc => {
                            self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key: false, new_key_buffer: String::new()
                            };
                        }
                        KeyCode::Char(c) => {
                            new_key_buffer.push(c);
                            self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key: true, new_key_buffer
                            };
                        }
                        KeyCode::Backspace => {
                            new_key_buffer.pop();
                            self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key: true, new_key_buffer
                            };
                        }
                        KeyCode::Enter => {
                            let trimmed = new_key_buffer.trim().to_string();
                            if !trimmed.is_empty() {
                                if !fields.iter().any(|(k, _, _, _)| k == &trimmed) {
                                    fields.push((trimmed.clone(), String::new(), String::new(), Vec::new()));
                                }
                                let new_filtered: Vec<&(String, String, String, Vec<String>)> = fields
                                    .iter()
                                    .filter(|(name, _, _, _)| !name.starts_with('_'))
                                    .collect();
                                let new_sel = new_filtered.iter().position(|(k, _, _, _)| *k == trimmed).unwrap_or(0);
                                self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                    service_name, file_path, is_user, fields, selected_idx: new_sel, scroll_offset: 0, is_editing: false, input_buffer: String::new(), active_tab: 1, adding_new_key: false, new_key_buffer: String::new()
                                };
                            }
                        }
                        _ => {}
                    }
                    return;
                }

                if is_editing {
                    let is_remote_field = filtered_fields.get(selected_idx).map(|f| f.0.as_str()) == Some("_remote");
                    if is_remote_field && (key.code == KeyCode::Up || key.code == KeyCode::Down) {
                        let remote_list = &self.services_state.all_remotes;
                        if !remote_list.is_empty() {
                            let (prefix, suffix) = if let Some(pos) = input_buffer.find(':') {
                                (input_buffer[..pos].to_string(), input_buffer[pos..].to_string())
                            } else {
                                (input_buffer.clone(), ":".to_string())
                            };
                            let current_idx = remote_list.iter().position(|r| r == &prefix);
                            let next_idx = match current_idx {
                                Some(idx) => {
                                    if key.code == KeyCode::Up {
                                        if idx == 0 {
                                            remote_list.len() - 1
                                        } else {
                                            idx - 1
                                        }
                                    } else {
                                        (idx + 1) % remote_list.len()
                                    }
                                }
                                None => 0,
                            };
                            input_buffer = format!("{}{}", remote_list[next_idx], suffix);
                        }
                        self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                            service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key, new_key_buffer
                        };
                    } else {
                        let mut cursor = self.services_state.edit_cursor_idx;
                        if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                            self.services_state.edit_cursor_idx = cursor;
                            self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: true, input_buffer, active_tab, adding_new_key, new_key_buffer
                            };
                        } else {
                            match key.code {
                                KeyCode::Esc => {
                                    self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                        service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                                    };
                                }
                                KeyCode::Enter => {
                                    let field_to_update = filtered_fields[selected_idx].0.clone();
                                    if let Some(item) = fields.iter_mut().find(|(k, _, _, _)| k == &field_to_update) {
                                        item.2 = input_buffer.clone();
                                    }
                                    self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                        service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                                    };
                                }
                                _ => {}
                            }
                        }
                    }
                    return;
                }

                match key.code {
                    KeyCode::Esc => {
                        self.services_state.wizard = ui::services::ServicesWizardState::None;
                    }
                    KeyCode::Tab | KeyCode::Left | KeyCode::Right => {
                        active_tab = if active_tab == 0 { 1 } else { 0 };
                        self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                            service_name, file_path, is_user, fields, selected_idx: 0, scroll_offset: 0, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                        };
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = total_options - 1;
                        } else {
                            selected_idx -= 1;
                        }
                        scroll_offset = ui::calculate_scroll_range(selected_idx, total_options, 15).start;
                        self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                            service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                        };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % total_options;
                        scroll_offset = ui::calculate_scroll_range(selected_idx, total_options, 15).start;
                        self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                            service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                        };
                    }
                    KeyCode::Enter => {
                        if selected_idx < total_fields_count {
                            let (field_to_update, choices, val) = {
                                let f = filtered_fields[selected_idx];
                                (f.0.clone(), f.3.clone(), f.2.clone())
                            };
                            if !choices.is_empty() && field_to_update != "_remote" {
                                let pos = choices.iter().position(|c| c == &val).unwrap_or(0);
                                let next_pos = (pos + 1) % choices.len();
                                let next_val = choices[next_pos].clone();
                                if let Some(item) = fields.iter_mut().find(|(k, _, _, _)| k == &field_to_update) {
                                    item.2 = next_val;
                                }
                                self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                    service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                                };
                            } else {
                                self.services_state.edit_cursor_idx = val.chars().count();
                                self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                    service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: true, input_buffer: val, active_tab, adding_new_key, new_key_buffer
                                };
                            }
                        } else if selected_idx == total_fields_count {
                            // Lưu
                            match self.save_systemd_service_file(false, &service_name, &file_path, is_user, &fields) {
                                Ok(_) => {
                                    self.services_state.info_message = Some(format!("Đã cập nhật dịch vụ '{}' thành công!", service_name));
                                    self.services_state.wizard = ui::services::ServicesWizardState::None;
                                    self.scan_systemd_services();
                                }
                                Err(e) => {
                                    self.services_state.error_message = Some(format!("Lỗi khi lưu cấu hình dịch vụ: {}", e));
                                }
                            }
                        } else {
                            // Hủy
                            self.services_state.wizard = ui::services::ServicesWizardState::None;
                        }
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        match self.save_systemd_service_file(false, &service_name, &file_path, is_user, &fields) {
                            Ok(_) => {
                                self.services_state.info_message = Some(format!("Đã cập nhật dịch vụ '{}' thành công!", service_name));
                                self.services_state.wizard = ui::services::ServicesWizardState::None;
                                self.scan_systemd_services();
                            }
                            Err(e) => {
                                self.services_state.error_message = Some(format!("Lỗi khi lưu cấu hình dịch vụ: {}", e));
                            }
                        }
                    }
                    KeyCode::Insert => {
                        if active_tab == 1 {
                            self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key: true, new_key_buffer: String::new()
                            };
                        }
                    }
                    KeyCode::Delete | KeyCode::Backspace => {
                        if active_tab == 1 && selected_idx < total_fields_count {
                            let field_to_delete = filtered_fields[selected_idx].0.clone();
                            fields.retain(|(k, _, _, _)| k != &field_to_delete);
                            let new_sel = if selected_idx >= fields.len() && !fields.is_empty() {
                                fields.len() - 1
                            } else {
                                selected_idx
                            };
                            self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                service_name, file_path, is_user, fields, selected_idx: new_sel, scroll_offset: 0, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                            };
                        }
                    }
                    _ => {}
                }
            }
            ui::services::ServicesWizardState::CreateSystemdService {
                mut fields,
                mut selected_idx,
                mut scroll_offset,
                is_editing,
                mut input_buffer,
                mut active_tab,
                adding_new_key,
                mut new_key_buffer,
            } => {
                let filtered_fields: Vec<&(String, String, String, Vec<String>)> = fields
                    .iter()
                    .filter(|(name, _, _, _)| {
                        if active_tab == 0 {
                            name.starts_with('_')
                        } else {
                            !name.starts_with('_')
                        }
                    })
                    .collect();
                let total_fields_count = filtered_fields.len();
                let total_options = total_fields_count + 2;

                if adding_new_key {
                    match key.code {
                        KeyCode::Esc => {
                            self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key: false, new_key_buffer: String::new()
                            };
                        }
                        KeyCode::Char(c) => {
                            new_key_buffer.push(c);
                            self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key: true, new_key_buffer
                            };
                        }
                        KeyCode::Backspace => {
                            new_key_buffer.pop();
                            self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key: true, new_key_buffer
                            };
                        }
                        KeyCode::Enter => {
                            let trimmed = new_key_buffer.trim().to_string();
                            if !trimmed.is_empty() {
                                if !fields.iter().any(|(k, _, _, _)| k == &trimmed) {
                                    fields.push((trimmed.clone(), String::new(), String::new(), Vec::new()));
                                }
                                let new_filtered: Vec<&(String, String, String, Vec<String>)> = fields
                                    .iter()
                                    .filter(|(name, _, _, _)| !name.starts_with('_'))
                                    .collect();
                                let new_sel = new_filtered.iter().position(|(k, _, _, _)| *k == trimmed).unwrap_or(0);
                                self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                    fields, selected_idx: new_sel, scroll_offset: 0, is_editing: false, input_buffer: String::new(), active_tab: 1, adding_new_key: false, new_key_buffer: String::new()
                                };
                            }
                        }
                        _ => {}
                    }
                    return;
                }

                if is_editing {
                    let is_remote_field = filtered_fields.get(selected_idx).map(|f| f.0.as_str()) == Some("_remote");
                    if is_remote_field && (key.code == KeyCode::Up || key.code == KeyCode::Down) {
                        let remote_list = &self.services_state.all_remotes;
                        if !remote_list.is_empty() {
                            let (prefix, suffix) = if let Some(pos) = input_buffer.find(':') {
                                (input_buffer[..pos].to_string(), input_buffer[pos..].to_string())
                            } else {
                                (input_buffer.clone(), ":".to_string())
                            };
                            let current_idx = remote_list.iter().position(|r| r == &prefix);
                            let next_idx = match current_idx {
                                Some(idx) => {
                                    if key.code == KeyCode::Up {
                                        if idx == 0 {
                                            remote_list.len() - 1
                                        } else {
                                            idx - 1
                                        }
                                    } else {
                                        (idx + 1) % remote_list.len()
                                    }
                                }
                                None => 0,
                            };
                            input_buffer = format!("{}{}", remote_list[next_idx], suffix);
                        }
                        self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                            fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key, new_key_buffer
                        };
                    } else {
                        let mut cursor = self.services_state.edit_cursor_idx;
                        if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                            self.services_state.edit_cursor_idx = cursor;
                            self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                fields, selected_idx, scroll_offset, is_editing: true, input_buffer, active_tab, adding_new_key, new_key_buffer
                            };
                        } else {
                            match key.code {
                                KeyCode::Esc => {
                                    self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                        fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                                    };
                                }
                                KeyCode::Enter => {
                                    let field_to_update = filtered_fields[selected_idx].0.clone();
                                    if let Some(item) = fields.iter_mut().find(|(k, _, _, _)| k == &field_to_update) {
                                        item.2 = input_buffer.clone();
                                    }
                                    self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                        fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                                    };
                                }
                                _ => {}
                            }
                        }
                    }
                    return;
                }

                match key.code {
                    KeyCode::Esc => {
                        self.services_state.wizard = ui::services::ServicesWizardState::None;
                    }
                    KeyCode::Tab | KeyCode::Left | KeyCode::Right => {
                        active_tab = if active_tab == 0 { 1 } else { 0 };
                        self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                            fields, selected_idx: 0, scroll_offset: 0, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                        };
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = total_options - 1;
                        } else {
                            selected_idx -= 1;
                        }
                        scroll_offset = ui::calculate_scroll_range(selected_idx, total_options, 15).start;
                        self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                            fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                        };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % total_options;
                        scroll_offset = ui::calculate_scroll_range(selected_idx, total_options, 15).start;
                        self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                            fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                        };
                    }
                    KeyCode::Enter => {
                        if selected_idx < total_fields_count {
                            let (field_to_update, choices, val) = {
                                let f = filtered_fields[selected_idx];
                                (f.0.clone(), f.3.clone(), f.2.clone())
                            };
                            if !choices.is_empty() && field_to_update != "_remote" {
                                let pos = choices.iter().position(|c| c == &val).unwrap_or(0);
                                let next_pos = (pos + 1) % choices.len();
                                let next_val = choices[next_pos].clone();
                                if let Some(item) = fields.iter_mut().find(|(k, _, _, _)| k == &field_to_update) {
                                    item.2 = next_val;
                                }
                                self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                    fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                                };
                            } else {
                                self.services_state.edit_cursor_idx = val.chars().count();
                                self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                    fields, selected_idx, scroll_offset, is_editing: true, input_buffer: val, active_tab, adding_new_key, new_key_buffer
                                };
                            }
                        } else if selected_idx == total_fields_count {
                            // Thực hiện tạo file dịch vụ mới
                            let get_val = |key: &str| {
                                fields.iter().find(|(k, _, _, _)| k == key).map(|(_, _, v, _)| v.clone()).unwrap_or_default()
                            };
                            let raw_svc_name = get_val("_service_name");
                            let service_name = if raw_svc_name.ends_with(".service") {
                                raw_svc_name
                            } else {
                                format!("{}.service", raw_svc_name)
                            };
                            let level = get_val("_service_level");
                            let is_user = level.contains("User");
                            let home_dir = crate::app_config::get_home_dir();
                            let file_path = if is_user {
                                format!("{}/.config/systemd/user/{}", home_dir, service_name)
                            } else {
                                format!("/etc/systemd/system/{}", service_name)
                            };

                            match self.save_systemd_service_file(true, &service_name, &file_path, is_user, &fields) {
                                Ok(_) => {
                                    self.services_state.info_message = Some(format!("Đã tạo thành công dịch vụ systemd '{}'!", service_name));
                                    self.services_state.wizard = ui::services::ServicesWizardState::None;
                                    self.scan_systemd_services();
                                }
                                Err(e) => {
                                    self.services_state.error_message = Some(format!("Lỗi khi lưu cấu hình dịch vụ: {}", e));
                                }
                            }
                        } else {
                            self.services_state.wizard = ui::services::ServicesWizardState::None;
                        }
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let get_val = |key: &str| {
                            fields.iter().find(|(k, _, _, _)| k == key).map(|(_, _, v, _)| v.clone()).unwrap_or_default()
                        };
                        let raw_svc_name = get_val("_service_name");
                        let service_name = if raw_svc_name.ends_with(".service") {
                            raw_svc_name
                        } else {
                            format!("{}.service", raw_svc_name)
                        };
                        let level = get_val("_service_level");
                        let is_user = level.contains("User");
                        let home_dir = crate::app_config::get_home_dir();
                        let file_path = if is_user {
                            format!("{}/.config/systemd/user/{}", home_dir, service_name)
                        } else {
                            format!("/etc/systemd/system/{}", service_name)
                        };

                        match self.save_systemd_service_file(true, &service_name, &file_path, is_user, &fields) {
                            Ok(_) => {
                                self.services_state.info_message = Some(format!("Đã tạo thành công dịch vụ systemd '{}'!", service_name));
                                self.services_state.wizard = ui::services::ServicesWizardState::None;
                                self.scan_systemd_services();
                            }
                            Err(e) => {
                                self.services_state.error_message = Some(format!("Lỗi khi lưu cấu hình dịch vụ: {}", e));
                            }
                        }
                    }
                    KeyCode::Insert => {
                        if active_tab == 1 {
                            self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key: true, new_key_buffer: String::new()
                            };
                        }
                    }
                    KeyCode::Delete | KeyCode::Backspace => {
                        if active_tab == 1 && selected_idx < total_fields_count {
                            let field_to_delete = filtered_fields[selected_idx].0.clone();
                            fields.retain(|(k, _, _, _)| k != &field_to_delete);
                            let new_sel = if selected_idx >= fields.len() && !fields.is_empty() {
                                fields.len() - 1
                            } else {
                                selected_idx
                            };
                            self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                fields, selected_idx: new_sel, scroll_offset: 0, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                            };
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn execute_launch_service(
        &mut self,
        service_type: ui::services::ServiceType,
        remote: String,
        path: String,
        protocol: Option<String>,
        flags: Vec<(String, String, String, String)>,
    ) {
        self.services_state.wizard = ui::services::ServicesWizardState::None;

        let config_path = self.config.get_active_profile_path();

        let mut args = vec![];

        match service_type {
            ui::services::ServiceType::Mount | ui::services::ServiceType::NfsMount => {
                let cmd_name = if service_type == ui::services::ServiceType::NfsMount {
                    "nfsmount"
                } else {
                    "mount"
                };
                args.push(cmd_name.to_string());
                
                // remote + path
                let r_path = format!("{}{}", remote, path);
                args.push(r_path);
 
                // Mount point
                let local_mnt = flags[0].3.clone();
                let mut need_escalation = false;
                let is_drive = cfg!(windows) && (
                    (local_mnt.len() == 2 && local_mnt.ends_with(':'))
                    || (local_mnt.len() == 3 && local_mnt.ends_with(":\\"))
                    || (local_mnt.len() == 3 && local_mnt.ends_with(":/"))
                );

                if !is_drive {
                    if fs::create_dir_all(&local_mnt).is_err() {
                        need_escalation = true;
                    } else {
                        let temp_file = Path::new(&local_mnt).join(".rclone_tui_temp");
                        if fs::write(&temp_file, "").is_err() {
                            need_escalation = true;
                        } else {
                            let _ = fs::remove_file(temp_file);
                        }
                    }
                }
 
                #[cfg(unix)]
                if need_escalation {
                    self.services_state.info_message = Some("Yêu cầu xác thực quyền root để tạo và phân quyền thư mục mount...".to_string());
                    let _ = Command::new("pkexec")
                        .args(&["mkdir", "-p", &local_mnt])
                        .status();
 
                    let username = std::process::Command::new("id")
                        .args(&["-u", "-n"])
                        .output()
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                        .unwrap_or_else(|_| "bimatkeo".to_string());
 
                    let groupname = std::process::Command::new("id")
                        .args(&["-g", "-n"])
                        .output()
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                        .unwrap_or_else(|_| "bimatkeo".to_string());
 
                    let owner_arg = format!("{}:{}", username, groupname);
                    let _ = Command::new("pkexec")
                        .args(&["chown", "-R", &owner_arg, &local_mnt])
                        .status();
                }

                #[cfg(windows)]
                if need_escalation {
                    self.services_state.info_message = Some("Yêu cầu xác thực Administrator để tạo thư mục mount...".to_string());
                    let _ = Command::new("powershell")
                        .args(&[
                            "-NoProfile",
                            "-Command",
                            &format!("Start-Process cmd -ArgumentList '/c mkdir \"{}\"' -Verb RunAs -WindowStyle Hidden -Wait", local_mnt)
                        ])
                        .status();
                }
 
                args.push(local_mnt.clone());
 
                args.push(format!("--config={}", config_path));
 
                // --vfs-cache-mode
                let vfs_cache = if flags.len() > 1 { flags[1].3.clone() } else { "writes".to_string() };
                if !vfs_cache.is_empty() {
                    args.push(format!("--vfs-cache-mode={}", vfs_cache));
                }
 
                // --dir-cache-time
                let dir_cache = if flags.len() > 2 { flags[2].3.clone() } else { "5m".to_string() };
                if !dir_cache.is_empty() {
                    args.push(format!("--dir-cache-time={}", dir_cache));
                }
 
                // --read-only
                let read_only = if flags.len() > 3 { flags[3].3.to_lowercase() } else { "n".to_string() };
                if read_only == "y" {
                    args.push("--read-only".to_string());
                }
 
                // --daemon
                #[cfg(unix)]
                {
                    let daemon = if flags.len() > 4 { flags[4].3.to_lowercase() } else { "y".to_string() };
                    if daemon == "y" {
                        args.push("--daemon".to_string());
                    }
                }
 
                // Chạy lệnh mount độc lập dưới quyền user hiện tại
                let rclone_cmd = get_rclone_cmd();
                #[cfg(unix)]
                let child = Command::new("setsid").arg(&rclone_cmd).args(&args).spawn();
                #[cfg(not(unix))]
                let child = Command::new(&rclone_cmd).args(&args).spawn();
                match child {
                    Ok(c) => {
                        let pid = c.id();
                        self.services_state
                            .active_services
                            .push(ui::services::ActiveService {
                                service_type_str: if service_type == ui::services::ServiceType::NfsMount {
                                    "NfsMount".to_string()
                                } else {
                                    "Mount".to_string()
                                },
                                remote,
                                path: local_mnt.clone(),
                                pid,
                                details: format!("{} -> {}", if service_type == ui::services::ServiceType::NfsMount { "NfsMount" } else { "Mount" }, local_mnt),
                            });
                        // Chờ một chút để tiến trình daemon khởi chạy (nếu có)
                        std::thread::sleep(std::time::Duration::from_millis(150));
                        self.scan_running_services();
                        self.services_state.info_message =
                            Some(format!("Khởi chạy {} thành công. PID: {}", cmd_name, pid));
                    }
                    Err(e) => {
                        self.services_state.error_message = Some(format!("Không thể {}: {}", cmd_name, e));
                    }
                }
            }
            ui::services::ServiceType::WebGui => {
                args.push("rcd".to_string());
                args.push("--rc-web-gui".to_string());
                args.push(format!("--config={}", config_path));

                let rc_addr = flags[0].3.clone();
                args.push(format!("--rc-addr={}", rc_addr));

                if flags[1].3.to_lowercase() == "y" {
                    args.push("--rc-no-auth".to_string());
                }

                let rclone_cmd = get_rclone_cmd();
                let child = Command::new(&rclone_cmd).args(&args).spawn();
                match child {
                    Ok(c) => {
                        let pid = c.id();
                        self.services_state
                            .active_services
                            .push(ui::services::ActiveService {
                                service_type_str: "WebGui".to_string(),
                                remote: String::new(),
                                path: rc_addr.clone(),
                                pid,
                                details: format!("Cổng Web: {}", rc_addr),
                            });
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        self.scan_running_services();
                        self.services_state.info_message = Some(format!(
                            "Web GUI hoạt động trên cổng {}. PID: {}",
                            rc_addr, pid
                        ));
                        // Tự động mở trình duyệt Web GUI
                        let _ = webbrowser::open(&format!("http://{}", rc_addr));
                    }
                    Err(e) => {
                        self.services_state.error_message = Some(format!("Lỗi bật Web GUI: {}", e));
                    }
                }
            }
            ui::services::ServiceType::Serve => {
                let proto = protocol.unwrap_or_else(|| "http".to_string());
                args.push("serve".to_string());
                args.push(proto.clone());
                args.push(format!("{}{}", remote, path));
                args.push(format!("--config={}", config_path));

                let addr = flags[0].3.clone();
                args.push(format!("--addr={}", addr));

                let user = flags[1].3.clone();
                if !user.is_empty() {
                    args.push(format!("--user={}", user));
                }

                let pass = flags[2].3.clone();
                if !pass.is_empty() {
                    args.push(format!("--pass={}", pass));
                }

                if flags[3].3.to_lowercase() == "y" {
                    args.push("--read-only".to_string());
                }

                let rclone_cmd = get_rclone_cmd();
                let child = Command::new(&rclone_cmd).args(&args).spawn();
                match child {
                    Ok(c) => {
                        let pid = c.id();
                        self.services_state
                            .active_services
                            .push(ui::services::ActiveService {
                                service_type_str: "Serve".to_string(),
                                remote,
                                path: addr.clone(),
                                pid,
                                details: format!("{}://{}", proto, addr),
                            });
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        self.scan_running_services();
                        self.services_state.info_message = Some(format!(
                            "Máy chủ chia sẻ {} đang chạy tại {}. PID: {}",
                            proto, addr, pid
                        ));
                    }
                    Err(e) => {
                        self.services_state.error_message =
                            Some(format!("Lỗi chia sẻ server: {}", e));
                    }
                }
            }
        }
    }
    pub fn refresh_tui_selector_list(
        &mut self,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        if let ui::explorer::ExplorerPopup::TuiExplorerSelector {
            ref mut loading,
            ref remote,
            ref path,
            ..
        } = self.explorer_state.popup
        {
            *loading = true;
            let remote = remote.clone();
            let path = path.clone();
            let tx_clone = tx.clone();

            tokio::spawn(async move {
                if remote.is_empty() && path.is_empty() {
                    let res = rclone::rpc_async("config/listremotes".to_string(), "{}".to_string()).await;
                    match res {
                        Ok(rpc_res) => {
                            if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                                if let Some(remotes_arr) = val.get("remotes").and_then(|r| r.as_array()) {
                                    let mut items = vec![ui::explorer::FileItem {
                                        name: "[Local System]".to_string(),
                                        size: 0,
                                        is_dir: true,
                                        mod_time: "---".to_string(),
                                    }];
                                    for r_val in remotes_arr {
                                        if let Some(r_str) = r_val.as_str() {
                                            items.push(ui::explorer::FileItem {
                                                name: r_str.to_string(),
                                                size: 0,
                                                is_dir: true,
                                                mod_time: "---".to_string(),
                                            });
                                        }
                                    }
                                    let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                                        result: Ok(items),
                                    });
                                    return;
                                }
                            }
                            let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                                result: Err("Không thể phân tích danh sách remote".to_string()),
                            });
                        }
                        Err(e) => {
                            let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                                result: Err(e),
                            });
                        }
                    }
                } else {
                    let fs_target = if remote.is_empty() {
                        path.clone()
                    } else {
                        let clean_remote = remote.trim_end_matches(':');
                        let clean_path = if path.starts_with('/') {
                            path.clone()
                        } else {
                            format!("/{}", path)
                        };
                        format!("{}:{}", clean_remote, clean_path)
                    };

                    let input_param = json!({
                        "fs": fs_target,
                        "remote": "",
                    })
                    .to_string();

                    let list_future = rclone::rpc_async("operations/list".to_string(), input_param);
                    let res = match tokio::time::timeout(std::time::Duration::from_secs(15), list_future).await {
                        Ok(inner_res) => inner_res,
                        Err(_) => Err("Hết thời gian chờ phản hồi từ Cloud (Timeout)".to_string()),
                    };

                    match res {
                        Ok(rpc_res) => {
                            if rpc_res.status == 200 {
                                if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                                    if let Some(err_str) = val.get("error").and_then(|e| e.as_str()) {
                                        let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                                            result: Err(err_str.to_string()),
                                        });
                                    } else if let Some(list_arr) = val.get("list").and_then(|l| l.as_array()) {
                                        let mut items = Vec::new();
                                        for item_val in list_arr {
                                            let name = item_val
                                                .get("Name")
                                                .and_then(|n| n.as_str())
                                                .unwrap_or("")
                                                .to_string();
                                            let size =
                                                item_val.get("Size").and_then(|s| s.as_u64()).unwrap_or(0);
                                            let is_dir = item_val
                                                .get("IsDir")
                                                .and_then(|d| d.as_bool())
                                                .unwrap_or(false);
                                            let mod_time = item_val
                                                .get("ModTime")
                                                .and_then(|m| m.as_str())
                                                .unwrap_or("")
                                                .to_string();

                                            let cleaned_time = mod_time
                                                .chars()
                                                .take(19)
                                                .collect::<String>()
                                                .replace("T", " ");

                                            if is_dir {
                                                items.push(ui::explorer::FileItem {
                                                    name,
                                                    size,
                                                    is_dir: true,
                                                    mod_time: cleaned_time,
                                                });
                                            }
                                        }
                                        items.sort_by(|a, b| a.name.cmp(&b.name));

                                        let at_root = if remote.is_empty() {
                                            path == "/" || path.is_empty()
                                        } else {
                                            path.is_empty()
                                        };
                                        if !at_root {
                                            items.insert(
                                                0,
                                                ui::explorer::FileItem {
                                                    name: "..".to_string(),
                                                    size: 0,
                                                    is_dir: true,
                                                    mod_time: "---".to_string(),
                                                },
                                            );
                                        }

                                        let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                                            result: Ok(items),
                                        });
                                    }
                                }
                            } else {
                                let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                                    result: Err(rpc_res.output),
                                });
                            }
                        }
                        Err(e) => {
                            let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                                result: Err(e),
                            });
                        }
                    }
                }
            });
        }
    }

    async fn handle_special_action_selected(
        &mut self,
        idx: usize,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        let active_pane = self.explorer_state.get_active_pane();
        let selected_item = if !active_pane.items.is_empty() {
            Some(active_pane.items[active_pane.selected_idx].clone())
        } else {
            None
        };

        match idx {
            0 => {
                if let Some(item) = selected_item {
                    if item.name != ".." {
                        let full_path = if active_pane.remote.is_empty() {
                            PathBuf::from(&active_pane.path).join(&item.name).to_string_lossy().to_string()
                        } else {
                            format!("{}:{}/{}", active_pane.remote.trim_end_matches(':'), active_pane.path.trim_start_matches('/'), item.name)
                        };
                        let (parent_fs, filename) = parse_parent_and_child(&full_path);
                        
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::SpecialActionMessage {
                            title: "Đang lấy link...".to_string(),
                            message: "Vui lòng chờ...".to_string(),
                        };
                        
                        let tx_clone = tx.clone();
                        tokio::spawn(async move {
                            let param = json!({
                                "fs": parent_fs,
                                "remote": filename,
                            }).to_string();
                            let res = rclone::rpc_async("operations/publiclink".to_string(), param).await;
                            match res {
                                Ok(rpc_res) if rpc_res.status == 200 => {
                                    if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                                        if let Some(url) = val.get("url").and_then(|u| u.as_str()) {
                                            let copied = copy_to_system_clipboard(url).is_ok();
                                            let msg = if copied {
                                                format!("Link đã được copy vào clipboard:\n{}", url)
                                            } else {
                                                format!("Không thể copy tự động. Link của bạn là:\n{}", url)
                                            };
                                            let _ = tx_clone.send(AppEvent::CryptdecodeResult { result: Ok(msg) });
                                            return;
                                        }
                                    }
                                    let _ = tx_clone.send(AppEvent::CryptdecodeResult {
                                        result: Err("Không phân tích được URL trả về từ Cloud".to_string()),
                                    });
                                }
                                Ok(rpc_res) => {
                                    let _ = tx_clone.send(AppEvent::CryptdecodeResult {
                                        result: Err(format!("Lỗi: {}", rpc_res.output)),
                                    });
                                }
                                Err(e) => {
                                    let _ = tx_clone.send(AppEvent::CryptdecodeResult {
                                        result: Err(e),
                                    });
                                }
                            }
                        });
                    }
                }
            }
            1 => {
                if let Some(item) = selected_item {
                    if !item.is_dir && item.name != ".." {
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::ChecksumTypeSelect {
                            selected_idx: 0,
                        };
                    }
                }
            }
            2 => {
                let fs_target = if active_pane.remote.is_empty() {
                    active_pane.path.clone()
                } else {
                    format!("{}:", active_pane.remote.trim_end_matches(':'))
                };
                self.explorer_state.popup = ui::explorer::ExplorerPopup::ConfirmFallback {
                    title: "Xác nhận Cleanup".to_string(),
                    options: vec!["[Có] Thực hiện".to_string(), "[Không] Hủy bỏ".to_string()],
                    selected_idx: 1,
                    actions: vec![
                        ui::explorer::FallbackAction::CleanupCloud { fs: fs_target },
                        ui::explorer::FallbackAction::Cancel,
                    ],
                };
            }
            3 | 4 => {
                let (target_fs, target_remote) = if let Some(ref item) = selected_item {
                    if item.is_dir && item.name != ".." {
                        let full_path = if active_pane.remote.is_empty() {
                            PathBuf::from(&active_pane.path).join(&item.name).to_string_lossy().to_string()
                        } else {
                            format!("{}:{}/{}", active_pane.remote.trim_end_matches(':'), active_pane.path.trim_start_matches('/'), item.name)
                        };
                        parse_parent_and_child(&full_path)
                    } else {
                        let full_path = if active_pane.remote.is_empty() {
                            active_pane.path.clone()
                        } else {
                            format!("{}:{}", active_pane.remote.trim_end_matches(':'), active_pane.path)
                        };
                        parse_parent_and_child(&full_path)
                    }
                } else {
                    let full_path = if active_pane.remote.is_empty() {
                        active_pane.path.clone()
                    } else {
                        format!("{}:{}", active_pane.remote.trim_end_matches(':'), active_pane.path)
                    };
                    parse_parent_and_child(&full_path)
                };

                let title = if idx == 3 { "Xác nhận rmdir" } else { "Xác nhận rmdirs" };
                let action = if idx == 3 {
                    ui::explorer::FallbackAction::Rmdir { fs: target_fs, remote: target_remote }
                } else {
                    ui::explorer::FallbackAction::Rmdirs { fs: target_fs, remote: target_remote }
                };

                self.explorer_state.popup = ui::explorer::ExplorerPopup::ConfirmFallback {
                    title: title.to_string(),
                    options: vec!["[Có] Thực hiện".to_string(), "[Không] Hủy bỏ".to_string()],
                    selected_idx: 1,
                    actions: vec![
                        action,
                        ui::explorer::FallbackAction::Cancel,
                    ],
                };
            }
            5 => {
                let initial_remote = if !active_pane.remote.is_empty() {
                    active_pane.remote.clone()
                } else {
                    String::new()
                };
                let initial_encrypted = if let Some(item) = selected_item {
                    if item.name != ".." {
                        item.name.clone()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                let focus = initial_remote.is_empty();
                self.explorer_state.edit_cursor_idx = if focus {
                    initial_remote.chars().count()
                } else {
                    initial_encrypted.chars().count()
                };
                self.explorer_state.popup = ui::explorer::ExplorerPopup::CryptdecodeForm {
                    remote_input: initial_remote,
                    encrypted_input: initial_encrypted,
                    is_remote_focused: focus,
                    output_result: None,
                };
            }
            6 => {
                if let Some(item) = selected_item {
                    if !item.is_dir && item.name != ".." {
                        let full_path = if active_pane.remote.is_empty() {
                            PathBuf::from(&active_pane.path).join(&item.name).to_string_lossy().to_string()
                        } else {
                            format!("{}:{}/{}", active_pane.remote.trim_end_matches(':'), active_pane.path.trim_start_matches('/'), item.name)
                        };
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::DecompressModeSelect {
                            archive_path: full_path,
                            selected_idx: 0,
                        };
                    }
                }
            }
            _ => {}
        }
    }

    async fn execute_hashsum_file(
        &mut self,
        hash_type: String,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        let active_pane = self.explorer_state.get_active_pane();
        if active_pane.items.is_empty() {
            return;
        }
        let item = &active_pane.items[active_pane.selected_idx];
        if item.is_dir || item.name == ".." {
            return;
        }

        let file_path = if active_pane.remote.is_empty() {
            PathBuf::from(&active_pane.path).join(&item.name).to_string_lossy().to_string()
        } else {
            format!("{}:{}/{}", active_pane.remote.trim_end_matches(':'), active_pane.path.trim_start_matches('/'), item.name)
        };

        let (parent_fs, file_name) = parse_parent_and_child(&file_path);
        let tx_clone = tx.clone();
        let hash_type_lower = hash_type.to_lowercase();
        
        self.explorer_state.popup = ui::explorer::ExplorerPopup::SpecialActionMessage {
            title: "Đang tính băm...".to_string(),
            message: "Vui lòng chờ...".to_string(),
        };

        tokio::spawn(async move {
            let param = json!({
                "fs": parent_fs,
                "remote": file_name,
            }).to_string();
            let res = rclone::rpc_async("operations/hash".to_string(), param).await;
            match res {
                Ok(rpc_res) if rpc_res.status == 200 => {
                    if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                        if let Some(hashes) = val.get("hashes").and_then(|h| h.as_object()) {
                            let mut found_hash = None;
                            for (k, v) in hashes {
                                let k_clean = k.replace("-", "").to_lowercase();
                                if k_clean == hash_type_lower {
                                    found_hash = v.as_str().map(|s| s.to_string());
                                    break;
                                }
                            }
                            if found_hash.is_none() {
                                if let Some(first_hash) = hashes.iter().next() {
                                    found_hash = Some(format!("{} (Kiểu băm khác: {})", first_hash.1.as_str().unwrap_or(""), first_hash.0));
                                }
                            }
                            
                            if let Some(hash_val) = found_hash {
                                let _ = tx_clone.send(AppEvent::CryptdecodeResult {
                                    result: Ok(format!("Mã băm ({}): {}", hash_type.to_uppercase(), hash_val)),
                                });
                            } else {
                                let _ = tx_clone.send(AppEvent::CryptdecodeResult {
                                    result: Err(format!("Cloud này không hỗ trợ kiểu băm {}", hash_type.to_uppercase())),
                                });
                            }
                        } else {
                            let _ = tx_clone.send(AppEvent::CryptdecodeResult {
                                result: Err("Không có thông tin băm trả về".to_string()),
                            });
                        }
                    } else {
                        let _ = tx_clone.send(AppEvent::CryptdecodeResult {
                            result: Err("Lỗi phân tích kết quả băm".to_string()),
                        });
                    }
                }
                Ok(rpc_res) => {
                    let _ = tx_clone.send(AppEvent::CryptdecodeResult {
                        result: Err(format!("Lỗi RPC: {}", rpc_res.output)),
                    });
                }
                Err(e) => {
                    let _ = tx_clone.send(AppEvent::CryptdecodeResult {
                        result: Err(e),
                    });
                }
            }
        });
    }

    async fn execute_cryptdecode(
        &mut self,
        remote: String,
        encrypted: String,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        if remote.trim().is_empty() || encrypted.trim().is_empty() {
            return;
        }
        let fs = remote.trim().to_string();
        let arg = encrypted.trim().to_string();
        let tx_clone = tx.clone();
        
        tokio::spawn(async move {
            let param = json!({
                "command": "decode",
                "fs": fs,
                "arg": vec![arg],
            }).to_string();
            let res = rclone::rpc_async("backend/command".to_string(), param).await;
            match res {
                Ok(rpc_res) if rpc_res.status == 200 => {
                    if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                        if let Some(decrypted) = val.get("result").and_then(|r| r.as_str()) {
                            let _ = tx_clone.send(AppEvent::CryptdecodeFinished {
                                result: Ok(decrypted.to_string()),
                            });
                            return;
                        }
                    }
                    let _ = tx_clone.send(AppEvent::CryptdecodeFinished {
                        result: Err("Không giải mã được tên tệp".to_string()),
                    });
                }
                Ok(rpc_res) => {
                    let _ = tx_clone.send(AppEvent::CryptdecodeFinished {
                        result: Err(rpc_res.output),
                    });
                }
                Err(e) => {
                    let _ = tx_clone.send(AppEvent::CryptdecodeFinished {
                        result: Err(e),
                    });
                }
            }
        });
    }

    async fn handle_decompress_mode_selected(
        &mut self,
        archive_path: String,
        mode_idx: usize,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        let is_remote_empty = self.explorer_state.get_active_pane().remote.is_empty();
        let (parent_fs, archive_name) = parse_parent_and_child(&archive_path);
        
        match mode_idx {
            0 => {
                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                self.execute_archive_decompress(archive_path, parent_fs, tx.clone()).await;
            }
            1 => {
                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                let folder_name = strip_archive_extensions(&archive_name);
                let dest_fs = if is_remote_empty {
                    PathBuf::from(&parent_fs).join(&folder_name).to_string_lossy().to_string()
                } else {
                    format!("{}/{}", parent_fs.trim_end_matches('/'), folder_name)
                };
                self.execute_archive_decompress(archive_path, dest_fs, tx.clone()).await;
            }
            2 => {
                self.explorer_state.popup = ui::explorer::ExplorerPopup::DecompressPathInput {
                    archive_path,
                    selected_idx: 0,
                };
            }
            _ => {}
        }
    }

    async fn execute_archive_decompress(
        &mut self,
        archive_path: String,
        dest_fs: String,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        let (parent_fs, archive_name) = parse_parent_and_child(&archive_path);
        let escaped_parent = parent_fs.replace("\"", "\\\"");
        let src_fs = format!(":archive,src_fs=\"{}\":{}", escaped_parent, archive_name);
        
        self.explorer_state.popup = ui::explorer::ExplorerPopup::CopyProgress {
            src: archive_name.clone(),
            dest: dest_fs.clone(),
            pct: 0.0,
            job_id: None,
        };
        
        let tx_clone = tx.clone();
        let src_fs_clone = src_fs.clone();
        let dest_fs_clone = dest_fs.clone();
        let archive_name_clone = archive_name.clone();
        tokio::spawn(async move {
            let res = run_rpc_job_async_with_progress(
                "sync/copy".to_string(),
                json!({
                    "srcFs": src_fs_clone,
                    "dstFs": dest_fs_clone,
                }),
                Some((archive_name_clone, dest_fs_clone, true)),
                Some(tx_clone.clone()),
            ).await;
            
            let _ = tx_clone.send(AppEvent::ExplorerOperationFinished {
                pane: ui::explorer::ActivePane::Left,
                op_name: "giải nén (extract archive)".to_string(),
                result: res,
            });
        });
    }
}

async fn run_rpc_job_async(
    method: String,
    param: serde_json::Value,
) -> Result<(), String> {
    let mut param_obj = match param {
        serde_json::Value::Object(m) => m,
        _ => serde_json::Map::new(),
    };
    param_obj.insert("_async".to_string(), serde_json::Value::Bool(true));
    let desc = if let Some(d) = param_obj.get("_description").and_then(|d| d.as_str()) {
        d.to_string()
    } else {
        let desc_str = match method.as_str() {
            "sync/copy" => {
                let src = param_obj.get("srcFs").and_then(|s| s.as_str()).unwrap_or("");
                let dst = param_obj.get("dstFs").and_then(|d| d.as_str()).unwrap_or("");
                format!("Sao chép thư mục: {} -> {}", src, dst)
            }
            "sync/move" => {
                let src = param_obj.get("srcFs").and_then(|s| s.as_str()).unwrap_or("");
                let dst = param_obj.get("dstFs").and_then(|d| d.as_str()).unwrap_or("");
                format!("Di chuyển thư mục: {} -> {}", src, dst)
            }
            "sync/sync" => {
                let src = param_obj.get("srcFs").and_then(|s| s.as_str()).unwrap_or("");
                let dst = param_obj.get("dstFs").and_then(|d| d.as_str()).unwrap_or("");
                format!("Đồng bộ thư mục: {} -> {}", src, dst)
            }
            "operations/copyfile" => {
                let remote = param_obj.get("srcRemote").and_then(|r| r.as_str()).unwrap_or("");
                format!("Sao chép tệp: {}", remote)
            }
            "operations/movefile" => {
                let remote = param_obj.get("srcRemote").and_then(|r| r.as_str()).unwrap_or("");
                format!("Di chuyển tệp: {}", remote)
            }
            "operations/deletefile" => {
                let remote = param_obj.get("remote").and_then(|r| r.as_str()).unwrap_or("");
                format!("Xóa tệp: {}", remote)
            }
            "operations/purge" => {
                let fs = param_obj.get("fs").and_then(|r| r.as_str()).unwrap_or("");
                format!("Xóa thư mục: {}", fs)
            }
            "operations/mkdir" => {
                let fs = param_obj.get("fs").and_then(|r| r.as_str()).unwrap_or("");
                format!("Tạo thư mục: {}", fs)
            }
            "operations/rmdir" => {
                let fs = param_obj.get("fs").and_then(|r| r.as_str()).unwrap_or("");
                format!("Xóa thư mục rỗng: {}", fs)
            }
            "operations/rmdirs" => {
                let fs = param_obj.get("fs").and_then(|r| r.as_str()).unwrap_or("");
                format!("Xóa các thư mục rỗng đệ quy: {}", fs)
            }
            "operations/cleanup" => {
                let fs = param_obj.get("fs").and_then(|r| r.as_str()).unwrap_or("");
                format!("Dọn dẹp: {}", fs)
            }
            _ => format!("Tác vụ: {}", method),
        };
        param_obj.insert("_description".to_string(), serde_json::Value::String(desc_str.clone()));
        desc_str
    };
    let param_str = serde_json::Value::Object(param_obj).to_string();

    let op_res = rclone::rpc_async(method, param_str).await;
    let mut job_id = None;
    if let Ok(r) = op_res {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&r.output) {
            job_id = val.get("jobid").and_then(|j| j.as_i64());
        }
    }

    if let Some(id) = job_id {
        register_job_description(id, desc);
        let mut status = "running".to_string();
        let mut err_msg = String::new();
        while status == "running" {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let status_res = rclone::rpc_async(
                "job/status".to_string(),
                json!({ "jobid": id }).to_string(),
            )
            .await;
            if let Ok(sr) = status_res {
                if let Ok(sval) = serde_json::from_str::<serde_json::Value>(&sr.output) {
                    if let Some(finished) = sval.get("finished").and_then(|f| f.as_bool()) {
                        if finished {
                            if let Some(err) = sval.get("error").and_then(|e| e.as_str()) {
                                if !err.is_empty() {
                                    status = "failed".to_string();
                                    err_msg = err.to_string();
                                } else {
                                    status = "success".to_string();
                                }
                            } else {
                                status = "success".to_string();
                            }
                            break;
                        }
                    }
                }
            }
        }
        if status == "success" {
            Ok(())
        } else {
            Err(err_msg)
        }
    } else {
        Err("Không lấy được Job ID từ Rclone".to_string())
    }
}

async fn run_rpc_job_async_with_progress(
    method: String,
    param: serde_json::Value,
    progress_info: Option<(String, String, bool)>,
    tx: Option<tokio::sync::mpsc::UnboundedSender<AppEvent>>,
) -> Result<(), String> {
    let mut param_obj = match param {
        serde_json::Value::Object(m) => m,
        _ => serde_json::Map::new(),
    };
    param_obj.insert("_async".to_string(), serde_json::Value::Bool(true));
    let desc = if let Some(d) = param_obj.get("_description").and_then(|d| d.as_str()) {
        d.to_string()
    } else {
        let desc_str = match &progress_info {
            Some((src, dest, is_copy)) => {
                if *is_copy {
                    format!("Sao chép: {} -> {}", src, dest)
                } else {
                    format!("Di chuyển: {} -> {}", src, dest)
                }
            }
            None => {
                match method.as_str() {
                    "sync/copy" => "Sao chép thư mục".to_string(),
                    "sync/move" => "Di chuyển thư mục".to_string(),
                    _ => format!("Tác vụ: {}", method),
                }
            }
        };
        param_obj.insert("_description".to_string(), serde_json::Value::String(desc_str.clone()));
        desc_str
    };
    let param_str = serde_json::Value::Object(param_obj).to_string();

    let op_res = rclone::rpc_async(method, param_str).await;
    let mut job_id = None;
    if let Ok(r) = op_res {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&r.output) {
            job_id = val.get("jobid").and_then(|j| j.as_i64());
        }
    }

    if let Some(id) = job_id {
        register_job_description(id, desc);
        let mut status = "running".to_string();
        let mut err_msg = String::new();
        while status == "running" {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            if let Some((ref src, ref dest, is_copy)) = progress_info {
                if let Some(ref tx_sender) = tx {
                    if let Ok(stats_res) = rclone::rpc_async("core/stats".to_string(), json!({ "group": format!("job/{}", id) }).to_string()).await {
                        if let Ok(stats_val) = serde_json::from_str::<serde_json::Value>(&stats_res.output) {
                            let src_filename = std::path::Path::new(src)
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| src.clone());

                            let mut found_pct = None;
                            if let Some(transfers) = stats_val.get("transferring").and_then(|t| t.as_array()) {
                                for t_val in transfers {
                                    let t_name = t_val.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                    if t_name == src_filename || src.ends_with(t_name) || t_name.ends_with(&src_filename) {
                                        if let Some(p) = t_val.get("percentage").and_then(|p| p.as_f64()) {
                                            found_pct = Some(p);
                                            break;
                                        }
                                    }
                                }
                            }

                            let pct = found_pct.unwrap_or_else(|| {
                                let bytes = stats_val.get("bytes").and_then(|b| b.as_f64()).unwrap_or(0.0);
                                let total_bytes = stats_val.get("totalBytes").and_then(|t| t.as_f64()).unwrap_or(0.0);
                                if total_bytes > 0.0 {
                                    (bytes / total_bytes) * 100.0
                                } else {
                                    0.0
                                }
                            });

                            let display_pct = pct.min(99.0);

                            if is_copy {
                                let _ = tx_sender.send(AppEvent::CopyProgress {
                                    src: src.clone(),
                                    dest: dest.clone(),
                                    pct: display_pct,
                                    job_id: Some(id),
                                });
                            } else {
                                let _ = tx_sender.send(AppEvent::MoveProgress {
                                    src: src.clone(),
                                    dest: dest.clone(),
                                    pct: display_pct,
                                    job_id: Some(id),
                                });
                            }
                        }
                    }
                }
            }

            let status_res = rclone::rpc_async(
                "job/status".to_string(),
                json!({ "jobid": id }).to_string(),
            )
            .await;
            if let Ok(sr) = status_res {
                if let Ok(sval) = serde_json::from_str::<serde_json::Value>(&sr.output) {
                    if let Some(finished) = sval.get("finished").and_then(|f| f.as_bool()) {
                        if finished {
                            if let Some(err) = sval.get("error").and_then(|e| e.as_str()) {
                                if !err.is_empty() {
                                    status = "failed".to_string();
                                    err_msg = err.to_string();
                                } else {
                                    status = "success".to_string();
                                }
                            } else {
                                status = "success".to_string();
                            }
                            break;
                        }
                    }
                }
            }
        }

        if let Some((ref src, ref dest, is_copy)) = progress_info {
            if let Some(ref tx_sender) = tx {
                if is_copy {
                    let _ = tx_sender.send(AppEvent::CopyProgress {
                        src: src.clone(),
                        dest: dest.clone(),
                        pct: 100.0,
                        job_id: Some(id),
                    });
                } else {
                    let _ = tx_sender.send(AppEvent::MoveProgress {
                        src: src.clone(),
                        dest: dest.clone(),
                        pct: 100.0,
                        job_id: Some(id),
                    });
                }
            }
        }

        if status == "success" {
            Ok(())
        } else {
            Err(err_msg)
        }
    } else {
        Err("Không lấy được Job ID từ Rclone".to_string())
    }
}
 
fn strip_archive_extensions(name: &str) -> String {
    let name_lower = name.to_lowercase();
    if name_lower.ends_with(".tar.gz") {
        name[..name.len() - 7].to_string()
    } else if name_lower.ends_with(".tar.xz") {
        name[..name.len() - 7].to_string()
    } else if name_lower.ends_with(".zip")
        || name_lower.ends_with(".tar")
        || name_lower.ends_with(".tgz")
        || name_lower.ends_with(".rar")
        || name_lower.ends_with(".sqfs")
    {
        name[..name.len() - 4].to_string()
    } else {
        name.to_string()
    }
}

fn parse_parent_and_child(fs: &str) -> (String, String) {
    let (remote_part, path_part) = if let Some(idx) = fs.find(':') {
        (format!("{}:", &fs[..idx]), &fs[idx+1..])
    } else {
        (String::new(), fs)
    };
    if let Some(idx) = path_part.rfind('/') {
        let parent = &path_part[..idx];
        let name = &path_part[idx+1..];
        (format!("{}{}", remote_part, parent), name.to_string())
    } else {
        (remote_part, path_part.to_string())
    }
}

fn copy_to_system_clipboard(text: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if Command::new("clip").stdin(std::process::Stdio::piped()).spawn().is_ok() {
            let mut child = Command::new("clip")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            }
            let _ = child.wait();
            return Ok(());
        }
    }

    #[cfg(target_os = "macos")]
    {
        if Command::new("pbcopy").stdin(std::process::Stdio::piped()).spawn().is_ok() {
            let mut child = Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            }
            let _ = child.wait();
            return Ok(());
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if Command::new("xclip").arg("-selection").arg("clipboard").stdin(std::process::Stdio::piped()).spawn().is_ok() {
            let mut child = Command::new("xclip")
                .arg("-selection")
                .arg("clipboard")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            }
            let _ = child.wait();
            return Ok(());
        }
        if Command::new("xsel").arg("--clipboard").arg("--input").stdin(std::process::Stdio::piped()).spawn().is_ok() {
            let mut child = Command::new("xsel")
                .arg("--clipboard")
                .arg("--input")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            }
            let _ = child.wait();
            return Ok(());
        }
        if Command::new("wl-copy").stdin(std::process::Stdio::piped()).spawn().is_ok() {
            let mut child = Command::new("wl-copy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            }
            let _ = child.wait();
            return Ok(());
        }
    }
    Err("Không tìm thấy tiện ích clipboard nào trên hệ thống".to_string())
}

#[allow(dead_code)]
fn parse_cmdline(cmdline: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = cmdline.chars().peekable();
    
    while let Some(c) = chars.next() {
        if c == '"' {
            in_quotes = !in_quotes;
        } else if c.is_whitespace() && !in_quotes {
            if !current.is_empty() {
                args.push(current.clone());
                current.clear();
            }
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

fn get_rclone_cmd() -> String {
    if let Ok(mut exe_path) = std::env::current_exe() {
        exe_path.pop(); // Thư mục chứa file exe hiện tại
        let local_rclone = if cfg!(target_os = "windows") {
            exe_path.join("rclone.exe")
        } else {
            exe_path.join("rclone")
        };
        if local_rclone.exists() {
            return local_rclone.to_string_lossy().to_string();
        }
    }
    "rclone".to_string()
}

