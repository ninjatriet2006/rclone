use crate::rclone;
use crate::ui;
use crate::app_config::AppConfig;
use crossterm::{
    event::{self, Event, KeyEvent},
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

pub(crate) fn handle_input_key(
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

lazy_static::lazy_static! {
    pub(crate) static ref RUNNING_SIZE_CHECKS: std::sync::Mutex<std::collections::HashSet<String>> = std::sync::Mutex::new(std::collections::HashSet::new());
    pub(crate) static ref JOB_DESCRIPTIONS: std::sync::Mutex<std::collections::HashMap<i64, String>> = std::sync::Mutex::new(std::collections::HashMap::new());
    pub(crate) static ref JOB_REAL_SIZES: std::sync::Mutex<std::collections::HashMap<i64, u64>> = std::sync::Mutex::new(std::collections::HashMap::new());
    pub(crate) static ref JOB_DIRECTIONS: std::sync::Mutex<std::collections::HashMap<i64, JobDirection>> = std::sync::Mutex::new(std::collections::HashMap::new());
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum JobDirection {
    Upload,
    Download,
    Local,
    RemoteToRemote,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActiveOperation {
    pub id: String,
    pub action_type: String, // "copy", "move", "delete", "purge", etc.
    pub src: String,
    pub dest: String,
    pub items: Vec<String>,
    pub is_dir: bool,
    pub use_checksum: bool,
    pub is_copy: bool,
    pub completed_items: Option<Vec<String>>,
}

pub fn register_job_direction(job_id: i64, direction: JobDirection) {
    if let Ok(mut map) = JOB_DIRECTIONS.lock() {
        map.insert(job_id, direction);
    }
}

pub fn get_job_direction(job_id: i64) -> Option<JobDirection> {
    if let Ok(map) = JOB_DIRECTIONS.lock() {
        map.get(&job_id).cloned()
    } else {
        None
    }
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

pub fn register_job_real_size(job_id: i64, size: u64) {
    if let Ok(mut map) = JOB_REAL_SIZES.lock() {
        map.insert(job_id, size);
    }
}

pub fn get_job_real_size(job_id: i64) -> Option<u64> {
    if let Ok(map) = JOB_REAL_SIZES.lock() {
        map.get(&job_id).cloned()
    } else {
        None
    }
}

pub fn save_active_operation(op: &ActiveOperation) {
    let path = crate::app_config::AppConfig::config_dir().join("active_ops.json");
    let mut ops = load_active_operations();
    ops.push(op.clone());
    if let Ok(serialized) = serde_json::to_string_pretty(&ops) {
        let _ = std::fs::write(path, serialized);
    }
}

pub fn complete_item_in_active_operation(id: &str, item_name: &str) {
    let path = crate::app_config::AppConfig::config_dir().join("active_ops.json");
    let mut ops = load_active_operations();
    let mut modified = false;
    for op in &mut ops {
        if op.id == id {
            if let Some(pos) = op.items.iter().position(|x| x == item_name) {
                op.items.remove(pos);
                if op.completed_items.is_none() {
                    op.completed_items = Some(Vec::new());
                }
                op.completed_items.as_mut().unwrap().push(item_name.to_string());
                modified = true;
            }
            break;
        }
    }
    if modified {
        if let Ok(serialized) = serde_json::to_string_pretty(&ops) {
            let _ = std::fs::write(path, serialized);
        }
    }
}

pub fn remove_active_operation(id: &str) {
    let path = crate::app_config::AppConfig::config_dir().join("active_ops.json");
    let mut ops = load_active_operations();
    ops.retain(|o| o.id != id);
    if let Ok(serialized) = serde_json::to_string_pretty(&ops) {
        let _ = std::fs::write(path, serialized);
    }
}

pub fn load_active_operations() -> Vec<ActiveOperation> {
    let path = crate::app_config::AppConfig::config_dir().join("active_ops.json");
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(ops) = serde_json::from_str::<Vec<ActiveOperation>>(&content) {
                return ops;
            }
        }
    }
    Vec::new()
}

pub fn clear_active_operations() {
    let path = crate::app_config::AppConfig::config_dir().join("active_ops.json");
    let _ = std::fs::remove_file(path);
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Screen {
    MainMenu,
    ConnectionManager,
    FileExplorer,
    JobMonitor,
    ConfigProfileManager,
    ServicesAndMounts,
    LanguageSelect,
    DependencyManager,
}

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
        upload_speed: f64,
        download_speed: f64,
        transferred: u64,
        total: u64,
        active: Vec<ui::monitor::TransferJob>,
        active_transfers: usize,
        active_checks: usize,
    },
    OAuthFinished {
        result: Result<(), String>,
    },
    OAuthUrlReceived {
        url: String,
    },
    #[allow(dead_code)]
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
        use_checksum: bool,
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
    PermissionErrorDetected {
        src: String,
        dest: String,
        is_dir: bool,
        restricted_files: Vec<String>,
        use_checksum: bool,
        #[allow(dead_code)]
        total_files: u64,
        #[allow(dead_code)]
        total_size: u64,
    },
    PermissionCheckPassed {
        src: String,
        dest: String,
        is_dir: bool,
        use_checksum: bool,
        #[allow(dead_code)]
        total_files: u64,
        total_size: u64,
    },
    MultiPermissionErrorDetected {
        items: Vec<ui::explorer::ClipboardItem>,
        dest_remote: String,
        dest_path: String,
        restricted_files: Vec<String>,
        use_checksum: bool,
    },
    MultiPermissionCheckPassed {
        items: Vec<ui::explorer::ClipboardItem>,
        dest_remote: String,
        dest_path: String,
        use_checksum: bool,
    },
    PermissionScanProgress {
        src: String,
        dest: String,
        is_dir: bool,
        scanned_count: usize,
        total_files: usize,
        restricted_count: usize,
    },
    MergeSimilarScanProgress {
        folders_count: usize,
        scanned_count: usize,
    },
    MergeSimilarScanFinished {
        result: Result<(Vec<String>, ui::explorer::TreeNode), String>,
        folders: Vec<ui::explorer::FileItem>,
        destination_idx: usize,
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

pub(crate) struct ScanState {
    pub(crate) queue: Vec<String>,
    pub(crate) active_tasks: usize,
    pub(crate) files: Vec<String>,
    pub(crate) restricted_files: Vec<String>,
    pub(crate) total_size: u64,
}

pub(crate) struct MultiScanState {
    pub(crate) queue: Vec<(String, String)>,
    pub(crate) active_tasks: usize,
    pub(crate) files_count: usize,
    pub(crate) restricted: Vec<String>,
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
    pub remote_types: std::collections::HashMap<String, String>,
    pub features_cache: std::collections::HashMap<String, serde_json::Value>,
    pub last_services_scan: std::time::Instant,
    pub last_stats_scan: std::time::Instant,
    pub stats_scan_in_progress: bool,
    pub fuse_installed: bool,
    pub filen_cli_installed: bool,
    pub selected_dependency_idx: usize,
    pub skip_permission_precheck: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

mod services;
mod permission;
mod operations;
mod event_handlers;
pub(crate) mod key_handlers;

pub use operations::inject_optimal_thread_config;

impl App {
    pub(crate) fn get_underlying_remote(config_path: &str, remote: &str) -> Option<String> {
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

        let home_dir = std::env::var("HOME").unwrap_or_default();
        let filen_cli_installed = if home_dir.is_empty() {
            false
        } else {
            std::path::Path::new(&home_dir).join(".filen-cli/bin/filen").exists()
        };
        let fuse_installed = crate::check_fuse_dependency();

        let mut monitor_state = ui::monitor::MonitorState::new();
        let saved_ops = load_active_operations();
        for op in saved_ops {
            let now_str = {
                let secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let hours = (secs / 3600) % 24;
                let minutes = (secs / 60) % 60;
                let seconds = secs % 60;
                format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
            };
            monitor_state.failed_files.push(ui::monitor::FailedCopyItem {
                src: op.src.clone(),
                dest: op.dest.clone(),
                error: "Tác vụ bị gián đoạn do crash / tắt đột ngột (Nhấn R để thử lại)".to_string(),
                time: now_str,
                is_copy: op.is_copy,
            });
        }
        clear_active_operations();

        App {
            screen: Screen::MainMenu,
            config,
            should_exit: false,
            delete_confirm: None,
            menu_state: ui::menu::MenuState::new(),
            connection_state: ui::connection::ConnectionState::new(),
            explorer_state: ui::explorer::ExplorerState::new(),
            monitor_state,
            profile_state: ui::profile::ProfileState::new(),
            services_state: ui::services::ServicesState::new(),
            available_languages,
            selected_lang_idx,
            status_trigger_tx: None,
            remote_dependencies: std::collections::HashMap::new(),
            remote_types: std::collections::HashMap::new(),
            features_cache,
            last_services_scan: std::time::Instant::now(),
            last_stats_scan: std::time::Instant::now(),
            stats_scan_in_progress: false,
            fuse_installed,
            filen_cli_installed,
            selected_dependency_idx: 0,
            skip_permission_precheck: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

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
            let is_fuse_installed = self.fuse_installed;
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
                        ui::connection::draw(&self.connection_state, f, main_layout[1], self.filen_cli_installed)
                    }
                    Screen::FileExplorer => {
                        ui::explorer::draw(&mut self.explorer_state, f, main_layout[1])
                    }
                    Screen::JobMonitor => ui::monitor::draw(&mut self.monitor_state, f, main_layout[1]),
                    Screen::ConfigProfileManager => ui::profile::draw(
                        &self.profile_state,
                        f,
                        main_layout[1],
                        &active_profile_name,
                    ),
                    Screen::ServicesAndMounts => {
                        ui::services::draw(&self.services_state, f, main_layout[1], self.fuse_installed)
                    }
                    Screen::LanguageSelect => self.draw_language_select(f, main_layout[1]),
                    Screen::DependencyManager => self.draw_dependency_manager(f, main_layout[1]),
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
                        upload_speed,
                        download_speed,
                        transferred,
                        total,
                        active,
                        active_transfers,
                        active_checks,
                    } => {
                        self.monitor_state.speed = speed;
                        self.monitor_state.upload_speed = upload_speed;
                        self.monitor_state.download_speed = download_speed;
                        self.monitor_state.bytes_transferred = transferred;
                        self.monitor_state.total_bytes = total;
                        self.monitor_state.active_jobs = active;
                        self.monitor_state.active_transfers = active_transfers;
                        self.monitor_state.active_checks = active_checks;

                        // Dựng lại cây thư mục node hiển thị phẳng
                        self.monitor_state.rebuild_visible_nodes();

                        if self.monitor_state.selected_node_idx >= self.monitor_state.visible_nodes.len() {
                            self.monitor_state.selected_node_idx = 0;
                        }

                        // Auto-bandwidth calibration: check if speed is higher than config limit and update it
                        self.monitor_state.max_bandwidth = self.config.max_bandwidth_bytes_per_sec;
                        if speed > self.config.max_bandwidth_bytes_per_sec as f64 {
                            self.config.max_bandwidth_bytes_per_sec = speed as u64;
                            self.monitor_state.max_bandwidth = speed as u64;
                            let _ = self.config.save();
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
                        for (dep, status_val) in updates {
                            self.connection_state.remote_statuses.insert(dep, status_val);
                        }
                    }
                    AppEvent::ExplorerOperationFinished { pane, op_name, result } => {
                        // Reload pane
                        self.refresh_explorer_pane(pane, tx.clone()).await;

                        match result {
                            Ok(_) => {
                                self.explorer_state.notification = Some((
                                    "THÀNH CÔNG".to_string(),
                                    format!("Đã thực hiện xong tác vụ {}", op_name),
                                ));
                            }
                            Err(e) => {
                                self.explorer_state.notification = Some((
                                    "LỖI TÁC VỤ".to_string(),
                                    format!("Lỗi khi thực hiện {}: {}", op_name, e),
                                ));
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
                        use_checksum,
                    } => {
                        self.handle_features_checked(action_type, src, dest, src_features, dst_features, is_dir, use_checksum, tx.clone()).await;
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
                                self.explorer_state.notification = Some(("LỖI ĐỌC FILE".to_string(), format!("Lỗi đọc file: {}", e)));
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
                                    self.explorer_state.notification = Some(("LỖI TẢI THƯ MỤC".to_string(), format!("Lỗi tải thư mục: {}", e)));
                                }
                            }
                        }
                    }
                    AppEvent::CryptdecodeFinished { result } => {
                        if let ui::explorer::ExplorerPopup::CryptdecodeForm {
                            ref mut output_result,
                            ..
                        } = self.explorer_state.popup
                        {
                            match result {
                                Ok(decrypted) => {
                                    *output_result = Some(format!("Kết quả giải mã:\n{}", decrypted));
                                }
                                Err(e) => {
                                    *output_result = Some(format!("Lỗi giải mã: {}", e));
                                }
                            }
                        }
                    }
                    AppEvent::CryptdecodeResult { result } => {
                        if let ui::explorer::ExplorerPopup::SpecialActionMessage {
                            ref mut message,
                            ..
                        } = self.explorer_state.popup
                        {
                            match result {
                                Ok(res) => {
                                    *message = res;
                                }
                                Err(e) => {
                                    *message = format!("Thất bại: {}", e);
                                }
                            }
                        }
                    }
                    AppEvent::PermissionErrorDetected { src, dest, is_dir, restricted_files, use_checksum, .. } => {
                        if let ui::explorer::ExplorerPopup::PermissionScanning { .. } = self.explorer_state.popup {
                            let mut options = Vec::new();
                            let mut actions = Vec::new();

                            options.push(crate::lang::translate("exp_permission_option_cancel").to_string());
                            actions.push(ui::explorer::FallbackAction::PermissionCancel);

                            options.push(crate::lang::translate("exp_permission_option_as_much").to_string());
                            actions.push(ui::explorer::FallbackAction::PermissionCopyAsMuchAsPossible {
                                src: src.clone(),
                                dest: dest.clone(),
                                is_dir,
                                restricted_files: restricted_files.clone(),
                                use_checksum,
                            });

                            options.push(crate::lang::translate("exp_permission_option_restricted").to_string());
                            actions.push(ui::explorer::FallbackAction::PermissionRestrictedCopy {
                                src: src.clone(),
                                dest: dest.clone(),
                                is_dir,
                                restricted_files: restricted_files.clone(),
                                use_checksum,
                            });

                            self.explorer_state.popup = ui::explorer::ExplorerPopup::ConfirmFallback {
                                title: crate::lang::translate("exp_permission_error_title").to_string(),
                                options,
                                selected_idx: 0,
                                actions,
                                restricted_files: Some(restricted_files),
                                restricted_scroll: 0,
                                focus_files: false,
                            };
                        } else {
                            if let Some(job) = self.monitor_state.pending_jobs.iter_mut().find(|j| j.src == src && j.dest == dest) {
                                job.restricted_files = restricted_files;
                                job.status = "Scanned (Has Restrictions)".to_string();
                            }
                        }
                    }
                    AppEvent::PermissionCheckPassed { src, dest, is_dir, use_checksum, total_files: _, total_size } => {
                        if let ui::explorer::ExplorerPopup::PermissionScanning { .. } = self.explorer_state.popup {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::CopyProgress {
                                src: src.clone(),
                                dest: dest.clone(),
                                pct: 0.0,
                                job_id: None,
                            };
                            let tx_copy = tx.clone();
                            let src_clone = src.clone();
                            let dest_clone = dest.clone();
                            let max_bw = self.config.max_bandwidth_bytes_per_sec;
                            tokio::spawn(async move {
                                let mut param = json!({
                                    "srcFs": src_clone,
                                    "dstFs": dest_clone,
                                });
                                // Tối ưu hóa số luồng dựa trên kích thước file và băng thông
                                let _ = crate::app::inject_optimal_thread_config(&mut param, &src_clone, is_dir, max_bw).await;
                                if use_checksum {
                                    if let Some(obj) = param.as_object_mut() {
                                        let mut config_obj = match obj.get("_config") {
                                            Some(serde_json::Value::Object(m)) => m.clone(),
                                            _ => serde_json::Map::new(),
                                        };
                                        config_obj.insert("checksum".to_string(), json!(true));
                                        obj.insert("_config".to_string(), serde_json::Value::Object(config_obj));
                                    }
                                }
                                let res = run_rpc_job_async_with_progress(
                                    "sync/copy".to_string(),
                                    param,
                                    Some((src_clone, dest_clone, true)),
                                    Some(tx_copy.clone()),
                                    Some(total_size),
                                ).await;
                                let _ = tx_copy.send(AppEvent::ExplorerOperationFinished {
                                    pane: ui::explorer::ActivePane::Left,
                                    op_name: "sao chép (copy)".to_string(),
                                    result: res,
                                });
                            });
                        } else {
                            if let Some(job) = self.monitor_state.pending_jobs.iter_mut().find(|j| j.src == src && j.dest == dest) {
                                job.status = "Scanned (No Restrictions)".to_string();
                            }
                        }
                    }
                    AppEvent::MultiPermissionErrorDetected { items, dest_remote, dest_path, restricted_files, use_checksum } => {
                        let dest_full = if dest_remote.is_empty() { dest_path.clone() } else { format!("{}:{}", dest_remote, dest_path) };
                        if let ui::explorer::ExplorerPopup::PermissionScanning { .. } = self.explorer_state.popup {
                            let mut options = Vec::new();
                            let mut actions = Vec::new();

                            options.push(crate::lang::translate("exp_permission_option_cancel").to_string());
                            actions.push(ui::explorer::FallbackAction::PermissionCancel);

                            options.push(crate::lang::translate("exp_permission_option_as_much").to_string());
                            actions.push(ui::explorer::FallbackAction::MultiPermissionCopyAsMuchAsPossible {
                                items: items.clone(),
                                dest_remote: dest_remote.clone(),
                                dest_path: dest_path.clone(),
                                restricted_files: restricted_files.clone(),
                                use_checksum,
                            });

                            options.push(crate::lang::translate("exp_permission_option_restricted").to_string());
                            actions.push(ui::explorer::FallbackAction::MultiPermissionRestrictedCopy {
                                items: items.clone(),
                                dest_remote: dest_remote.clone(),
                                dest_path: dest_path.clone(),
                                restricted_files: restricted_files.clone(),
                                use_checksum,
                            });

                            self.explorer_state.popup = ui::explorer::ExplorerPopup::ConfirmFallback {
                                title: crate::lang::translate("exp_permission_error_title").to_string(),
                                options,
                                selected_idx: 0,
                                actions,
                                restricted_files: Some(restricted_files),
                                restricted_scroll: 0,
                                focus_files: false,
                            };
                        } else {
                            let src_full = format!("({} mục)", items.len());
                            if let Some(job) = self.monitor_state.pending_jobs.iter_mut().find(|j| j.src == src_full && j.dest == dest_full) {
                                job.restricted_files = restricted_files;
                                job.status = "Scanned (Has Restrictions)".to_string();
                            }
                        }
                    }
                    AppEvent::MultiPermissionCheckPassed { items, dest_remote, dest_path, use_checksum } => {
                        let dest_full = if dest_remote.is_empty() { dest_path.clone() } else { format!("{}:{}", dest_remote, dest_path) };
                        let src_full = format!("({} mục)", items.len());
                        if let ui::explorer::ExplorerPopup::PermissionScanning { .. } = self.explorer_state.popup {
                            let dest_remote_clone = dest_remote.clone();
                            let dest_path_clone = dest_path.clone();
                            let items_clone = items.clone();
                            let tx_op = tx.clone();
                            let pane_type = self.explorer_state.active_pane.clone();

                            self.explorer_state.popup = ui::explorer::ExplorerPopup::CopyProgress {
                                src: format!("({} mục)", items_clone.len()),
                                dest: dest_full.clone(),
                                pct: 0.0,
                                job_id: None,
                            };

                            let op_id = format!("copy_multi_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                            let op = ActiveOperation {
                                id: op_id.clone(),
                                action_type: "copy".to_string(),
                                src: if items_clone.is_empty() {
                                    String::new()
                                } else {
                                    let item = &items_clone[0];
                                    if item.remote.is_empty() {
                                        item.path.clone()
                                    } else {
                                        format!("{}:{}", item.remote.trim_end_matches(':'), item.path)
                                    }
                                },
                                dest: dest_full.clone(),
                                items: items_clone.iter().map(|item| item.name.clone()).collect(),
                                is_dir: true,
                                use_checksum,
                                is_copy: true,
                                completed_items: Some(Vec::new()),
                            };
                            save_active_operation(&op);

                            tokio::spawn(async move {
                                let mut last_err = None;
                                for item in items_clone {
                                    let item_src = if item.remote.is_empty() {
                                        PathBuf::from(&item.path)
                                            .join(&item.name)
                                            .to_string_lossy()
                                            .to_string()
                                    } else {
                                        let clean_remote = item.remote.trim_end_matches(':');
                                        let clean_path = if item.path.starts_with('/') {
                                            item.path.clone()
                                        } else {
                                            format!("/{}", item.path)
                                        };
                                        if clean_path.ends_with('/') {
                                            format!("{}:{}{}", clean_remote, clean_path, item.name)
                                        } else {
                                            format!("{}:{}/{}", clean_remote, clean_path, item.name)
                                        }
                                    };

                                    let item_dest = if dest_remote_clone.is_empty() {
                                        PathBuf::from(&dest_path_clone)
                                            .join(&item.name)
                                            .to_string_lossy()
                                            .to_string()
                                    } else {
                                        let clean_remote = dest_remote_clone.trim_end_matches(':');
                                        let clean_path = if dest_path_clone.starts_with('/') {
                                            dest_path_clone.clone()
                                        } else {
                                            format!("/{}", dest_path_clone)
                                        };
                                        if clean_path.ends_with('/') {
                                            format!("{}:{}{}", clean_remote, clean_path, item.name)
                                        } else {
                                            format!("{}:{}/{}", clean_remote, clean_path, item.name)
                                        }
                                    };

                                    let method = "sync/copy".to_string();
                                    let mut param = json!({
                                        "srcFs": item_src,
                                        "dstFs": item_dest,
                                    });
                                    if use_checksum {
                                        if let Some(obj) = param.as_object_mut() {
                                            obj.insert("_config".to_string(), json!({ "checksum": true }));
                                        }
                                    }

                                    let res = run_rpc_job_async_with_progress(
                                        method,
                                        param,
                                        Some((item_src.clone(), item_dest.clone(), true)),
                                        Some(tx_op.clone()),
                                        None,
                                    ).await;

                                    match res {
                                        Ok(_) => {
                                            complete_item_in_active_operation(&op_id, &item.name);
                                        }
                                        Err(e) => {
                                            last_err = Some(e);
                                        }
                                    }
                                }

                                remove_active_operation(&op_id);

                                let final_result = match last_err {
                                    None => Ok(()),
                                    Some(e) => Err(e),
                                };

                                let _ = tx_op.send(AppEvent::ExplorerOperationFinished {
                                    pane: pane_type,
                                    op_name: "sao chép nhiều mục".to_string(),
                                    result: final_result,
                                });
                            });
                        } else {
                            if let Some(job) = self.monitor_state.pending_jobs.iter_mut().find(|j| j.src == src_full && j.dest == dest_full) {
                                job.status = "Scanned (No Restrictions)".to_string();
                            }
                        }
                    }
                    AppEvent::PermissionScanProgress { src, dest, is_dir, scanned_count, total_files, restricted_count } => {
                        if let ui::explorer::ExplorerPopup::PermissionScanning { .. } = self.explorer_state.popup {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::PermissionScanning {
                                src,
                                dest,
                                is_dir,
                                scanned_count,
                                total_files,
                                restricted_count,
                            };
                        }
                    }
                    AppEvent::MergeSimilarScanProgress { folders_count, scanned_count } => {
                        if let ui::explorer::ExplorerPopup::MergeSimilarScanning { .. } = self.explorer_state.popup {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::MergeSimilarScanning {
                                folders_count,
                                scanned_count,
                            };
                        }
                    }
                    AppEvent::MergeSimilarScanFinished { result, folders, destination_idx } => {
                        if let ui::explorer::ExplorerPopup::MergeSimilarScanning { .. } = self.explorer_state.popup {
                            match result {
                                Ok((summary_report, tree_root)) => {
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::MergeSimilarPreview {
                                        summary_report,
                                        tree_root,
                                        expanded_paths: std::collections::HashSet::new(),
                                        selected_rel_path: String::new(),
                                        scroll_offset: 0,
                                        folders,
                                        destination_idx,
                                    };
                                }
                                Err(e) => {
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                                    self.explorer_state.notification = Some(("LỖI QUÉT THƯ MỤC".to_string(), e));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Tắt Alternate Screen và giải phóng raw mode khi thoát app
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        Ok(())
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
                .title(" CHỌN NGÔN NGỮ (LANGUAGE SELECT) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );
        f.render_widget(list, chunks[1]);
    }

    fn draw_dependency_manager(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        use ratatui::widgets::{List, ListItem};
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Hướng dẫn
                Constraint::Min(5),    // Danh sách
            ])
            .split(area);

        let welcome = Paragraph::new("Dùng các phím Mũi tên để di chuyển, Enter để cài đặt phụ thuộc đã chọn. Esc để quay lại Menu chính.")
            .style(Style::default())
            .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));
        f.render_widget(welcome, chunks[0]);

        let fuse_status = if self.fuse_installed { "Đã cài đặt (Installed)" } else { "Chưa cài đặt (Not installed)" };
        let filen_status = if self.filen_cli_installed { "Đã cài đặt (Installed)" } else { "Chưa cài đặt (Not installed)" };

        let items = vec![
            ListItem::new(format!("1. Tiện ích FUSE (Hỗ trợ Mount ổ đĩa ảo) - Trạng thái: {}", fuse_status)),
            ListItem::new(format!("2. Filen CLI (Hỗ trợ đồng bộ Filen) - Trạng thái: {}", filen_status)),
        ];

        let styled_items: Vec<ListItem> = items
            .into_iter()
            .enumerate()
            .map(|(i, item)| {
                let style = if i == self.selected_dependency_idx {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                item.style(style)
            })
            .collect();

        let list = List::new(styled_items).block(
            Block::default()
                .title(" QUẢN LÝ CÀI ĐẶT PHỤ THUỘC (DEPENDENCY MANAGER) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );
        f.render_widget(list, chunks[1]);
    }
}

pub(crate) async fn run_rpc_job_async(
    method: String,
    param: serde_json::Value,
) -> Result<(), String> {
    let mut param_obj = match param {
        serde_json::Value::Object(m) => m,
        _ => serde_json::Map::new(),
    };

    // Tự động tiêm cấu hình tối ưu số luồng cho các tác vụ truyền tải nếu chưa có
    if method == "sync/copy" || method == "sync/move" || method == "sync/sync" {
        if let Some(src_fs) = param_obj.get("srcFs").and_then(|s| s.as_str()).map(|s| s.to_string()) {
            let config = crate::app_config::AppConfig::load();
            let max_bw = config.max_bandwidth_bytes_per_sec;
            let has_thread_config = if let Some(serde_json::Value::Object(cfg_obj)) = param_obj.get("_config") {
                cfg_obj.contains_key("Transfers") && cfg_obj.contains_key("Checkers")
            } else {
                false
            };
            if !has_thread_config {
                let mut param_val = serde_json::Value::Object(param_obj);
                let _ = inject_optimal_thread_config(&mut param_val, &src_fs, true, max_bw).await;
                param_obj = match param_val {
                    serde_json::Value::Object(m) => m,
                    _ => serde_json::Map::new(),
                };
            }
        }
    } else if method == "operations/copyfile" || method == "operations/movefile" {
        if let Some(src_fs) = param_obj.get("srcFs").and_then(|s| s.as_str()).map(|s| s.to_string()) {
            let config = crate::app_config::AppConfig::load();
            let max_bw = config.max_bandwidth_bytes_per_sec;
            let has_thread_config = if let Some(serde_json::Value::Object(cfg_obj)) = param_obj.get("_config") {
                cfg_obj.contains_key("Transfers") && cfg_obj.contains_key("Checkers")
            } else {
                false
            };
            if !has_thread_config {
                let mut param_val = serde_json::Value::Object(param_obj);
                let _ = inject_optimal_thread_config(&mut param_val, &src_fs, false, max_bw).await;
                param_obj = match param_val {
                    serde_json::Value::Object(m) => m,
                    _ => serde_json::Map::new(),
                };
            }
        }
    }

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

    let max_attempts = crate::app_config::AppConfig::load().retries.max(1);
    let mut attempt = 0;

    loop {
        attempt += 1;
        let op_res = rclone::rpc_async(method.clone(), param_str.clone()).await;
        let mut job_id = None;
        if let Ok(r) = op_res {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&r.output) {
                job_id = val.get("jobid").and_then(|j| j.as_i64());
            }
        }

        if let Some(id) = job_id {
            register_job_description(id, desc.clone());
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
                return Ok(());
            } else {
                crate::app_config::log_info(&format!(
                    "[Auto-Retry] Job {} thất bại ở lần thử {}/{}: {}. Chuẩn bị thử lại...",
                    id, attempt, max_attempts, err_msg
                ));
                if attempt >= max_attempts {
                    return Err(err_msg);
                }
            }
        } else {
            let err_msg = "Không lấy được Job ID từ Rclone".to_string();
            crate::app_config::log_info(&format!(
                "[Auto-Retry] Lần thử {}/{} thất bại: {}. Chuẩn bị thử lại...",
                attempt, max_attempts, err_msg
            ));
            if attempt >= max_attempts {
                return Err(err_msg);
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

pub(crate) async fn run_rpc_job_async_with_progress(
    method: String,
    param: serde_json::Value,
    progress_info: Option<(String, String, bool)>,
    tx: Option<tokio::sync::mpsc::UnboundedSender<AppEvent>>,
    real_size: Option<u64>,
) -> Result<(), String> {
    let mut param_obj = match param {
        serde_json::Value::Object(m) => m,
        _ => serde_json::Map::new(),
    };

    // Tự động tiêm cấu hình tối ưu số luồng cho các tác vụ truyền tải nếu chưa có
    if method == "sync/copy" || method == "sync/move" || method == "sync/sync" {
        if let Some(src_fs) = param_obj.get("srcFs").and_then(|s| s.as_str()).map(|s| s.to_string()) {
            let config = crate::app_config::AppConfig::load();
            let max_bw = config.max_bandwidth_bytes_per_sec;
            let has_thread_config = if let Some(serde_json::Value::Object(cfg_obj)) = param_obj.get("_config") {
                cfg_obj.contains_key("Transfers") && cfg_obj.contains_key("Checkers")
            } else {
                false
            };
            if !has_thread_config {
                let mut param_val = serde_json::Value::Object(param_obj);
                let _ = inject_optimal_thread_config(&mut param_val, &src_fs, true, max_bw).await;
                param_obj = match param_val {
                    serde_json::Value::Object(m) => m,
                    _ => serde_json::Map::new(),
                };
            }
        }
    } else if method == "operations/copyfile" || method == "operations/movefile" {
        if let Some(src_fs) = param_obj.get("srcFs").and_then(|s| s.as_str()).map(|s| s.to_string()) {
            let config = crate::app_config::AppConfig::load();
            let max_bw = config.max_bandwidth_bytes_per_sec;
            let has_thread_config = if let Some(serde_json::Value::Object(cfg_obj)) = param_obj.get("_config") {
                cfg_obj.contains_key("Transfers") && cfg_obj.contains_key("Checkers")
            } else {
                false
            };
            if !has_thread_config {
                let mut param_val = serde_json::Value::Object(param_obj);
                let _ = inject_optimal_thread_config(&mut param_val, &src_fs, false, max_bw).await;
                param_obj = match param_val {
                    serde_json::Value::Object(m) => m,
                    _ => serde_json::Map::new(),
                };
            }
        }
    }

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
    let param_str = serde_json::Value::Object(param_obj.clone()).to_string();

    let max_attempts = crate::app_config::AppConfig::load().retries.max(1);
    let mut attempt = 0;

    loop {
        attempt += 1;
        let op_res = rclone::rpc_async(method.clone(), param_str.clone()).await;
        let mut job_id = None;
        if let Ok(r) = op_res {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&r.output) {
                job_id = val.get("jobid").and_then(|j| j.as_i64());
            }
        }

        if let Some(id) = job_id {
            register_job_description(id, desc.clone());
            if let Some(sz) = real_size {
                register_job_real_size(id, sz);
            }

            let dir = if let Some((ref src, ref dest, _)) = progress_info {
                let src_remote = src.contains(':');
                let dest_remote = dest.contains(':');
                if src_remote && !dest_remote {
                    JobDirection::Download
                } else if !src_remote && dest_remote {
                    JobDirection::Upload
                } else if src_remote && dest_remote {
                    JobDirection::RemoteToRemote
                } else {
                    JobDirection::Local
                }
            } else {
                if method == "sync/copy" || method == "sync/move" {
                    JobDirection::Upload
                } else {
                    JobDirection::Local
                }
            };
            register_job_direction(id, dir);

            let op_id = format!("{}", id);
            if let Some((ref src, ref dest, is_copy)) = progress_info {
                let use_checksum = param_obj.get("_config")
                    .and_then(|c| c.get("checksum"))
                    .and_then(|cs| cs.as_bool())
                    .unwrap_or(false);
                let op = ActiveOperation {
                    id: op_id.clone(),
                    action_type: if is_copy { "copy".to_string() } else { "move".to_string() },
                    src: src.clone(),
                    dest: dest.clone(),
                    items: vec![src.clone()],
                    is_dir: true,
                    use_checksum,
                    is_copy,
                    completed_items: Some(Vec::new()),
                };
                save_active_operation(&op);
            }

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

            if progress_info.is_some() {
                remove_active_operation(&op_id);
            }

            if status == "success" {
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
                return Ok(());
            } else {
                crate::app_config::log_info(&format!(
                    "[Auto-Retry] Job {} thất bại ở lần thử {}/{}: {}. Chuẩn bị thử lại...",
                    id, attempt, max_attempts, err_msg
                ));
                if attempt >= max_attempts {
                    return Err(err_msg);
                }
            }
        } else {
            let err_msg = "Không lấy được Job ID từ Rclone".to_string();
            crate::app_config::log_info(&format!(
                "[Auto-Retry] Lần thử {}/{} thất bại: {}. Chuẩn bị thử lại...",
                attempt, max_attempts, err_msg
            ));
            if attempt >= max_attempts {
                return Err(err_msg);
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

pub(crate) fn strip_archive_extensions(name: &str) -> String {
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

pub(crate) fn parse_parent_and_child(fs: &str) -> (String, String) {
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

pub(crate) fn join_fs_path(fs: &str, sub_path: &str) -> String {
    if fs.contains(':') {
        let parts: Vec<&str> = fs.splitn(2, ':').collect();
        let remote = parts[0];
        let path = parts[1];
        let joined_path = if path.is_empty() {
            sub_path.to_string()
        } else if path.ends_with('/') {
            format!("{}{}", path, sub_path)
        } else {
            format!("{}/{}", path, sub_path)
        };
        format!("{}:{}", remote, joined_path)
    } else {
        if fs.ends_with('/') {
            format!("{}{}", fs, sub_path)
        } else {
            format!("{}/{}", fs, sub_path)
        }
    }
}

pub(crate) fn copy_to_system_clipboard(text: &str) -> Result<(), String> {
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
pub(crate) fn parse_cmdline(cmdline: &str) -> Vec<String> {
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

pub(crate) fn get_rclone_cmd() -> String {
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

pub(crate) async fn create_all_source_directories(src: &str, dest: &str) -> Result<(), String> {
    // Tạo thư mục đích gốc
    let mkdir_res = rclone::rpc_async(
        "operations/mkdir".to_string(),
        serde_json::json!({
            "fs": dest,
            "remote": "",
        })
        .to_string(),
    )
    .await;
    if let Err(e) = mkdir_res {
        return Err(e);
    }

    // Liệt kê đệ quy để tìm tất cả các thư mục con ở nguồn
    let list_param = serde_json::json!({
        "fs": src,
        "remote": "",
        "opt": {
            "recurse": true
        }
    })
    .to_string();

    if let Ok(list_res) = rclone::rpc_async("operations/list".to_string(), list_param).await {
        if list_res.status == 200 {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&list_res.output) {
                if let Some(list_arr) = val.get("list").and_then(|l| l.as_array()) {
                    for item in list_arr {
                        let is_item_dir = item.get("IsDir").and_then(|d| d.as_bool()).unwrap_or(false);
                        if is_item_dir {
                            if let Some(path) = item.get("Path").and_then(|p| p.as_str()) {
                                if !path.is_empty() {
                                    // Tạo thư mục con tương ứng ở đích
                                    let _ = rclone::rpc_async(
                                        "operations/mkdir".to_string(),
                                        serde_json::json!({
                                            "fs": dest,
                                            "remote": path,
                                        })
                                        .to_string(),
                                    )
                                    .await;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) async fn execute_restricted_copy(
    src: String,
    dest: String,
    is_dir: bool,
    use_checksum: bool,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
) -> Result<(), String> {
    if !is_dir {
        let (src_fs, src_file) = parse_parent_and_child(&src);
        let (dst_fs, dst_file) = parse_parent_and_child(&dest);
        let mut param = serde_json::json!({
            "srcFs": src_fs,
            "srcRemote": src_file,
            "dstFs": dst_fs,
            "dstRemote": dst_file,
        });
        if use_checksum {
            if let Some(obj) = param.as_object_mut() {
                obj.insert("_config".to_string(), serde_json::json!({ "checksum": true }));
            }
        }
        let param_str = param.to_string();

        let res = rclone::rpc_async("operations/copyfile".to_string(), param_str).await;
        match res {
            Ok(rpc_res) => {
                if rpc_res.status == 200 {
                    let _ = tx.send(AppEvent::CopyProgress {
                        src: src.clone(),
                        dest: dest.clone(),
                        pct: 100.0,
                        job_id: None,
                    });
                    Ok(())
                } else {
                    let err_msg = rpc_res.output.to_lowercase();
                    if err_msg.contains("restrictedlink") 
                        || err_msg.contains("download") 
                        || err_msg.contains("forbidden") 
                        || err_msg.contains("only the owner")
                    {
                        let _ = tx.send(AppEvent::CopyProgress {
                            src: src.clone(),
                            dest: dest.clone(),
                            pct: 100.0,
                            job_id: None,
                        });
                        Ok(())
                    } else {
                        let err = format!("Lỗi sao chép tệp: {}", rpc_res.output);
                        Err(err)
                    }
                }
            }
            Err(e) => {
                let err_msg = e.to_lowercase();
                if err_msg.contains("restrictedlink") 
                    || err_msg.contains("download") 
                    || err_msg.contains("forbidden") 
                    || err_msg.contains("only the owner")
                {
                    let _ = tx.send(AppEvent::CopyProgress {
                        src: src.clone(),
                        dest: dest.clone(),
                        pct: 100.0,
                        job_id: None,
                    });
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    } else {
        let mkdir_param = serde_json::json!({
            "fs": dest,
            "remote": "",
        }).to_string();
        let _ = rclone::rpc_async("operations/mkdir".to_string(), mkdir_param).await;

        let list_param = serde_json::json!({
            "fs": src,
            "remote": "",
            "opt": {
                "recurse": true
            }
        }).to_string();

        let list_res = rclone::rpc_async("operations/list".to_string(), list_param).await?;
        if list_res.status != 200 {
            return Err(format!("Lỗi khi liệt kê thư mục nguồn: {}", list_res.output));
        }

        let val: serde_json::Value = serde_json::from_str(&list_res.output)
            .map_err(|e| format!("Lỗi parse JSON kết quả list: {}", e))?;

        let list_arr = match val.get("list").and_then(|l| l.as_array()) {
            Some(arr) => arr,
            None => {
                let mkdir_param = serde_json::json!({
                    "fs": dest,
                    "remote": "",
                }).to_string();
                let mkdir_res = rclone::rpc_async("operations/mkdir".to_string(), mkdir_param).await?;
                if mkdir_res.status != 200 {
                    return Err(format!("Lỗi khi tạo thư mục đích: {}", mkdir_res.output));
                }
                let _ = tx.send(AppEvent::CopyProgress {
                    src: src.clone(),
                    dest: dest.clone(),
                    pct: 100.0,
                    job_id: None,
                });
                return Ok(());
            }
        };

        if list_arr.is_empty() {
            let mkdir_param = serde_json::json!({
                "fs": dest,
                "remote": "",
            }).to_string();
            let mkdir_res = rclone::rpc_async("operations/mkdir".to_string(), mkdir_param).await?;
            if mkdir_res.status != 200 {
                return Err(format!("Lỗi khi tạo thư mục đích: {}", mkdir_res.output));
            }
            let _ = tx.send(AppEvent::CopyProgress {
                src: src.clone(),
                dest: dest.clone(),
                pct: 100.0,
                job_id: None,
            });
            return Ok(());
        }

        let mut files = Vec::new();
        let mut dirs = Vec::new();

        for item in list_arr {
            let path = item.get("Path").and_then(|p| p.as_str()).unwrap_or("").to_string();
            if path.is_empty() {
                continue;
            }
            let is_item_dir = item.get("IsDir").and_then(|d| d.as_bool()).unwrap_or(false);
            if is_item_dir {
                dirs.push(path);
            } else {
                files.push(path);
            }
        }

        let mut empty_dirs = Vec::new();
        for dir in &dirs {
            let prefix = format!("{}/", dir);
            let has_files = files.iter().any(|f| f.starts_with(&prefix));
            let has_subdirs = dirs.iter().any(|d| d != dir && d.starts_with(&prefix));
            if !has_files && !has_subdirs {
                empty_dirs.push(dir.clone());
            }
        }

        let total_files = files.len();
        let mut success_count = 0;
        let mut error_messages = Vec::new();

        for (idx, file_path) in files.iter().enumerate() {
            let pct = (idx as f64) / (total_files as f64) * 100.0;
            let _ = tx.send(AppEvent::CopyProgress {
                src: src.clone(),
                dest: dest.clone(),
                pct,
                job_id: None,
            });

            let (parent_path, file_name) = if let Some(last_slash_idx) = file_path.rfind('/') {
                (&file_path[..last_slash_idx], &file_path[last_slash_idx+1..])
            } else {
                ("", file_path.as_str())
            };

            let src_fs = if parent_path.is_empty() {
                src.clone()
            } else {
                join_fs_path(&src, parent_path)
            };

            let dst_fs = if parent_path.is_empty() {
                dest.clone()
            } else {
                join_fs_path(&dest, parent_path)
            };

            let mut copy_param = serde_json::json!({
                "srcFs": src_fs,
                "srcRemote": file_name,
                "dstFs": dst_fs,
                "dstRemote": file_name,
            });
            if use_checksum {
                if let Some(obj) = copy_param.as_object_mut() {
                    obj.insert("_config".to_string(), serde_json::json!({ "checksum": true }));
                }
            }
            let copy_param_str = copy_param.to_string();

            let copy_res = rclone::rpc_async("operations/copyfile".to_string(), copy_param_str).await;
            match copy_res {
                Ok(rpc_res) => {
                    if rpc_res.status == 200 {
                        success_count += 1;
                    } else {
                        let err_msg = rpc_res.output.to_lowercase();
                        if err_msg.contains("restrictedlink") 
                            || err_msg.contains("download") 
                            || err_msg.contains("forbidden") 
                            || err_msg.contains("only the owner")
                        {
                            // Skip
                        } else {
                            error_messages.push(format!("File {}: {}", file_path, rpc_res.output));
                        }
                    }
                }
                Err(e) => {
                    error_messages.push(format!("File {}: {}", file_path, e));
                }
            }
        }

        for empty_dir in &empty_dirs {
            let mkdir_param = serde_json::json!({
                "fs": dest.clone(),
                "remote": empty_dir,
            }).to_string();
            let _ = rclone::rpc_async("operations/mkdir".to_string(), mkdir_param).await;
        }

        let _ = tx.send(AppEvent::CopyProgress {
            src: src.clone(),
            dest: dest.clone(),
            pct: 100.0,
            job_id: None,
        });

        if success_count == 0 && total_files > 0 && !error_messages.is_empty() {
            Err(format!("Không sao chép được file nào. Các lỗi gặp phải:\n{}", error_messages.join("\n")))
        } else {
            Ok(())
        }
    }
}
