use crate::rclone;
use crate::ui;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

use crate::app::{
    App, AppEvent, run_rpc_job_async, run_rpc_job_async_with_progress,
    copy_to_system_clipboard, parse_parent_and_child, strip_archive_extensions, ScanState, execute_restricted_copy, create_all_source_directories
};

impl App {

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

                    // Cập nhật remote_dependencies và remote_types từ config/dump
                    self.remote_dependencies.clear();
                    self.remote_types.clear();
                    let dump_res = rclone::rpc_async("config/dump".to_string(), "{}".to_string()).await;
                    if let Ok(rpc_dump) = dump_res {
                        if let Ok(dump_val) = serde_json::from_str::<Value>(&rpc_dump.output) {
                            if let Some(obj) = dump_val.as_object() {
                                for (name, details) in obj {
                                    if let Some(details_obj) = details.as_object() {
                                        if let Some(r_type) = details_obj.get("type").and_then(|t| t.as_str()) {
                                            self.remote_types.insert(name.clone(), r_type.to_string());
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
                                    let id = item_val
                                        .get("ID")
                                        .and_then(|i| i.as_str())
                                        .map(|s| s.to_string());

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
                                        id,
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
                                            id: None,
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

                                        let id = item_val
                                            .get("ID")
                                            .and_then(|i| i.as_str())
                                            .map(|s| s.to_string());

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
                                                id,
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
                                                id: None,
                                            },
                                        );
                                    }

                                    let _ = tx.send(AppEvent::WizardGuiListResult {
                                        result: Ok(items),
                                    });
                                }
                            }
                        } else {
                            let err_msg = serde_json::from_str::<serde_json::Value>(&rpc_res.output)
                                .ok()
                                .and_then(|val| val.get("error").and_then(|e| e.as_str()).map(|s| s.to_string()))
                                .unwrap_or_else(|| format!("Lỗi kết nối RPC: {}", rpc_res.status));
                            let _ = tx.send(AppEvent::WizardGuiListResult {
                                result: Err(err_msg),
                            });
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::WizardGuiListResult { result: Err(e) });
                    }
                }
            });
    }

    pub(crate) fn handle_explorer_list_result(
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
            }
            Err(e) => {
                pane.items = Vec::new();
                self.explorer_state.notification = Some(("LỖI EXPLORER".to_string(), e));
            }
        }
    }

    pub(crate) fn save_features_cache(&self) {
        let cache_path = crate::app_config::AppConfig::config_dir().join("features_cache.json");
        if let Some(parent) = cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(serialized) = serde_json::to_string_pretty(&self.features_cache) {
            let _ = std::fs::write(&cache_path, serialized);
        }
    }

    pub(crate) fn check_features_and_execute(
        &mut self,
        action_type: &str,
        src: String,
        dest: String,
        is_dir: bool,
        use_checksum: bool,
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
                use_checksum,
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
                use_checksum,
            });
        });
    }

    pub(crate) async fn handle_features_checked(
        &mut self,
        action_type: String,
        src: String,
        dest: String,
        src_features: Option<serde_json::Value>,
        dst_features: Option<serde_json::Value>,
        is_dir: bool,
        use_checksum: bool,
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
                        None,
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
                    restricted_files: None,
                    restricted_scroll: 0,
                    focus_files: false,
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
                    restricted_files: None,
                    restricted_scroll: 0,
                    focus_files: false,
                };
            }
        } else if action_type == "copy" {
            if dst_copy {
                self.explorer_state.popup = ui::explorer::ExplorerPopup::PermissionScanning {
                    src: src.clone(),
                    dest: dest.clone(),
                    is_dir,
                    scanned_count: 0,
                    total_files: 0,
                    restricted_count: 0,
                };
                let tx_check = tx.clone();
                let src_clone = src.clone();
                let dest_clone = dest.clone();
                let is_dir_clone = is_dir;
                let scan_concurrency = self.config.scan_concurrency;
                
                let skip_flag = self.skip_permission_precheck.clone();
                skip_flag.store(false, std::sync::atomic::Ordering::Relaxed);
                
                tokio::spawn(async move {
                    let mut restricted_files = Vec::new();
                    let mut single_file_size = 0u64;
                    let total_files;
                    let total_size;

                    if !is_dir_clone {
                        // Check single file
                        let (src_fs, filename) = parse_parent_and_child(&src_clone);
                        let list_param = json!({
                            "fs": src_fs,
                            "remote": filename,
                            "opt": {
                                "recurse": false,
                                "metadata": true
                            }
                        }).to_string();

                        if let Ok(res) = rclone::rpc_async("operations/list".to_string(), list_param).await {
                            if res.status == 200 {
                                if let Ok(val) = serde_json::from_str::<Value>(&res.output) {
                                    if let Some(list_arr) = val.get("list").and_then(|l| l.as_array()) {
                                        for item in list_arr {
                                            let size = item.get("Size").and_then(|s| s.as_u64()).unwrap_or(0);
                                            single_file_size = size;
                                            let is_restricted = if let Some(meta) = item.get("Metadata") {
                                                meta.get("copy-requires-writer-permission")
                                                    .and_then(|v| v.as_str())
                                                    == Some("true")
                                            } else {
                                                false
                                            };
                                            let mime_type = item.get("MimeType").and_then(|m| m.as_str()).unwrap_or("");
                                            let is_dangling = mime_type.contains("shortcut.dangling");

                                            if is_restricted || is_dangling {
                                                restricted_files.push(src_clone.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        let scanned_count = 1;
                        total_files = 1;
                        total_size = single_file_size;
                        if skip_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            restricted_files = Vec::new();
                        }
                        // Send progress update
                        let _ = tx_check.send(AppEvent::PermissionScanProgress {
                            src: src_clone.clone(),
                            dest: dest_clone.clone(),
                            is_dir: is_dir_clone,
                            scanned_count,
                            total_files: 1,
                            restricted_count: restricted_files.len(),
                        });
                    } else {
                        // Concurrent directory walk
                        let state = Arc::new(Mutex::new(ScanState {
                            queue: vec!["".to_string()],
                            active_tasks: 0,
                            files: Vec::new(),
                            restricted_files: Vec::new(),
                            total_size: 0,
                        }));
                        let notify = Arc::new(Notify::new());
                        let max_concurrency = scan_concurrency;

                        loop {
                            if skip_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                break;
                            }
                            let mut to_spawn = 0;
                            let mut finished = false;

                            {
                                let s = state.lock().unwrap();
                                if s.queue.is_empty() && s.active_tasks == 0 {
                                    finished = true;
                                } else {
                                    let available_slots = max_concurrency - s.active_tasks;
                                    to_spawn = available_slots.min(s.queue.len());
                                }
                            }

                            if finished {
                                break;
                            }

                            if to_spawn == 0 {
                                notify.notified().await;
                                continue;
                            }

                            for _ in 0..to_spawn {
                                let dir = {
                                    let mut s = state.lock().unwrap();
                                    s.active_tasks += 1;
                                    s.queue.remove(0)
                                };

                                let state_clone = Arc::clone(&state);
                                let notify_clone = Arc::clone(&notify);
                                let folder_fs = src_clone.clone();
                                let tx_progress = tx_check.clone();
                                let src_p = src_clone.clone();
                                let dest_p = dest_clone.clone();

                                tokio::spawn(async move {
                                    let list_param = json!({
                                        "fs": folder_fs,
                                        "remote": dir,
                                        "opt": {
                                            "recurse": false,
                                            "metadata": true
                                        }
                                    }).to_string();

                                    let mut new_dirs = Vec::new();
                                    let mut new_files = Vec::new();
                                    let mut new_restricted = Vec::new();
                                    let mut new_sizes_sum = 0u64;

                                    if let Ok(res) = rclone::rpc_async("operations/list".to_string(), list_param).await {
                                        if res.status == 200 {
                                            if let Ok(val) = serde_json::from_str::<Value>(&res.output) {
                                                if let Some(list_arr) = val.get("list").and_then(|l| l.as_array()) {
                                                    for item in list_arr {
                                                        let is_item_dir = item.get("IsDir").and_then(|d| d.as_bool()).unwrap_or(false);
                                                        let path = item.get("Path").and_then(|p| p.as_str()).unwrap_or("");

                                                        if is_item_dir {
                                                            new_dirs.push(path.to_string());
                                                        } else {
                                                            new_files.push(path.to_string());
                                                            let size = item.get("Size").and_then(|s| s.as_u64()).unwrap_or(0);
                                                            new_sizes_sum += size;
                                                            let is_restricted = if let Some(meta) = item.get("Metadata") {
                                                                meta.get("copy-requires-writer-permission")
                                                                    .and_then(|v| v.as_str())
                                                                    == Some("true")
                                                            } else {
                                                                false
                                                            };
                                                            let mime_type = item.get("MimeType").and_then(|m| m.as_str()).unwrap_or("");
                                                            let is_dangling = mime_type.contains("shortcut.dangling");

                                                            if is_restricted || is_dangling {
                                                                new_restricted.push(path.to_string());
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    let (current_scanned, current_restricted) = {
                                        let mut s = state_clone.lock().unwrap();
                                        s.queue.extend(new_dirs);
                                        s.files.extend(new_files);
                                        s.restricted_files.extend(new_restricted);
                                        s.total_size += new_sizes_sum;
                                        s.active_tasks -= 1;
                                        (s.files.len(), s.restricted_files.len())
                                    };

                                    // Send progress update
                                    let _ = tx_progress.send(AppEvent::PermissionScanProgress {
                                        src: src_p,
                                        dest: dest_p,
                                        is_dir: true,
                                        scanned_count: current_scanned,
                                        total_files: 0,
                                        restricted_count: current_restricted,
                                    });

                                    notify_clone.notify_one();
                                });
                            }
                        }

                        let final_state = state.lock().unwrap();
                        if skip_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            restricted_files = Vec::new();
                        } else {
                            restricted_files = final_state.restricted_files.clone();
                        }
                        total_files = final_state.files.len() as u64;
                        total_size = final_state.total_size;
                    }

                    if restricted_files.is_empty() {
                        let _ = tx_check.send(AppEvent::PermissionCheckPassed {
                            src: src_clone,
                            dest: dest_clone,
                            is_dir: is_dir_clone,
                            use_checksum,
                            total_files,
                            total_size,
                        });
                    } else {
                        let _ = tx_check.send(AppEvent::PermissionErrorDetected {
                            src: src_clone,
                            dest: dest_clone,
                            is_dir: is_dir_clone,
                            restricted_files,
                            use_checksum,
                            total_files,
                            total_size,
                        });
                    }
                });

            } else {
                let mut options = Vec::new();
                let mut actions = Vec::new();

                options.push("Tải về máy rồi Upload lên đích (Local Transfer - Rất chậm)".to_string());
                actions.push(ui::explorer::FallbackAction::CopyLocalTransfer {
                    src: src.clone(),
                    dest: dest.clone(),
                    use_checksum,
                });

                options.push("Hủy bỏ tác vụ".to_string());
                actions.push(ui::explorer::FallbackAction::Cancel);

                self.explorer_state.popup = ui::explorer::ExplorerPopup::ConfirmFallback {
                    title: "SAO CHÉP KHÔNG ĐƯỢC HỖ TRỢ".to_string(),
                    options,
                    selected_idx: 0,
                    actions,
                    restricted_files: None,
                    restricted_scroll: 0,
                    focus_files: false,
                };
            }
        }
    }

    pub(crate) async fn execute_fallback_action(
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
                        None,
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
                            None,
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
            ui::explorer::FallbackAction::CopyNative { src, dest, use_checksum }
            | ui::explorer::FallbackAction::CopyLocalTransfer { src, dest, use_checksum } => {
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
                    let mut param = json!({
                        "srcFs": src_clone,
                        "dstFs": dest_clone,
                    });
                    if use_checksum {
                        if let Some(obj) = param.as_object_mut() {
                            obj.insert("_config".to_string(), json!({ "checksum": true }));
                        }
                    }
                    let res = run_rpc_job_async_with_progress(
                        "sync/copy".to_string(),
                        param,
                        Some((src_clone, dest_clone, true)),
                        Some(tx_copy.clone()),
                        None,
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
                let op_id = format!("del_fallback_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                let op = crate::app::ActiveOperation {
                    id: op_id.clone(),
                    action_type: "delete".to_string(),
                    src: target.clone(),
                    dest: String::new(),
                    items: Vec::new(),
                    is_dir,
                    use_checksum: false,
                    is_copy: false,
                    completed_items: Some(Vec::new()),
                };
                crate::app::save_active_operation(&op);
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
                    crate::app::remove_active_operation(&op_id);
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
                let op_id = format!("del_indiv_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                let op = crate::app::ActiveOperation {
                    id: op_id.clone(),
                    action_type: "delete".to_string(),
                    src: target.clone(),
                    dest: String::new(),
                    items: Vec::new(),
                    is_dir: true,
                    use_checksum: false,
                    is_copy: false,
                    completed_items: Some(Vec::new()),
                };
                crate::app::save_active_operation(&op);
                tokio::spawn(async move {
                    let res = run_rpc_job_async("operations/purge".to_string(), json!({ "fs": target, "remote": "" })).await;
                    crate::app::remove_active_operation(&op_id);
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
                                None,
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
                                None,
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
            ui::explorer::FallbackAction::PermissionCancel => {}
            ui::explorer::FallbackAction::PermissionCopyAsMuchAsPossible { src, dest, is_dir, restricted_files: _, use_checksum } => {
                self.explorer_state.popup = ui::explorer::ExplorerPopup::CopyProgress {
                    src: src.clone(),
                    dest: dest.clone(),
                    pct: 0.0,
                    job_id: None,
                };
                let tx_copy = tx.clone();
                let src_clone = src.clone();
                let dest_clone = dest.clone();
                let op_id = format!("copy_as_much_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                let op = crate::app::ActiveOperation {
                    id: op_id.clone(),
                    action_type: "copy".to_string(),
                    src: src_clone.clone(),
                    dest: dest_clone.clone(),
                    items: vec![src_clone.clone()],
                    is_dir,
                    use_checksum,
                    is_copy: true,
                    completed_items: Some(Vec::new()),
                };
                crate::app::save_active_operation(&op);

                tokio::spawn(async move {
                    if is_dir {
                        let _ = create_all_source_directories(&src_clone, &dest_clone).await;
                    }
                    let mut param = serde_json::json!({
                        "srcFs": src_clone,
                        "dstFs": dest_clone,
                    });
                    if use_checksum {
                        if let Some(obj) = param.as_object_mut() {
                            obj.insert("_config".to_string(), json!({ "checksum": true }));
                        }
                    }
                    let res = run_rpc_job_async_with_progress(
                        "sync/copy".to_string(),
                        param,
                        Some((src_clone, dest_clone, true)),
                        Some(tx_copy.clone()),
                        None,
                    ).await;
                    let result = match res {
                        Ok(()) => Ok(()),
                        Err(e) => {
                            let err_lower = e.to_lowercase();
                            if err_lower.contains("restrictedlink")
                                || err_lower.contains("download")
                                || err_lower.contains("forbidden")
                                || err_lower.contains("only the owner")
                            {
                                Ok(())
                            } else {
                                Err(e)
                            }
                        }
                    };
                    let _ = tx_copy.send(AppEvent::ExplorerOperationFinished {
                        pane: ui::explorer::ActivePane::Left,
                        op_name: "sao chép nhiều nhất có thể (copy)".to_string(),
                        result,
                    });
                });
            }
            ui::explorer::FallbackAction::PermissionRestrictedCopy { src, dest, is_dir, restricted_files: _, use_checksum } => {
                self.explorer_state.popup = ui::explorer::ExplorerPopup::CopyProgress {
                    src: src.clone(),
                    dest: dest.clone(),
                    pct: 0.0,
                    job_id: None,
                };
                let tx_copy = tx.clone();
                let src_clone = src.clone();
                let dest_clone = dest.clone();
                let op_id = format!("restr_copy_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                let op = crate::app::ActiveOperation {
                    id: op_id.clone(),
                    action_type: "copy".to_string(),
                    src: src_clone.clone(),
                    dest: dest_clone.clone(),
                    items: vec![src_clone.clone()],
                    is_dir,
                    use_checksum,
                    is_copy: true,
                    completed_items: Some(Vec::new()),
                };
                crate::app::save_active_operation(&op);

                tokio::spawn(async move {
                    let res = execute_restricted_copy(src_clone, dest_clone, is_dir, use_checksum, tx_copy.clone()).await;
                    let _ = tx_copy.send(AppEvent::ExplorerOperationFinished {
                        pane: ui::explorer::ActivePane::Left,
                        op_name: "sao chép hạn chế (restricted copy)".to_string(),
                        result: res,
                    });
                });
            }
            ui::explorer::FallbackAction::MultiPermissionCopyAsMuchAsPossible { items, dest_remote, dest_path, restricted_files: _, use_checksum } => {
                let dest_full = if dest_remote.is_empty() { dest_path.clone() } else { format!("{}:{}", dest_remote, dest_path) };
                self.explorer_state.popup = ui::explorer::ExplorerPopup::CopyProgress {
                    src: format!("({} mục)", items.len()),
                    dest: dest_full.clone(),
                    pct: 0.0,
                    job_id: None,
                };
                let tx_op = tx.clone();
                let dest_remote_clone = dest_remote.clone();
                let dest_path_clone = dest_path.clone();
                let items_clone = items.clone();
                let pane_type = self.explorer_state.active_pane.clone();
                let op_id = format!("multi_copy_as_much_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                let op = crate::app::ActiveOperation {
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
                crate::app::save_active_operation(&op);

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
                            json!({ "srcFs": src, "dstFs": dest })
                        } else {
                            json!({ "srcFs": src.rsplit_once('/').map(|(p,_)| p).unwrap_or(&src), "srcRemote": clip_item.name, "dstFs": dest.rsplit_once('/').map(|(p,_)| p).unwrap_or(&dest), "dstRemote": clip_item.name })
                        };

                        if use_checksum {
                            if let Some(obj) = param.as_object_mut() {
                                obj.insert("_config".to_string(), json!({ "checksum": true }));
                            }
                        }

                        if clip_item.is_dir {
                            let _ = create_all_source_directories(&src, &dest).await;
                        }

                        let res = run_rpc_job_async(method.to_string(), param).await;
                        crate::app::complete_item_in_active_operation(&op_id, &clip_item.name);
                        if let Err(e) = res {
                            let err_lower = e.to_lowercase();
                            if err_lower.contains("restrictedlink")
                                || err_lower.contains("download")
                                || err_lower.contains("forbidden")
                                || err_lower.contains("only the owner")
                            {
                                // Bỏ qua lỗi do file bị hạn chế download
                            } else {
                                last_err = Some(e);
                            }
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
            ui::explorer::FallbackAction::MultiPermissionRestrictedCopy { items, dest_remote, dest_path, restricted_files: _, use_checksum } => {
                let dest_full = if dest_remote.is_empty() { dest_path.clone() } else { format!("{}:{}", dest_remote, dest_path) };
                self.explorer_state.popup = ui::explorer::ExplorerPopup::CopyProgress {
                    src: format!("({} mục)", items.len()),
                    dest: dest_full.clone(),
                    pct: 0.0,
                    job_id: None,
                };
                let tx_op = tx.clone();
                let dest_remote_clone = dest_remote.clone();
                let dest_path_clone = dest_path.clone();
                let items_clone = items.clone();
                let pane_type = self.explorer_state.active_pane.clone();
                let op_id = format!("multi_restr_copy_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                let op = crate::app::ActiveOperation {
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
                crate::app::save_active_operation(&op);

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

                        let res = execute_restricted_copy(src, dest, clip_item.is_dir, use_checksum, tx_op.clone()).await;
                        crate::app::complete_item_in_active_operation(&op_id, &clip_item.name);
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
                        op_name: "sao chép hạn chế nhiều mục".to_string(),
                        result,
                    });
                });
            }
            ui::explorer::FallbackAction::Cancel => {}
        }
    }

    pub(crate) fn load_profile_list(&mut self) {
        let mut list = Vec::new();
        for (name, path) in &self.config.profiles {
            list.push((name.clone(), path.clone()));
        }
        list.sort_by(|a, b| a.0.cmp(&b.0));
        self.profile_state.profiles = list;
    }

    pub(crate) async fn handle_special_action_selected(
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
                    restricted_files: None,
                    restricted_scroll: 0,
                    focus_files: false,
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
                    restricted_files: None,
                    restricted_scroll: 0,
                    focus_files: false,
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
            7 => {
                self.explorer_state.popup = ui::explorer::ExplorerPopup::DedupeModeSelect {
                    by_hash: false,
                    selected_idx: 0,
                };
            }
            8 => {
                let selected_dirs: Vec<ui::explorer::FileItem> = active_pane.items.iter()
                    .filter(|item| item.is_dir && item.name != ".." && active_pane.selected_names.contains(&item.name))
                    .cloned()
                    .collect();

                if selected_dirs.len() >= 2 {
                    self.explorer_state.popup = ui::explorer::ExplorerPopup::MergeSimilarDestinationSelect {
                        folders: selected_dirs,
                        selected_idx: 0,
                    };
                } else {
                    let mut groups: std::collections::HashMap<String, Vec<ui::explorer::FileItem>> = std::collections::HashMap::new();
                    for item in &active_pane.items {
                        if item.is_dir && item.name != ".." {
                            let normalized = item.name.trim().to_lowercase();
                            groups.entry(normalized).or_default().push(item.clone());
                        }
                    }
                    
                    let merge_groups: Vec<(String, Vec<ui::explorer::FileItem>)> = groups
                        .into_iter()
                        .filter(|(_, items)| items.len() > 1)
                        .map(|(_, items)| {
                            let canonical = items[0].name.trim().to_string();
                            (canonical, items)
                        })
                        .collect();

                    if merge_groups.is_empty() {
                        self.explorer_state.notification = Some((
                            "CẢNH BÁO".to_string(),
                            crate::lang::translate("exp_no_similar_dirs"),
                        ));
                    } else {
                        // Take the first found group
                        let folders = merge_groups[0].1.clone();
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::MergeSimilarDestinationSelect {
                            folders,
                            selected_idx: 0,
                        };
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) async fn execute_hashsum_file(
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

    pub(crate) async fn execute_cryptdecode(
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

    pub(crate) async fn handle_decompress_mode_selected(
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

    pub(crate) async fn execute_archive_decompress(
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
                None,
            ).await;
            
            let _ = tx_clone.send(AppEvent::ExplorerOperationFinished {
                pane: ui::explorer::ActivePane::Left,
                op_name: "giải nén (extract archive)".to_string(),
                result: res,
            });
        });
    }

    pub(crate) async fn execute_dedupe(
        &mut self,
        mode: String,
        by_hash: bool,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        let active_pane = self.explorer_state.get_active_pane();
        let fs = if active_pane.remote.is_empty() {
            active_pane.path.clone()
        } else {
            format!("{}:{}", active_pane.remote.trim_end_matches(':'), active_pane.path)
        };

        self.explorer_state.popup = ui::explorer::ExplorerPopup::CopyProgress {
            src: format!("Lọc trùng: {}", fs),
            dest: String::new(),
            pct: 0.0,
            job_id: None,
        };

        let tx_clone = tx.clone();
        let fs_clone = fs.clone();
        let mode_clone = mode.clone();
        tokio::spawn(async move {
            let res = run_rpc_job_async_with_progress(
                "operations/dedupe".to_string(),
                json!({
                    "fs": fs_clone,
                    "mode": mode_clone,
                    "byHash": by_hash,
                    "_description": format!("Lọc trùng: {} ({})", fs_clone, mode_clone),
                }),
                None,
                Some(tx_clone.clone()),
                None,
            ).await;

            let _ = tx_clone.send(AppEvent::ExplorerOperationFinished {
                pane: ui::explorer::ActivePane::Left,
                op_name: format!("Lọc trùng (dedupe: {})", mode_clone),
                result: res,
            });
        });
    }

    pub(crate) async fn execute_merge_similar(
        &mut self,
        folders: Vec<ui::explorer::FileItem>,
        destination_idx: usize,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        let active_pane = self.explorer_state.get_active_pane();
        let current_dir = active_pane.path.clone();
        let remote = active_pane.remote.clone();
        let clean_remote = remote.trim_end_matches(':').to_string();
        let is_drive = if !clean_remote.is_empty() {
            self.remote_types.iter().any(|(k, v)| k.eq_ignore_ascii_case(&clean_remote) && v == "drive")
        } else {
            false
        };

        self.explorer_state.popup = ui::explorer::ExplorerPopup::CopyProgress {
            src: "Đang gộp thư mục tương tự...".to_string(),
            dest: String::new(),
            pct: 0.0,
            job_id: None,
        };

        let tx_clone = tx.clone();
        tokio::spawn(async move {
            let mut errors = Vec::new();
            let dest_folder = &folders[destination_idx];

            crate::app_config::log_info(&format!(
                "Bắt đầu execute_merge_similar. Remote: '{}', Path: '{}', is_drive: {}, destination: '{}' (ID: {:?})",
                remote, current_dir, is_drive, dest_folder.name, dest_folder.id
            ));

            let clean_remote_str = clean_remote.clone();
            let clean_path = if current_dir.starts_with('/') {
                current_dir.to_string()
            } else {
                format!("/{}", current_dir)
            };
            let parent_path = if remote.is_empty() {
                clean_path
            } else {
                format!("{}:{}", clean_remote_str, clean_path)
            };

            if is_drive && dest_folder.id.is_some() {
                let dst_id = dest_folder.id.as_ref().unwrap();
                for (idx, folder) in folders.iter().enumerate() {
                    if idx == destination_idx {
                        continue;
                    }
                    if let Some(ref src_id) = folder.id {
                        let src_fs = format!("{},root_folder_id={}:", clean_remote_str, src_id);
                        let dst_fs = format!("{},root_folder_id={}:", clean_remote_str, dst_id);
                        
                        crate::app_config::log_info(&format!(
                            "[Google Drive ID Flow] Đang gộp '{}' (ID: {}) -> '{}' (ID: {}). srcFs: '{}', dstFs: '{}'",
                            folder.name, src_id, dest_folder.name, dst_id, src_fs, dst_fs
                        ));

                        let move_res = run_rpc_job_async("sync/move".to_string(), json!({
                            "srcFs": src_fs,
                            "dstFs": dst_fs,
                            "deleteEmptySrcDirs": true,
                            "createEmptySrcDirs": true,
                        })).await;
                        if let Err(e) = move_res {
                            let err_msg = format!("Lỗi gộp thư mục '{}': {}", folder.name, e);
                            crate::app_config::log_info(&format!("[Google Drive ID Flow] {}", err_msg));
                            errors.push(err_msg);
                        } else {
                            crate::app_config::log_info(&format!(
                                "[Google Drive ID Flow] Di chuyển thành công các tệp từ '{}' sang '{}'",
                                folder.name, dest_folder.name
                            ));

                            // Xóa cache của rclone trước khi quét và dọn dẹp để đảm bảo phân giải tên chính xác case-sensitive
                            let _ = rclone::rpc_async("fscache/clear".to_string(), "{}".to_string()).await;

                            // Kiểm tra đệ quy xem còn tệp tin nào ở nguồn không trước khi purge
                            let list_param = json!({
                                "fs": src_fs,
                                "remote": "",
                                "opt": {
                                    "recurse": true,
                                    "metadata": false
                                }
                            }).to_string();

                            let mut has_files = false;
                            let list_res = rclone::rpc_async("operations/list".to_string(), list_param).await;
                            crate::app_config::log_info(&format!(
                                "[Google Drive ID Flow] Kết quả list_param ở nguồn '{}': {:?}",
                                folder.name, list_res
                            ));
                            match list_res {
                                Ok(rpc_res) if rpc_res.status == 200 => {
                                    if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                                        if let Some(list_arr) = val.get("list").and_then(|l| l.as_array()) {
                                            for item in list_arr {
                                                let is_dir = item.get("IsDir").and_then(|d| d.as_bool()).unwrap_or(false);
                                                if !is_dir {
                                                    has_files = true;
                                                    crate::app_config::log_info(&format!(
                                                        "[Google Drive ID Flow] Thư mục '{}' vẫn còn file: {}",
                                                        folder.name, item.get("Path").and_then(|p| p.as_str()).unwrap_or("")
                                                    ));
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }

                            if has_files {
                                let warn_msg = format!(
                                    "Cảnh báo: Thư mục nguồn '{}' vẫn còn chứa tệp tin chưa được di chuyển. Giữ lại thư mục nguồn để an toàn dữ liệu.",
                                    folder.name
                                );
                                crate::app_config::log_info(&format!("[Google Drive ID Flow] {}", warn_msg));
                                errors.push(warn_msg);
                            } else {
                                crate::app_config::log_info(&format!(
                                    "[Google Drive ID Flow] Không phát hiện tệp tin còn lại ở nguồn '{}'. Tiến hành purge...",
                                    folder.name
                                ));

                                // Xóa cache lần nữa trước khi purge để rclone bắt buộc quét lại parent path case-sensitively
                                let _ = rclone::rpc_async("fscache/clear".to_string(), "{}".to_string()).await;

                                // Xóa thư mục nguồn bằng purge qua parent_path và folder.name (vì purge qua ID root không làm gì)
                                let purge_res = run_rpc_job_async("operations/purge".to_string(), json!({
                                    "fs": parent_path.clone(),
                                    "remote": folder.name.clone(),
                                })).await;
                                if let Err(e) = purge_res {
                                    let err_msg = format!("Lỗi dọn dẹp thư mục nguồn '{}': {}", folder.name, e);
                                    crate::app_config::log_info(&format!("[Google Drive ID Flow] {}", err_msg));
                                    errors.push(err_msg);
                                } else {
                                    crate::app_config::log_info(&format!(
                                        "[Google Drive ID Flow] Đã xóa thành công thư mục nguồn '{}' (parent_path: '{}')",
                                        folder.name, parent_path
                                    ));
                                }
                            }
                        }
                    } else {
                        errors.push(format!("Lỗi: Không tìm thấy ID của thư mục nguồn '{}'", folder.name));
                    }
                }
            } else {
                let parent_path_clone = parent_path.clone();
                let list_folder = move |folder_name: String| {
                    let parent_path = parent_path_clone.clone();
                    async move {
                        let param = json!({
                            "fs": parent_path,
                            "remote": folder_name,
                            "opt": {
                                "recurse": true,
                                "metadata": true
                            }
                        }).to_string();

                        rclone::rpc_async("operations/list".to_string(), param).await
                    }
                };

                // Scan destination
                let mut dest_files = std::collections::HashMap::new();
                let dest_res = list_folder(dest_folder.name.clone()).await;
                match dest_res {
                    Ok(rpc_res) if rpc_res.status == 200 => {
                        if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                            if let Some(list_arr) = val.get("list").and_then(|l| l.as_array()) {
                                let dest_prefix = format!("{}/", dest_folder.name);
                                for item in list_arr {
                                    if let Some(path_str) = item.get("Path").and_then(|p| p.as_str()) {
                                        if path_str == dest_folder.name {
                                            continue;
                                        }
                                        let rel_path = if path_str.starts_with(&dest_prefix) {
                                            &path_str[dest_prefix.len()..]
                                        } else {
                                            continue;
                                        };
                                        let is_dir = item.get("IsDir").and_then(|d| d.as_bool()).unwrap_or(false);
                                        if !is_dir {
                                            let size = item.get("Size").and_then(|s| s.as_u64()).unwrap_or(0);
                                            let hashes = item.get("Hashes").and_then(|h| h.as_object()).cloned();
                                            let mod_time = item.get("ModTime").and_then(|m| m.as_str()).unwrap_or("").to_string();
                                            dest_files.insert(rel_path.to_string(), (size, hashes, mod_time));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(rpc_res) => {
                        errors.push(format!("Lỗi quét thư mục đích '{}': {}", dest_folder.name, rpc_res.output));
                    }
                    Err(e) => {
                        errors.push(format!("Lỗi quét thư mục đích '{}': {}", dest_folder.name, e));
                    }
                }

                if !errors.is_empty() {
                    let _ = tx_clone.send(AppEvent::ExplorerOperationFinished {
                        pane: ui::explorer::ActivePane::Left,
                        op_name: "Gộp thư mục".to_string(),
                        result: Err(errors.join("\n")),
                    });
                    return;
                }

                // Iterate and process source folders
                for (idx, folder) in folders.iter().enumerate() {
                    if idx == destination_idx {
                        continue;
                    }

                    let src_res = list_folder(folder.name.clone()).await;
                    match src_res {
                        Ok(rpc_res) if rpc_res.status == 200 => {
                            if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                                if let Some(list_arr) = val.get("list").and_then(|l| l.as_array()) {
                                    let src_prefix = format!("{}/", folder.name);
                                    for item in list_arr {
                                        if let Some(path_str) = item.get("Path").and_then(|p| p.as_str()) {
                                            if path_str == folder.name {
                                                continue;
                                            }
                                            let rel_path = if path_str.starts_with(&src_prefix) {
                                                &path_str[src_prefix.len()..]
                                            } else {
                                                continue;
                                            };

                                            let is_dir = item.get("IsDir").and_then(|d| d.as_bool()).unwrap_or(false);
                                            if is_dir {
                                                let dst_remote = format!("{}/{}", dest_folder.name, rel_path);
                                                let mkdir_res = run_rpc_job_async("operations/mkdir".to_string(), json!({
                                                    "fs": parent_path.clone(),
                                                    "remote": dst_remote,
                                                })).await;
                                                if let Err(e) = mkdir_res {
                                                    errors.push(format!("Lỗi tạo thư mục con trống '{}': {}", dst_remote, e));
                                                }
                                            } else {
                                                let size = item.get("Size").and_then(|s| s.as_u64()).unwrap_or(0);
                                                let hashes = item.get("Hashes").and_then(|h| h.as_object());
                                                let mod_time = item.get("ModTime").and_then(|m| m.as_str()).unwrap_or("").to_string();

                                                let src_remote = format!("{}/{}", folder.name, rel_path);
                                                let dst_remote = format!("{}/{}", dest_folder.name, rel_path);

                                                let mut action_move = true;
                                                let mut action_delete_src = false;

                                                if let Some((dst_size, dst_hashes, dst_mod_time)) = dest_files.get(rel_path) {
                                                    let mut is_identical = false;

                                                    if let (Some(s_hash), Some(d_hash)) = (hashes, dst_hashes) {
                                                        for (k, v) in s_hash {
                                                            if let Some(dv) = d_hash.get(k) {
                                                                if v == dv {
                                                                    is_identical = true;
                                                                    break;
                                                                }
                                                            }
                                                        }
                                                    } else {
                                                        if size == *dst_size {
                                                            is_identical = true;
                                                        }
                                                    }

                                                    if is_identical {
                                                        action_move = false;
                                                        action_delete_src = true;
                                                    } else {
                                                        if mod_time > *dst_mod_time {
                                                            action_move = true;
                                                        } else {
                                                            action_move = false;
                                                            action_delete_src = true;
                                                        }
                                                    }
                                                }

                                                if action_move {
                                                    let move_res = run_rpc_job_async("operations/movefile".to_string(), json!({
                                                        "srcFs": parent_path,
                                                        "srcRemote": src_remote,
                                                        "dstFs": parent_path,
                                                        "dstRemote": dst_remote,
                                                    })).await;
                                                    if let Err(e) = move_res {
                                                        errors.push(format!("Lỗi di chuyển tệp '{}': {}", src_remote, e));
                                                    }
                                                } else if action_delete_src {
                                                    let del_res = run_rpc_job_async("operations/delete".to_string(), json!({
                                                        "fs": parent_path,
                                                        "remote": src_remote,
                                                    })).await;
                                                    if let Err(e) = del_res {
                                                        errors.push(format!("Lỗi xóa tệp trùng ở nguồn '{}': {}", src_remote, e));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Ok(rpc_res) => {
                            errors.push(format!("Lỗi quét thư mục nguồn '{}': {}", folder.name, rpc_res.output));
                        }
                        Err(e) => {
                            errors.push(format!("Lỗi quét thư mục nguồn '{}': {}", folder.name, e));
                        }
                    }

                    crate::app_config::log_info(&format!(
                        "[Fallback Flow] Đang xử lý gộp nguồn '{}' -> đích '{}' (parent_path: '{}')",
                        folder.name, dest_folder.name, parent_path
                    ));

                    // Xóa cache của rclone trước khi quét đệ quy để tránh directory cache bị nhiễm bẩn case-insensitive
                    let _ = rclone::rpc_async("fscache/clear".to_string(), "{}".to_string()).await;

                    // Kiểm tra đệ quy xem còn tệp tin nào trong thư mục nguồn không trước khi purge
                    let list_param = json!({
                        "fs": parent_path.clone(),
                        "remote": folder.name.clone(),
                        "opt": {
                            "recurse": true,
                            "metadata": false
                        }
                    }).to_string();

                    let mut has_files = false;
                    let list_res = rclone::rpc_async("operations/list".to_string(), list_param).await;
                    crate::app_config::log_info(&format!(
                        "[Fallback Flow] Kết quả list_param ở nguồn '{}': {:?}",
                        folder.name, list_res
                    ));
                    match list_res {
                        Ok(rpc_res) if rpc_res.status == 200 => {
                            if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                                if let Some(list_arr) = val.get("list").and_then(|l| l.as_array()) {
                                    for item in list_arr {
                                        let is_dir = item.get("IsDir").and_then(|d| d.as_bool()).unwrap_or(false);
                                        if !is_dir {
                                            has_files = true;
                                            crate::app_config::log_info(&format!(
                                                "[Fallback Flow] Thư mục '{}' vẫn còn file: {}",
                                                folder.name, item.get("Path").and_then(|p| p.as_str()).unwrap_or("")
                                            ));
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }

                    if has_files {
                        let warn_msg = format!(
                            "Cảnh báo: Thư mục nguồn '{}' vẫn còn chứa tệp tin chưa được di chuyển. Giữ lại thư mục nguồn để an toàn dữ liệu.",
                            folder.name
                        );
                        crate::app_config::log_info(&format!("[Fallback Flow] {}", warn_msg));
                        errors.push(warn_msg);
                    } else {
                        crate::app_config::log_info(&format!(
                            "[Fallback Flow] Không phát hiện tệp tin còn lại ở nguồn '{}'. Tiến hành purge...",
                            folder.name
                        ));
                        // Xóa cache lần nữa trước khi purge để rclone bắt buộc quét lại parent path case-sensitively
                        let _ = rclone::rpc_async("fscache/clear".to_string(), "{}".to_string()).await;

                        // Xóa thư mục nguồn bằng purge (để xóa sạch thư mục gốc và các thư mục con rỗng)
                        let purge_res = run_rpc_job_async("operations/purge".to_string(), json!({
                            "fs": parent_path.clone(),
                            "remote": folder.name.clone(),
                        })).await;
                        if let Err(e) = purge_res {
                            let err_msg = format!("Lỗi dọn dẹp thư mục nguồn '{}': {}", folder.name, e);
                            crate::app_config::log_info(&format!("[Fallback Flow] {}", err_msg));
                            errors.push(err_msg);
                        } else {
                            crate::app_config::log_info(&format!(
                                "[Fallback Flow] Đã xóa thành công thư mục nguồn '{}' (parent_path: '{}')",
                                folder.name, parent_path
                            ));
                        }
                    }
                }
            }

            let final_res = if errors.is_empty() {
                crate::app_config::log_info("[execute_merge_similar] Hoàn tất gộp thành công không có lỗi.");
                Ok(())
            } else {
                let errs = errors.join("\n");
                crate::app_config::log_info(&format!("[execute_merge_similar] Hoàn tất gộp có lỗi: {}", errs));
                Err(errs)
            };

            let _ = tx_clone.send(AppEvent::ExplorerOperationFinished {
                pane: ui::explorer::ActivePane::Left,
                op_name: "Gộp thư mục".to_string(),
                result: final_res,
            });
        });
    }

    pub(crate) async fn execute_merge_similar_scan(
        &mut self,
        folders: Vec<ui::explorer::FileItem>,
        destination_idx: usize,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        let active_pane = self.explorer_state.get_active_pane();
        let current_dir = active_pane.path.clone();
        let remote = active_pane.remote.clone();
        let tx_clone = tx.clone();
        let folders_clone = folders.clone();

        tokio::spawn(async move {
            let mut scanned_count = 0;
            let total_folders = folders_clone.len();
            let mut dest_files = std::collections::HashMap::new();
            let mut dest_dirs = std::collections::HashSet::new();
            let mut source_files = Vec::new();
            let mut source_dirs = Vec::new();
            let mut errors = Vec::new();

            let dest_folder = &folders_clone[destination_idx];

            let clean_remote = remote.trim_end_matches(':');
            let clean_path = if current_dir.starts_with('/') {
                current_dir.to_string()
            } else {
                format!("/{}", current_dir)
            };
            let parent_path = if remote.is_empty() {
                clean_path
            } else {
                format!("{}:{}", clean_remote, clean_path)
            };

            let parent_path_clone = parent_path.clone();
            let list_folder = move |folder_name: String| {
                let parent_path = parent_path_clone.clone();
                async move {
                    let param = json!({
                        "fs": parent_path,
                        "remote": folder_name,
                        "opt": {
                            "recurse": true,
                            "metadata": true
                        }
                    }).to_string();

                    rclone::rpc_async("operations/list".to_string(), param).await
                }
            };

            let dest_res = list_folder(dest_folder.name.clone()).await;
            match dest_res {
                Ok(rpc_res) if rpc_res.status == 200 => {
                    if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                        if let Some(list_arr) = val.get("list").and_then(|l| l.as_array()) {
                            let dest_prefix = format!("{}/", dest_folder.name);
                            for item in list_arr {
                                if let Some(path_str) = item.get("Path").and_then(|p| p.as_str()) {
                                    if path_str == dest_folder.name {
                                        continue;
                                    }
                                    let rel_path = if path_str.starts_with(&dest_prefix) {
                                        &path_str[dest_prefix.len()..]
                                    } else {
                                        continue;
                                    };

                                    let is_dir = item.get("IsDir").and_then(|d| d.as_bool()).unwrap_or(false);
                                    if is_dir {
                                        dest_dirs.insert(rel_path.to_string());
                                    } else {
                                        let size = item.get("Size").and_then(|s| s.as_u64()).unwrap_or(0);
                                        let hashes = item.get("Hashes").and_then(|h| h.as_object()).cloned();
                                        let mod_time = item.get("ModTime").and_then(|m| m.as_str()).unwrap_or("").to_string();
                                        dest_files.insert(rel_path.to_string(), (size, hashes, mod_time));
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(rpc_res) => {
                    errors.push(format!("Lỗi quét thư mục đích '{}': {}", dest_folder.name, rpc_res.output));
                }
                Err(e) => {
                    errors.push(format!("Lỗi quét thư mục đích '{}': {}", dest_folder.name, e));
                }
            }

            scanned_count += 1;
            let _ = tx_clone.send(AppEvent::MergeSimilarScanProgress {
                folders_count: total_folders,
                scanned_count,
            });

            for (idx, folder) in folders_clone.iter().enumerate() {
                if idx == destination_idx {
                    continue;
                }

                let src_res = list_folder(folder.name.clone()).await;
                match src_res {
                    Ok(rpc_res) if rpc_res.status == 200 => {
                        if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                            if let Some(list_arr) = val.get("list").and_then(|l| l.as_array()) {
                                let src_prefix = format!("{}/", folder.name);
                                for item in list_arr {
                                    if let Some(path_str) = item.get("Path").and_then(|p| p.as_str()) {
                                        if path_str == folder.name {
                                            continue;
                                        }
                                        let rel_path = if path_str.starts_with(&src_prefix) {
                                            &path_str[src_prefix.len()..]
                                        } else {
                                            continue;
                                        };

                                        let is_dir = item.get("IsDir").and_then(|d| d.as_bool()).unwrap_or(false);
                                        if is_dir {
                                            source_dirs.push((folder.name.clone(), rel_path.to_string()));
                                        } else {
                                            source_files.push((folder.name.clone(), rel_path.to_string(), item.clone()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(rpc_res) => {
                        errors.push(format!("Lỗi quét thư mục nguồn '{}': {}", folder.name, rpc_res.output));
                    }
                    Err(e) => {
                        errors.push(format!("Lỗi quét thư mục nguồn '{}': {}", folder.name, e));
                    }
                }

                scanned_count += 1;
                let _ = tx_clone.send(AppEvent::MergeSimilarScanProgress {
                    folders_count: total_folders,
                    scanned_count,
                });
            }

            if !errors.is_empty() {
                let _ = tx_clone.send(AppEvent::MergeSimilarScanFinished {
                    result: Err(errors.join("\n")),
                    folders: folders_clone,
                    destination_idx,
                });
                return;
            }

            let mut tree_root = ui::explorer::TreeNode::new(dest_folder.name.clone(), "".to_string(), true);

            for dir_path in &dest_dirs {
                let parts: Vec<&str> = dir_path.split('/').filter(|s| !s.is_empty()).collect();
                tree_root.insert(&parts, true, None);
            }

            for (file_path, _) in &dest_files {
                let parts: Vec<&str> = file_path.split('/').filter(|s| !s.is_empty()).collect();
                tree_root.insert(&parts, false, None);
            }

            for (src_folder, dir_path) in source_dirs {
                let parts: Vec<&str> = dir_path.split('/').filter(|s| !s.is_empty()).collect();
                if dest_dirs.contains(&dir_path) {
                    tree_root.insert(&parts, true, None);
                } else {
                    tree_root.insert(&parts, true, Some(format!("[+ DI CHUYỂN / MOVE] (từ \"{}\")", src_folder)));
                }
            }

            let mut total_moved = 0;
            let mut total_skipped = 0;
            let mut total_conflict = 0;

            let format_time = |t: &str| -> String {
                if t.len() >= 19 {
                    t[..19].replace("T", " ")
                } else {
                    t.to_string()
                }
            };

            for (src_folder, rel_path, file_val) in source_files {
                let parts: Vec<&str> = rel_path.split('/').filter(|s| !s.is_empty()).collect();
                let path = rel_path.as_str();
                let size = file_val.get("Size").and_then(|s| s.as_u64()).unwrap_or(0);
                let hashes = file_val.get("Hashes").and_then(|h| h.as_object());
                let mod_time = file_val.get("ModTime").and_then(|m| m.as_str()).unwrap_or("").to_string();
                let size_str = crate::ui::format_size(size);

                if let Some((dst_size, dst_hashes, dst_mod_time)) = dest_files.get(path) {
                    let mut is_identical = false;

                    if let (Some(s_hash), Some(d_hash)) = (hashes, dst_hashes) {
                        for (k, v) in s_hash {
                            if let Some(dv) = d_hash.get(k) {
                                if v == dv {
                                    is_identical = true;
                                    break;
                                }
                            }
                        }
                    } else {
                        if size == *dst_size {
                            is_identical = true;
                        }
                    }

                    if is_identical {
                        total_skipped += 1;
                        tree_root.insert(
                            &parts,
                            false,
                            Some("[- XÓA TRÙNG LẶP] (File ở nguồn sẽ bị xóa)".to_string())
                        );
                    } else {
                        total_conflict += 1;
                        let src_time_formatted = format_time(&mod_time);
                        let dst_time_formatted = format_time(dst_mod_time);

                        if mod_time > *dst_mod_time {
                            tree_root.insert(
                                &parts,
                                false,
                                Some(format!("[GHI ĐÈ / OVERWRITE] (Bản nguồn mới hơn: {})", src_time_formatted))
                            );
                        } else if mod_time < *dst_mod_time {
                            tree_root.insert(
                                &parts,
                                false,
                                Some(format!("[- XÓA BẢN CŨ] (Bản đích mới hơn: {})", dst_time_formatted))
                            );
                        } else {
                            tree_root.insert(
                                &parts,
                                false,
                                Some(format!("[GIỮ BẢN ĐÍCH] (Cùng thời gian sửa đổi: {})", src_time_formatted))
                            );
                        }
                    }
                } else {
                    total_moved += 1;
                    tree_root.insert(
                        &parts,
                        false,
                        Some(format!("[+ DI CHUYỂN / MOVE] ({} từ \"{}\")", size_str, src_folder))
                    );
                }
            }

            let mut summary_report = Vec::new();
            summary_report.push("==================================================================".to_string());
            summary_report.push("             BÁO CÁO XEM TRƯỚC GỘP THƯ MỤC / MERGE REPORT         ".to_string());
            summary_report.push("==================================================================".to_string());
            summary_report.push(format!(" Thư mục đích (Destination): \"{}\"", dest_folder.name));
            let src_names: Vec<String> = folders_clone.iter().enumerate()
                .filter(|(i, _)| *i != destination_idx)
                .map(|(_, f)| format!("\"{}\"", f.name))
                .collect();
            summary_report.push(format!(" Thư mục nguồn (Source):     {}", src_names.join(", ")));
            summary_report.push("------------------------------------------------------------------".to_string());
            summary_report.push(" TÓM TẮT KẾT QUẢ / SUMMARY:".to_string());
            summary_report.push(format!("  • Số tệp tin di chuyển mới:    {}", total_moved));
            summary_report.push(format!("  • Số tệp tin trùng (sẽ xóa ở nguồn):   {}", total_skipped));
            summary_report.push(format!("  • Số tệp tin xung đột (giữ file mới nhất): {}", total_conflict));
            summary_report.push("------------------------------------------------------------------".to_string());
            summary_report.push(" SƠ ĐỒ CÂY THƯ MỤC SAU KHI GỘP / MERGED FOLDER TREE:".to_string());

            let _ = tx_clone.send(AppEvent::MergeSimilarScanFinished {
                result: Ok((summary_report, tree_root)),
                folders: folders_clone,
                destination_idx,
            });
        });
    }
}

pub async fn get_directory_stats(src: &str) -> Option<(u64, u64)> {
    let param = serde_json::json!({
        "fs": src,
    }).to_string();
    if let Ok(res) = rclone::rpc_async("operations/size".to_string(), param).await {
        if res.status == 200 {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&res.output) {
                let count = val.get("count").and_then(|c| c.as_u64()).unwrap_or(0);
                let bytes = val.get("bytes").and_then(|b| b.as_u64()).unwrap_or(0);
                return Some((count, bytes));
            }
        }
    }
    None
}

pub fn calculate_optimal_threads(total_files: u64, total_size: u64, max_bandwidth: u64) -> (u64, u64) {
    let config = crate::app_config::AppConfig::load();

    // Nếu người dùng ép buộc cấu hình ghi đè số luồng cố định
    if let (Some(t_over), Some(c_over)) = (config.transfers_prior_fixed, config.checkers_prior_fixed) {
        return (t_over, c_over);
    }

    let mut transfers = if let Some(t_over) = config.transfers_prior_fixed {
        t_over
    } else {
        if total_files == 0 {
            config.min_transfers
        } else {
            let avg_file_size_bytes = total_size / total_files;

            if avg_file_size_bytes >= 10_000_000 {
                // File kích thước lớn (>= 10MB)
                // Số luồng tối ưu tỉ lệ thuận với số lượng file.
                let max_large = config.max_transfers.max(config.min_transfers);
                (total_files / 2).clamp(config.min_transfers, max_large)
            } else {
                // Đối với file nhỏ (< 10MB), độ trễ API cloud (latency) đóng vai trò chính.
                // Ước lượng độ trễ API trung bình mỗi request file là 1.0 giây
                let latency_secs = 1.0;
                let single_thread_throughput = (avg_file_size_bytes as f64) / latency_secs;
                if single_thread_throughput <= 0.0 {
                    config.max_transfers
                } else {
                    let required_transfers = (max_bandwidth as f64) / single_thread_throughput;
                    (required_transfers.round() as u64).clamp(config.min_transfers, config.max_transfers)
                }
            }
        }
    };

    let checkers = if let Some(c_over) = config.checkers_prior_fixed {
        c_over
    } else {
        (transfers * 2).clamp(config.min_checkers, config.max_checkers)
    };

    transfers = transfers.min(config.max_transfers);

    (transfers, checkers)
}

pub async fn inject_optimal_thread_config(
    param: &mut serde_json::Value,
    src: &str,
    is_dir: bool,
    max_bandwidth: u64,
) -> (u64, u64) {
    let config = crate::app_config::AppConfig::load();
    
    // 1. Áp dụng prior_fixed từ cấu hình nếu có, ngược lại dùng default min_transfers / min_checkers
    let mut transfers = config.transfers_prior_fixed.unwrap_or(config.min_transfers);
    let mut checkers = config.checkers_prior_fixed.unwrap_or(config.min_checkers);

    if config.transfers_prior_fixed.is_none() || config.checkers_prior_fixed.is_none() {
        if is_dir {
            if let Some((count, bytes)) = get_directory_stats(src).await {
                let (opt_t, opt_c) = calculate_optimal_threads(count, bytes, max_bandwidth);
                if config.transfers_prior_fixed.is_none() {
                    transfers = opt_t;
                }
                if config.checkers_prior_fixed.is_none() {
                    checkers = opt_c;
                }
                crate::app_config::log_info(&format!(
                    "[Thread Optimizer] Thư mục: {} | Số file: {} | Tổng size: {} bytes | Băng thông: {} bytes/s -> Luồng tối ưu: Transfers={}, Checkers={}",
                    src, count, bytes, max_bandwidth, transfers, checkers
                ));
            } else {
                // Thất bại khi lấy size (timeout/lỗi mạng): Dùng luồng mặc định tối ưu cao hơn cho thư mục
                if config.transfers_prior_fixed.is_none() {
                    transfers = (config.min_transfers * 2).clamp(config.min_transfers, config.max_transfers);
                }
                if config.checkers_prior_fixed.is_none() {
                    checkers = (config.min_checkers * 2).clamp(config.min_checkers, config.max_checkers);
                }
                crate::app_config::log_info(&format!(
                    "[Thread Optimizer] Thất bại khi lấy size thư mục: {}. Sử dụng luồng mặc định tối ưu: Transfers={}, Checkers={}",
                    src, transfers, checkers
                ));
            }
        } else {
            // File đơn lẻ
            if config.transfers_prior_fixed.is_none() {
                transfers = 4.min(config.min_transfers).max(1);
            }
            if config.checkers_prior_fixed.is_none() {
                checkers = 8.min(config.min_checkers).max(1);
            }
        }
    }

    // Giới hạn bởi max_transfers / max_checkers
    transfers = transfers.min(config.max_transfers);
    checkers = checkers.min(config.max_checkers);
    
    if let Some(obj) = param.as_object_mut() {
        let mut config_obj = match obj.remove("_config") {
            Some(serde_json::Value::Object(o)) => o,
            _ => serde_json::Map::new(),
        };
        config_obj.insert("Transfers".to_string(), serde_json::json!(transfers));
        config_obj.insert("Checkers".to_string(), serde_json::json!(checkers));
        obj.insert("_config".to_string(), serde_json::Value::Object(config_obj));
    }
    
    (transfers, checkers)
}

