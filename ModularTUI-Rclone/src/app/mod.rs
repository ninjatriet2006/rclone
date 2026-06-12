use crate::functions::*;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use std::io;
use std::time::Duration;
use crossterm::{
    event::{self, Event, KeyEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    style::{Color, Modifier, Style},
};

pub mod connection_manager;
pub mod file_explorer;
pub mod job_monitor;
pub mod language_settings;
pub mod main_menu;
pub mod operations;
pub mod profile_manager;
pub mod services_utilities;

pub mod start_async_checker_and_transfer;

lazy_static::lazy_static! {
    pub(crate) static ref RUNNING_SIZE_CHECKS: std::sync::Mutex<std::collections::HashSet<String>> = std::sync::Mutex::new(std::collections::HashSet::new());
    pub(crate) static ref ACTIVE_OPS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    pub(crate) static ref PRE_OPS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}

use crate::functions::widgets::structs::{ActiveOperation, PreOperation};

pub fn save_active_operation(op: &ActiveOperation) {
    let _lock = ACTIVE_OPS_LOCK.lock().unwrap();
    let _ = crate::functions::save_active_operation(op);
}

pub fn complete_item_in_active_operation(id: &str, item_name: &str) {
    let _lock = ACTIVE_OPS_LOCK.lock().unwrap();
    let _ = crate::functions::complete_item_in_active_operation(id, item_name);
}

pub fn complete_items_in_active_operation(id: &str, item_names: &[String]) {
    let _lock = ACTIVE_OPS_LOCK.lock().unwrap();
    let _ = crate::functions::complete_items_in_active_operation(id, item_names);
}

pub fn update_task_status_in_active_operation(id: &str, item_name: &str, status: crate::functions::TaskStatus, error: Option<String>) {
    let _lock = ACTIVE_OPS_LOCK.lock().unwrap();
    let _ = crate::functions::update_task_status_in_active_operation(id, item_name, status, error);
}

pub fn update_tasks_status_in_active_operation(id: &str, item_names: &[String], status: crate::functions::TaskStatus, error: Option<String>) {
    let _lock = ACTIVE_OPS_LOCK.lock().unwrap();
    let _ = crate::functions::update_tasks_status_in_active_operation(id, item_names, status, error);
}

pub fn append_tasks_to_active_operation(id: &str, new_tasks: &[crate::functions::FileTask]) {
    let _lock = ACTIVE_OPS_LOCK.lock().unwrap();
    let _ = crate::functions::append_tasks_to_active_operation(id, new_tasks);
}

pub fn prepare_active_operation_for_resume(id: &str) {
    let _lock = ACTIVE_OPS_LOCK.lock().unwrap();
    let _ = crate::functions::prepare_active_operation_for_resume(id);
}

pub fn remove_active_operation(id: &str) {
    let _lock = ACTIVE_OPS_LOCK.lock().unwrap();
    let _ = crate::functions::remove_active_operation(id);
}

pub fn update_active_operation_threads(id: &str, transfers: u64, checkers: u64) {
    let _lock = ACTIVE_OPS_LOCK.lock().unwrap();
    let _ = crate::functions::update_active_operation_threads(id, transfers, checkers);
}

pub fn load_active_operations() -> Vec<ActiveOperation> {
    let _lock = ACTIVE_OPS_LOCK.lock().unwrap();
    crate::functions::load_active_operations().unwrap_or_default()
}

pub fn clear_active_operations() {
    let _lock = ACTIVE_OPS_LOCK.lock().unwrap();
    let _ = crate::functions::clear_active_operations();
}

fn load_pre_operations_unlocked() -> Vec<PreOperation> {
    let path = crate::functions::AppConfig::config_dir().join("pre_ops.json");
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(ops) = serde_json::from_str::<Vec<PreOperation>>(&content) {
                return ops;
            }
        }
    }
    Vec::new()
}

pub fn load_pre_operations() -> Vec<PreOperation> {
    let _lock = PRE_OPS_LOCK.lock().unwrap();
    load_pre_operations_unlocked()
}

pub fn save_pre_operation(op: &PreOperation) {
    let _lock = PRE_OPS_LOCK.lock().unwrap();
    let path = crate::functions::AppConfig::config_dir().join("pre_ops.json");
    let mut ops = load_pre_operations_unlocked();
    if let Some(pos) = ops.iter().position(|o| o.id == op.id) {
        ops[pos] = op.clone();
    } else {
        ops.push(op.clone());
    }
    if let Ok(serialized) = serde_json::to_string_pretty(&ops) {
        let _ = std::fs::write(path, serialized);
    }
}

pub fn remove_pre_operation(id: &str) {
    let _lock = PRE_OPS_LOCK.lock().unwrap();
    let path = crate::functions::AppConfig::config_dir().join("pre_ops.json");
    let mut ops = load_pre_operations_unlocked();
    ops.retain(|o| o.id != id);
    if let Ok(serialized) = serde_json::to_string_pretty(&ops) {
        let _ = std::fs::write(path, serialized);
    }
}

pub fn clear_pre_operations() {
    let _lock = PRE_OPS_LOCK.lock().unwrap();
    let path = crate::functions::AppConfig::config_dir().join("pre_ops.json");
    let _ = std::fs::remove_file(path);
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
    DependencyManager,
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum AppEvent {
    Input(KeyEvent),
    Tick,
    ExplorerListResult {
        pane: ActivePane,
        result: Result<Vec<FileItem>, String>,
    },
    WizardGuiListResult {
        result: Result<Vec<FileItem>, String>,
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
        active: Vec<TransferJob>,
        active_transfers: usize,
        active_checks: usize,
        bottleneck_reason: String,
    },
    OAuthFinished {
        result: Result<(), String>,
    },
    OAuthUrlReceived {
        url: String,
    },
    ActiveServicesLoaded(Vec<ActiveService>),
    RemoteStatusUpdate {
        remote: String,
        status: String,
    },
    ExplorerOperationFinished {
        pane: ActivePane,
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
        result: Result<Vec<FileItem>, String>,
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
    },
    PermissionCheckPassed {
        src: String,
        dest: String,
        is_dir: bool,
        use_checksum: bool,
    },
    MultiPermissionErrorDetected {
        items: Vec<ClipboardItem>,
        dest_remote: String,
        dest_path: String,
        restricted_files: Vec<String>,
        use_checksum: bool,
    },
    MultiPermissionCheckPassed {
        items: Vec<ClipboardItem>,
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
        result: Result<(Vec<String>, TreeNode), String>,
        folders: Vec<FileItem>,
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
    pub queue: Vec<String>,
    pub active_tasks: usize,
    pub files: Vec<String>,
    pub restricted_files: Vec<String>,
}

pub(crate) struct MultiScanState {
    pub queue: Vec<(String, String)>,
    pub active_tasks: usize,
    pub files_count: usize,
    pub restricted: Vec<String>,
}

pub struct App {
    pub screen: Screen,
    pub config: AppConfig,
    pub should_exit: bool,
    pub delete_confirm: Option<DeleteTarget>,

    // States
    pub menu_state: MenuState,
    pub connection_state: ConnectionState,
    pub explorer_state: ExplorerState,
    pub monitor_state: MonitorState,
    pub profile_state: ProfileState,
    pub services_state: ServicesState,

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

// Re-export key handler helper to keep key handlers imports happy
pub use crate::functions::keys::handle_input_key;

fn migrate_old_json_data() {
    let json_path = crate::functions::AppConfig::config_dir().join("active_ops.json");
    if json_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&json_path) {
            if let Ok(ops) = serde_json::from_str::<Vec<ActiveOperation>>(&content) {
                for op in ops {
                    let _ = crate::functions::save_active_operation(&op);
                }
            }
        }
        let _ = std::fs::remove_file(json_path);
    }
}

impl App {
    pub fn new() -> Self {
        // Khởi tạo database SQLite và thực hiện migration
        let _ = crate::functions::init_db();
        migrate_old_json_data();

        // Khởi tạo và nạp ngôn ngữ
        crate::functions::init_languages();
        let config = AppConfig::load();
        crate::functions::load_translation(&config.active_language);
        let available_languages = crate::functions::get_available_languages();
        let selected_lang_idx = available_languages
            .iter()
            .position(|l| l == &config.active_language)
            .unwrap_or(0);

        let mut features_cache = std::collections::HashMap::new();
        let cache_path = PathBuf::from(crate::functions::app_config::TuiCustomConfig::load().features_cache_file_path);
        if let Ok(content) = std::fs::read_to_string(&cache_path) {
            if let Ok(parsed) = serde_json::from_str::<std::collections::HashMap<String, serde_json::Value>>(&content) {
                features_cache = parsed;
            }
        }

        let home_dir = crate::functions::get_home_dir();
        let filen_cli_installed = std::path::Path::new(&home_dir).join(".filen-cli/bin/filen").exists();
        let fuse_installed = crate::functions::check_fuse_dependency();

        let mut monitor_state = MonitorState::new();
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
            monitor_state.failed_files.push(crate::functions::widgets::structs::FailedCopyItem {
                src: op.src.clone(),
                dest: op.dest.clone(),
                error: "Tác vụ bị gián đoạn do crash / tắt đột ngột (Nhấn R để thử lại)".to_string(),
                time: now_str,
                is_copy: op.is_copy,
            });
        }
        clear_active_operations();

        let saved_pre_ops = load_pre_operations();
        for op in saved_pre_ops {
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
            monitor_state.failed_files.push(crate::functions::widgets::structs::FailedCopyItem {
                src: op.src.clone(),
                dest: op.dest.clone(),
                error: "Tác vụ bị gián đoạn khi đang quét quyền/checkhash do crash / tắt đột ngột (Nhấn R để thử lại)".to_string(),
                time: now_str,
                is_copy: op.action_type == "copy",
            });
        }
        clear_pre_operations();

        App {
            screen: Screen::MainMenu,
            config,
            should_exit: false,
            delete_confirm: None,
            menu_state: MenuState::new(),
            connection_state: ConnectionState::new(),
            explorer_state: ExplorerState::new(),
            monitor_state,
            profile_state: ProfileState::new(),
            services_state: ServicesState::new(),
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

    pub(crate) async fn handle_tick_event(&mut self, tx: tokio::sync::mpsc::UnboundedSender<AppEvent>) {
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
                let res = crate::functions::rpc_async("core/stats".to_string(), "{}".to_string()).await;
                
                // Lấy danh sách Job ID
                let list_res = crate::functions::rpc_async("job/list".to_string(), "{}".to_string()).await;
                
                let mut active: Vec<TransferJob> = Vec::new();
                let mut speed = 0.0;
                let mut transferred = 0;
                let mut total = 0;
                let mut active_transfers = 0;
                let mut active_checks = 0;
                let mut upload_speed = 0.0;
                let mut download_speed = 0.0;

                let mut ids_to_check = Vec::new();
                if let Ok(list_rpc) = list_res {
                    if let Ok(list_val) = serde_json::from_str::<Value>(&list_rpc.output) {
                        let ids = if let Some(r_ids) = list_val.get("runningIds").and_then(|j| j.as_array()) {
                            r_ids.clone()
                        } else if let Some(job_ids) = list_val.get("jobids").and_then(|j| j.as_array()) {
                            job_ids.clone()
                        } else {
                            Vec::new()
                        };
                        ids_to_check = ids;
                    }
                }

                let has_running_jobs = !ids_to_check.is_empty();

                if !has_running_jobs {
                    // Nếu không có job chạy ngầm, sử dụng stats toàn cục của rclone
                    if let Ok(rpc_res) = res {
                        if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                            speed = val.get("speed").and_then(|s| s.as_f64()).unwrap_or(0.0);
                            transferred = val.get("bytes").and_then(|b| b.as_u64()).unwrap_or(0);
                            total = val.get("totalBytes").and_then(|t| t.as_u64()).unwrap_or(0);

                            if let Some(transfers) = val.get("transferring").and_then(|t| t.as_array()) {
                                active_transfers = transfers.len();
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

                                    active.push(TransferJob {
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
                                        files: Vec::new(),
                                    });
                                }
                            }
                            if let Some(checking) = val.get("checking").and_then(|c| c.as_array()) {
                                active_checks = checking.len();
                            }
                        }
                    }
                } else {
                    // Nếu có job chạy ngầm, chúng ta sẽ chỉ lấy thông tin và tính tổng từ các job này
                    for id_val in ids_to_check {
                        if let Some(id) = id_val.as_i64() {
                            // Lấy thông tin chi tiết từng Job
                            let status_res = crate::functions::rpc_async(
                                "job/status".to_string(),
                                serde_json::json!({ "jobid": id }).to_string(),
                            )
                            .await;

                            if let Ok(s_rpc) = status_res {
                                if let Ok(s_val) = serde_json::from_str::<Value>(&s_rpc.output) {
                                    let finished = s_val.get("finished").and_then(|f| f.as_bool()).unwrap_or(false);
                                    if !finished {
                                        let description = crate::functions::get_job_description(id)
                                            .unwrap_or_else(|| format!("Tác vụ #{}", id));
                                        
                                        // Lấy duration và start_time của job
                                        let duration = s_val.get("duration").and_then(|d| d.as_f64()).unwrap_or(0.0);
                                        let start_time = s_val.get("startTime").and_then(|s| s.as_str()).unwrap_or("").to_string();
                                        let cleaned_start = start_time.chars().take(19).collect::<String>().replace("T", " ");

                                        // Dự đoán hướng của Job
                                        let mut direction = crate::functions::get_job_direction(id);
                                        if direction.is_none() {
                                            let desc_lower = description.to_lowercase();
                                            if desc_lower.contains("sao chép") || desc_lower.contains("di chuyển") || desc_lower.contains("copy") || desc_lower.contains("move") {
                                                if let Some(arrow_idx) = description.find("->") {
                                                    let src_part = &description[..arrow_idx];
                                                    let dest_part = &description[arrow_idx + 2..];
                                                    let src_remote = src_part.contains(':');
                                                    let dest_remote = dest_part.contains(':');
                                                    if src_remote && !dest_remote {
                                                        direction = Some(crate::functions::JobDirection::Download);
                                                    } else if !src_remote && dest_remote {
                                                        direction = Some(crate::functions::JobDirection::Upload);
                                                    } else if src_remote && dest_remote {
                                                        direction = Some(crate::functions::JobDirection::RemoteToRemote);
                                                    } else {
                                                        direction = Some(crate::functions::JobDirection::Local);
                                                    }
                                                }
                                            }
                                        }

                                        // Lấy chi tiết dung lượng truyền tải từ group core/stats
                                        let group = format!("job/{}", id);
                                        let mut bytes = 0;
                                        let mut speed_job = 0;
                                        let mut percentage = 0;
                                        let mut total_bytes = 0;

                                        if let Ok(job_stats_res) = crate::functions::rpc_async(
                                            "core/stats".to_string(),
                                            serde_json::json!({ "group": group }).to_string(),
                                        )
                                        .await {
                                            if let Ok(js_val) = serde_json::from_str::<Value>(&job_stats_res.output) {
                                                bytes = js_val.get("bytes").and_then(|b| b.as_u64()).unwrap_or(0);
                                                speed_job = js_val.get("speed").and_then(|s| s.as_u64()).unwrap_or(0);
                                                total_bytes = js_val.get("totalBytes").and_then(|t| t.as_u64()).unwrap_or(0);
                                                if total_bytes > 0 {
                                                    percentage = ((bytes as f64 / total_bytes as f64) * 100.0).min(99.0) as u16;
                                                }

                                                if let Some(transfers) = js_val.get("transferring").and_then(|t| t.as_array()) {
                                                    active_transfers += transfers.len();
                                                }
                                                if let Some(checking) = js_val.get("checking").and_then(|c| c.as_array()) {
                                                    active_checks += checking.len();
                                                }

                                                if let Some(dir) = direction {
                                                    match dir {
                                                        crate::functions::JobDirection::Upload => upload_speed += speed_job as f64,
                                                        crate::functions::JobDirection::Download => download_speed += speed_job as f64,
                                                        crate::functions::JobDirection::RemoteToRemote => {
                                                            upload_speed += speed_job as f64;
                                                            download_speed += speed_job as f64;
                                                        }
                                                        crate::functions::JobDirection::Local => {}
                                                    }
                                                }
                                            }
                                        }

                                        speed += speed_job as f64;
                                        transferred += bytes;
                                        total += total_bytes;

                                        active.push(TransferJob {
                                            name: description,
                                            size: total_bytes,
                                            bytes,
                                            speed: speed_job,
                                            percentage,
                                            eta: -1,
                                            job_id: Some(id),
                                            start_time: cleaned_start,
                                            duration,
                                            group,
                                            description: String::new(),
                                            files: Vec::new(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }

                // Cộng dồn active_transfers và active_checks cho các active operations nội bộ
                let active_ops_local = load_active_operations();
                let pre_ops_local = load_pre_operations();
                for op in &active_ops_local {
                    let is_scanning = pre_ops_local.iter().any(|po| po.id == op.id && po.status == "scanning");
                    let opt_checkers = op.checkers.unwrap_or(4) as usize;
                    if is_scanning {
                        active_checks += opt_checkers;
                    } else if let Some(ref tasks) = op.tasks {
                        let transferring_count = tasks.iter().filter(|t| t.status == crate::functions::TaskStatus::Transferring).count();
                        active_transfers += transferring_count;
                        if transferring_count > 0 {
                            active_checks += opt_checkers;
                        }
                    }
                }

                let mut bottleneck_reason = "Tốc độ tối ưu / Bình thường (Optimal)".to_string();

                if speed > 0.0 {
                    let config = crate::functions::app_config::AppConfig::load();
                    let max_bw = config.max_bandwidth_bytes_per_sec as f64;
                    
                    if max_bw > 0.0 && speed >= max_bw * 0.90 {
                        bottleneck_reason = "Đạt giới hạn băng thông tối đa thiết lập (Bandwidth Limit)".to_string();
                    } else {
                        let avg_speed_per_transfer = if active_transfers > 0 {
                            speed / (active_transfers as f64)
                        } else {
                            0.0
                        };

                        if active_transfers >= 16 && avg_speed_per_transfer < 30_000.0 && speed < 1_500_000.0 {
                            bottleneck_reason = "Bị giới hạn API Cloud (Throttling / Rate Limit - Mở quá nhiều luồng)".to_string();
                        } else if active_transfers > 0 && active_transfers <= 3 && speed < 2_000_000.0 {
                            bottleneck_reason = "Nghẽn do thiếu luồng cho nhiều file nhỏ (Low Threads)".to_string();
                        }
                    }
                } else if has_running_jobs {
                    bottleneck_reason = "Đang kết nối hoặc chờ phản hồi từ Cloud (Connecting / Latency)".to_string();
                } else {
                    bottleneck_reason = "Không có truyền tải dữ liệu (Idle)".to_string();
                }

                let _ = tx_clone.send(AppEvent::JobStatsUpdate {
                    speed,
                    upload_speed,
                    download_speed,
                    transferred,
                    total,
                    active,
                    active_transfers,
                    active_checks,
                    bottleneck_reason,
                });
            });
        }
    }

    pub fn refresh_tui_selector_list(&mut self, tx: tokio::sync::mpsc::UnboundedSender<AppEvent>) {
        crate::functions::custom::refresh_tui_selector_list(self, tx);
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
        let _ = crate::functions::rpc(
            "config/setpath",
            &serde_json::json!({"path": active_profile}).to_string(),
        );

        // Tải các tiến trình chạy ngầm
        self.load_active_services_from_file();

        // 3. Khởi chạy luồng kiểm tra trạng thái các remote tuần hoàn/chạy ngầm
        let (status_tx, mut status_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        self.status_trigger_tx = Some(status_tx);

        let tx_status = tx.clone();
        tokio::spawn(async move {
            // Chờ 3 giây lúc khởi động để TUI vẽ giao diện xong trước, tránh nghẽn khởi động do tranh chấp lock RCLONE_ENGINE_LOCK
            tokio::time::sleep(Duration::from_secs(3)).await;
            loop {
                // Fetch list of remotes
                let res =
                    crate::functions::rpc_async("config/listremotes".to_string(), "{}".to_string()).await;
                if let Ok(rpc_res) = res {
                    if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                        if let Some(arr) = val.get("remotes").and_then(|r| r.as_array()) {
                            let remotes: Vec<String> = arr
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect();

                            // Xây dựng dependency map ngay trong tác vụ ngầm từ config/dump
                            let mut local_dependencies = HashMap::new();
                            let dump_res = crate::functions::rpc_async("config/dump".to_string(), "{}".to_string()).await;
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
                                                serde_json::json!({ "fs": format!("{}:", remote_clone) })
                                                    .to_string();

                                            // 1. Thử gọi operations/about trước để lấy dung lượng thực tế
                                            let about_future = crate::functions::rpc_async(
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
                                                                    crate::functions::ui_helpers::format_size(used_bytes),
                                                                    crate::functions::ui_helpers::format_size(total_bytes)
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
                                                            let fsinfo_future = crate::functions::rpc_async(
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

                                                                let size_future = crate::functions::rpc_async(
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
                                                                                            crate::functions::ui_helpers::format_size(used_bytes)
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
                    Screen::MainMenu => crate::app::main_menu::draw_main_menu(&self.menu_state, f, main_layout[1]),
                    Screen::ConnectionManager => {
                        crate::app::connection_manager::draw_connection_manager(&self.connection_state, f, main_layout[1], &self.remote_types)
                    }
                    Screen::FileExplorer => {
                        crate::app::file_explorer::draw_file_explorer(&mut self.explorer_state, f, main_layout[1])
                    }
                    Screen::JobMonitor => crate::app::job_monitor::draw_job_monitor(&mut self.monitor_state, f, main_layout[1]),
                    Screen::ConfigProfileManager => crate::app::profile_manager::draw_profile_manager(
                        &self.profile_state,
                        f,
                        main_layout[1],
                        &active_profile_name,
                    ),
                    Screen::ServicesAndMounts => {
                        crate::app::services_utilities::draw_services_utilities(&self.services_state, f, main_layout[1])
                    }
                    Screen::LanguageSelect => crate::app::language_settings::draw_language_settings(
                        &self.available_languages,
                        self.selected_lang_idx,
                        &self.config.active_language,
                        f,
                        main_layout[1],
                    ),
                    Screen::DependencyManager => {
                        crate::functions::widgets::draw_dependency_manager(self, f, main_layout[1])
                    }
                }

                if let Some(ref target) = self.delete_confirm {
                    let (title, message) = match target {
                        DeleteTarget::Connection(name) => (
                            crate::functions::translate("confirm_delete_remote_title").to_string(),
                            crate::functions::translate("confirm_delete_remote").replace("{}", name),
                        ),
                        DeleteTarget::FileExplorer(name) => (
                            crate::functions::translate("confirm_delete_file_title").to_string(),
                            crate::functions::translate("confirm_delete_file").replace("{}", name),
                        ),
                        DeleteTarget::FileExplorerMultiple(names) => (
                            crate::functions::translate("confirm_delete_multiple_title").to_string(),
                            crate::functions::translate("confirm_delete_multiple").replace("{}", &names.len().to_string()),
                        ),
                        DeleteTarget::Service(idx) => {
                            let service_details = if *idx < self.services_state.active_services.len() {
                                &self.services_state.active_services[*idx].details
                            } else {
                                ""
                            };
                            (
                                crate::functions::translate("confirm_delete_service_title").to_string(),
                                crate::functions::translate("confirm_delete_service").replace("{}", service_details),
                            )
                        }
                        DeleteTarget::SystemdService(idx) => {
                            let name = if *idx < self.services_state.systemd_services.len() {
                                &self.services_state.systemd_services[*idx].name
                            } else {
                                ""
                            };
                            (
                                crate::functions::translate("confirm_delete_systemd_title").to_string(),
                                crate::functions::translate("confirm_delete_systemd").replace("{}", name),
                            )
                        }
                    };
                    crate::functions::draw_popup(f, &title, &message, 60, 30);
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
                            ServicesWizardState::GuiSelectPath {
                                ref mut items,
                                ref mut loading,
                                ref mut error_msg,
                                ..
                            } | ServicesWizardState::GuiSelectLocalPath {
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
                        if let ExplorerPopup::CopyProgress { .. } =
                            self.explorer_state.popup
                        {
                            if pct >= 100.0 {
                                self.explorer_state.popup = ExplorerPopup::None;
                                self.refresh_explorer_pane(
                                    ActivePane::Left,
                                    tx.clone(),
                                )
                                .await;
                                self.refresh_explorer_pane(
                                    ActivePane::Right,
                                    tx.clone(),
                                )
                                .await;
                            } else {
                                self.explorer_state.popup =
                                    ExplorerPopup::CopyProgress { src, dest, pct, job_id };
                            }
                        }
                    }
                    AppEvent::MoveProgress { src, dest, pct, job_id } => {
                        if let ExplorerPopup::MoveProgress { .. } =
                            self.explorer_state.popup
                        {
                            if pct >= 100.0 {
                                self.explorer_state.popup = ExplorerPopup::None;
                                self.refresh_explorer_pane(
                                    ActivePane::Left,
                                    tx.clone(),
                                )
                                .await;
                                self.refresh_explorer_pane(
                                    ActivePane::Right,
                                    tx.clone(),
                                )
                                .await;
                            } else {
                                self.explorer_state.popup =
                                    ExplorerPopup::MoveProgress { src, dest, pct, job_id };
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
                        bottleneck_reason,
                    } => {
                        self.monitor_state.speed = speed;
                        self.monitor_state.upload_speed = upload_speed;
                        self.monitor_state.download_speed = download_speed;
                        self.monitor_state.bytes_transferred = transferred;
                        self.monitor_state.total_bytes = total;
                        self.monitor_state.active_jobs = active;
                        self.monitor_state.active_transfers = active_transfers;
                        self.monitor_state.active_checks = active_checks;
                        self.monitor_state.bottleneck_reason = bottleneck_reason;
                        if self.monitor_state.selected_job_idx >= self.monitor_state.active_jobs.len() {
                            self.monitor_state.selected_job_idx = 0;
                        }
                        self.stats_scan_in_progress = false;
                    }
                    AppEvent::OAuthFinished { result } => {
                        if let WizardState::SimpleOAuthLoop {
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
                                        WizardState::None;
                                }
                            }
                            self.load_remotes(tx.clone()).await;
                        }
                    }
                    AppEvent::OAuthUrlReceived { url } => {
                        if let WizardState::SimpleOAuthLoop {
                            provider,
                            remote_name,
                            selected_providers,
                            ..
                        } = &self.connection_state.wizard
                        {
                            self.connection_state.wizard = WizardState::SimpleOAuthLoop {
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
                            ExplorerPopup::CopyProgress { .. }
                                | ExplorerPopup::MoveProgress { .. }
                                | ExplorerPopup::SpecialActionMessage { .. }
                        ) {
                            self.explorer_state.popup = ExplorerPopup::None;
                        }
                        match result {
                            Ok(_) => {
                                self.refresh_explorer_pane(ActivePane::Left, tx.clone()).await;
                                self.refresh_explorer_pane(ActivePane::Right, tx.clone()).await;
                                self.explorer_state.notification = Some((
                                    "THÀNH CÔNG".to_string(),
                                    format!("Tác vụ '{}' hoàn tất thành công!", op_name),
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
                                self.explorer_state.popup = ExplorerPopup::ViewFile {
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
                        if let ExplorerPopup::TuiExplorerSelector {
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
                        if let ExplorerPopup::CryptdecodeForm {
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
                            self.explorer_state.popup = ExplorerPopup::CryptdecodeForm {
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
                                self.explorer_state.popup = ExplorerPopup::SpecialActionMessage {
                                    title: "Kết quả".to_string(),
                                    message: msg,
                                };
                            }
                            Err(e) => {
                                self.explorer_state.notification = Some(("LỖI GIẢI MÃ CRYPT".to_string(), e));
                            }
                        }
                    }
                    AppEvent::MergeSimilarScanProgress { folders_count, scanned_count } => {
                        if let ExplorerPopup::MergeSimilarScanning { .. } = self.explorer_state.popup {
                            self.explorer_state.popup = ExplorerPopup::MergeSimilarScanning {
                                folders_count,
                                scanned_count,
                            };
                        }
                    }
                    AppEvent::MergeSimilarScanFinished { result, folders, destination_idx } => {
                        if let ExplorerPopup::MergeSimilarScanning { .. } = self.explorer_state.popup {
                            match result {
                                Ok((summary_report, tree_root)) => {
                                    let selected_rel_path = tree_root.rel_path.clone(); // root relative path is ""
                                    let expanded_paths = std::collections::HashSet::new(); // empty means root collapsed
                                    
                                    self.explorer_state.popup = ExplorerPopup::MergeSimilarPreview {
                                        summary_report,
                                        tree_root,
                                        expanded_paths,
                                        selected_rel_path,
                                        scroll_offset: 0,
                                        folders,
                                        destination_idx,
                                    };
                                }
                                Err(e) => {
                                    self.explorer_state.popup = ExplorerPopup::None;
                                    self.explorer_state.notification = Some(("LỖI QUÉT THƯ MỤC".to_string(), e));
                                }
                            }
                        }
                    }
                    AppEvent::PermissionScanProgress { src, dest, is_dir, scanned_count, total_files, restricted_count } => {
                        if let ExplorerPopup::PermissionScanning { .. } = self.explorer_state.popup {
                            self.explorer_state.popup = ExplorerPopup::PermissionScanning {
                                src,
                                dest,
                                is_dir,
                                scanned_count,
                                total_files,
                                restricted_count,
                            };
                        }
                    }
                    AppEvent::PermissionErrorDetected { src, dest, is_dir, restricted_files, use_checksum } => {
                        if let ExplorerPopup::PermissionScanning { .. } = self.explorer_state.popup {
                            let mut options = Vec::new();
                            let mut actions = Vec::new();

                            options.push(translate("exp_permission_option_cancel"));
                            actions.push(FallbackAction::PermissionCancel);

                            options.push(translate("exp_permission_option_as_much"));
                            actions.push(FallbackAction::PermissionCopyAsMuchAsPossible {
                                src: src.clone(),
                                dest: dest.clone(),
                                is_dir,
                                restricted_files: restricted_files.clone(),
                                use_checksum,
                            });

                            options.push(translate("exp_permission_option_restricted"));
                            actions.push(FallbackAction::PermissionRestrictedCopy {
                                src: src.clone(),
                                dest: dest.clone(),
                                is_dir,
                                restricted_files: restricted_files.clone(),
                                use_checksum,
                            });

                            self.explorer_state.popup = ExplorerPopup::ConfirmFallback {
                                title: translate("exp_permission_error_title").to_string(),
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
                    AppEvent::PermissionCheckPassed { src, dest, is_dir: _, use_checksum } => {
                        if let ExplorerPopup::PermissionScanning { .. } = self.explorer_state.popup {
                            self.explorer_state.popup = ExplorerPopup::CopyProgress {
                                src: src.clone(),
                                dest: dest.clone(),
                                pct: 0.0,
                                job_id: None,
                            };
                            let tx_copy = tx.clone();
                            let src_clone = src.clone();
                            let dest_clone = dest.clone();
                            tokio::spawn(async move {
                                let mut param = serde_json::json!({
                                    "srcFs": src_clone,
                                    "dstFs": dest_clone,
                                });
                                if use_checksum {
                                    if let Some(obj) = param.as_object_mut() {
                                        obj.insert("_config".to_string(), serde_json::json!({ "checksum": true }));
                                    }
                                }
                                let res = crate::functions::run_rpc_job_async_with_progress(
                                    "sync/copy".to_string(),
                                    param,
                                    Some((src_clone, dest_clone, true)),
                                    Some(tx_copy.clone()), None).await;
                                let _ = tx_copy.send(AppEvent::ExplorerOperationFinished {
                                    pane: ActivePane::Left,
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
                        if let ExplorerPopup::PermissionScanning { .. } = self.explorer_state.popup {
                            let mut options = Vec::new();
                            let mut actions = Vec::new();

                            options.push(translate("exp_permission_option_cancel"));
                            actions.push(FallbackAction::PermissionCancel);

                            options.push(translate("exp_permission_option_as_much"));
                            actions.push(FallbackAction::MultiPermissionCopyAsMuchAsPossible {
                                items: items.clone(),
                                dest_remote: dest_remote.clone(),
                                dest_path: dest_path.clone(),
                                restricted_files: restricted_files.clone(),
                                use_checksum,
                            });

                            options.push(translate("exp_permission_option_restricted"));
                            actions.push(FallbackAction::MultiPermissionRestrictedCopy {
                                items: items.clone(),
                                dest_remote: dest_remote.clone(),
                                dest_path: dest_path.clone(),
                                restricted_files: restricted_files.clone(),
                                use_checksum,
                            });

                            self.explorer_state.popup = ExplorerPopup::ConfirmFallback {
                                title: translate("exp_permission_error_title").to_string(),
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
                        if let ExplorerPopup::PermissionScanning { .. } = self.explorer_state.popup {
                            let dest_remote_clone = dest_remote.clone();
                            let dest_path_clone = dest_path.clone();
                            let items_clone = items.clone();
                            let tx_op = tx.clone();
                            let pane_type = self.explorer_state.active_pane.clone();

                            self.explorer_state.popup = ExplorerPopup::CopyProgress {
                                src: format!("({} mục)", items_clone.len()),
                                dest: dest_full.clone(),
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
                                        let clean_remote = clip_item.remote.trim_end_matches(':');
                                        let clean_path = if clip_item.path.starts_with('/') {
                                            clip_item.path.clone()
                                        } else {
                                            format!("/{}", clip_item.path)
                                        };
                                        let clean_path = if clean_path.ends_with('/') {
                                            format!("{}{}", clean_path, clip_item.name)
                                        } else {
                                            format!("{}/{}", clean_path, clip_item.name)
                                        };
                                        format!("{}:{}", clean_remote, clean_path)
                                    };
                                    let dest = if dest_remote_clone.is_empty() {
                                        PathBuf::from(&dest_path_clone)
                                            .join(&clip_item.name)
                                            .to_string_lossy()
                                            .to_string()
                                    } else {
                                        format!("{}:{}/{}", dest_remote_clone.trim_end_matches(':'), dest_path_clone.trim_start_matches('/'), clip_item.name)
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
                                    let mut param = if clip_item.is_dir {
                                        serde_json::json!({ "srcFs": src, "dstFs": dest })
                                    } else {
                                        serde_json::json!({
                                            "srcFs": src.rsplit_once('/').map(|(p,_)| p).unwrap_or(&src),
                                            "srcRemote": clip_item.name,
                                            "dstFs": dest.rsplit_once('/').map(|(p,_)| p).unwrap_or(&dest),
                                            "dstRemote": clip_item.name
                                        })
                                    };

                                    if use_checksum {
                                        if let Some(obj) = param.as_object_mut() {
                                            obj.insert("_config".to_string(), serde_json::json!({ "checksum": true }));
                                        }
                                    }

                                    let res = crate::functions::run_rpc_job_async(method.to_string(), param).await;
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
                        } else {
                            let src_full = format!("({} mục)", items.len());
                            if let Some(job) = self.monitor_state.pending_jobs.iter_mut().find(|j| j.src == src_full && j.dest == dest_full) {
                                job.status = "Scanned (No Restrictions)".to_string();
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

        Ok(())
    }

    pub(crate) async fn advance_connection_wizard(
        &mut self,
        mut remaining_providers: Vec<String>,
        _tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        if remaining_providers.is_empty() {
            self.connection_state.wizard = WizardState::None;
            return;
        }

        let provider = remaining_providers.remove(0);
        self.connection_state.wizard = WizardState::InputRemoteName {
            provider,
            input_buffer: String::new(),
            selected_providers: remaining_providers,
        };
    }

    pub(crate) async fn handle_key_event(
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
                            let _ = crate::functions::rpc("config/delete", &param);
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

                                let op_res = crate::functions::rpc_async(method, param.to_string()).await;
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

                                    let op_res = crate::functions::rpc_async(method, param.to_string()).await;
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
                                    let _ = std::process::Command::new("kill").arg(service.pid.to_string()).status();
                                    if service.service_type_str == "Mount" {
                                        let _ = std::process::Command::new("fusermount")
                                            .args(["-uz", &service.path])
                                            .status();
                                    }
                                }
                                #[cfg(not(unix))]
                                {
                                    let _ = std::process::Command::new("taskkill")
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
                                
                                // Stop, disable, delete and reload service configuration
                                let res = if service.is_user {
                                    let _ = std::process::Command::new("systemctl")
                                        .args(["--user", "stop", &service.name])
                                        .status();
                                    let _ = std::process::Command::new("systemctl")
                                        .args(["--user", "disable", &service.name])
                                        .status();
                                    let r = std::fs::remove_file(&service.file_path);
                                    let _ = std::process::Command::new("systemctl")
                                        .args(["--user", "daemon-reload"])
                                        .status();
                                    r
                                } else {
                                    let status = std::process::Command::new("pkexec")
                                        .args([
                                            "sh",
                                            "-c",
                                            "systemctl stop \"$1\" && systemctl disable \"$1\" && rm -f \"$2\" && systemctl daemon-reload",
                                            "_",
                                            &service.name,
                                            &service.file_path,
                                        ])
                                        .status();
                                    match status {
                                        Ok(st) if st.success() => Ok(()),
                                        Ok(_) => Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "pkexec commands failed")),
                                        Err(e) => Err(e),
                                    }
                                };

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

        // ESC hủy popups thông báo chung (chặn tất cả phím khác khi đang hiện thông báo)
        if self.connection_state.error_message.is_some()
            || self.connection_state.info_message.is_some()
            || self.explorer_state.notification.is_some()
            || self.profile_state.error_message.is_some()
            || self.services_state.error_message.is_some()
            || self.services_state.info_message.is_some()
        {
            if key.code == KeyCode::Esc {
                self.connection_state.error_message = None;
                self.connection_state.info_message = None;
                self.explorer_state.notification = None;
                self.profile_state.error_message = None;
                self.services_state.error_message = None;
                self.services_state.info_message = None;
            }
            return;
        }

        match self.screen {
            Screen::MainMenu => crate::functions::keys::handle_menu_keys(self, key, tx).await,
            Screen::ConnectionManager => crate::functions::keys::handle_connection_keys(self, key, tx).await,
            Screen::FileExplorer => crate::functions::keys::handle_explorer_keys(self, key, tx).await,
            Screen::JobMonitor => crate::functions::keys::handle_monitor_keys(self, key, tx).await,
            Screen::ConfigProfileManager => crate::functions::keys::handle_profile_keys(self, key, tx).await,
            Screen::ServicesAndMounts => crate::functions::keys::handle_services_keys(self, key, tx).await,
            Screen::LanguageSelect => crate::functions::keys::handle_language_keys(self, key).await,
            Screen::DependencyManager => crate::functions::keys::handle_dependency_keys(self, key).await,
        }
    }
}
