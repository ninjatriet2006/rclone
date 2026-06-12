use crate::app::{App, AppEvent, Screen, DeleteTarget, ScanState, MultiScanState};
use crate::functions::*;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

impl App {
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
        pane_type: ActivePane,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        let pane = match pane_type {
            ActivePane::Left => &mut self.explorer_state.left_pane,
            ActivePane::Right => &mut self.explorer_state.right_pane,
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

                                    items.push(FileItem {
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
                                        FileItem {
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
            ServicesWizardState::GuiSelectPath {
                ref remote,
                ref current_path,
                ref mut loading,
                ..
            } => {
                *loading = true;
                (remote.clone(), current_path.clone())
            }
            ServicesWizardState::GuiSelectLocalPath {
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
                                            items.push(FileItem {
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
                                            FileItem {
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

    pub(crate) fn handle_explorer_list_result(
        &mut self,
        pane_type: ActivePane,
        result: Result<Vec<FileItem>, String>,
    ) {
        let pane = match pane_type {
            ActivePane::Left => &mut self.explorer_state.left_pane,
            ActivePane::Right => &mut self.explorer_state.right_pane,
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
        let cache_path = std::path::PathBuf::from(crate::functions::app_config::TuiCustomConfig::load().features_cache_file_path);
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
                self.explorer_state.popup = ExplorerPopup::MoveProgress {
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
                        Some(tx_move.clone()), None).await;
                    let _ = tx_move.send(AppEvent::ExplorerOperationFinished {
                        pane: ActivePane::Left,
                        op_name: "di chuyển (move)".to_string(),
                        result: res,
                    });
                });
            } else {
                let mut options = Vec::new();
                let mut actions = Vec::new();

                if dst_copy && src_purge {
                    options.push("Sử dụng Sao chép & Xóa (Copy & Delete) trên máy chủ".to_string());
                    actions.push(FallbackAction::MoveCopyDelete {
                        src: src.clone(),
                        dest: dest.clone(),
                    });
                }

                options.push("Tải về máy rồi Upload lên đích (Local Transfer - Rất chậm)".to_string());
                actions.push(FallbackAction::MoveLocalTransfer {
                    src: src.clone(),
                    dest: dest.clone(),
                });

                options.push("Hủy bỏ tác vụ".to_string());
                actions.push(FallbackAction::Cancel);

                self.explorer_state.popup = ExplorerPopup::ConfirmFallback {
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
                self.explorer_state.popup = ExplorerPopup::MoveProgress {
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
                        pane: ActivePane::Left,
                        op_name: "đổi tên (rename)".to_string(),
                        result: res,
                    });
                });
            } else {
                let mut options = Vec::new();
                let mut actions = Vec::new();

                if dst_copy && src_purge {
                    options.push("Sử dụng Sao chép & Xóa (Copy & Delete) trên máy chủ".to_string());
                    actions.push(FallbackAction::RenameCopyDelete {
                        src: src.clone(),
                        dest: dest.clone(),
                        is_dir,
                    });
                }

                options.push("Tải về máy rồi Upload lên đích (Local Transfer - Rất chậm)".to_string());
                actions.push(FallbackAction::RenameLocalTransfer {
                    src: src.clone(),
                    dest: dest.clone(),
                    is_dir,
                });

                options.push("Hủy bỏ tác vụ".to_string());
                actions.push(FallbackAction::Cancel);

                self.explorer_state.popup = ExplorerPopup::ConfirmFallback {
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
                self.explorer_state.popup = ExplorerPopup::PermissionScanning {
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
                tokio::spawn(async move {
                    let mut restricted_files = Vec::new();

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
                        }));
                        let notify = Arc::new(Notify::new());
                        let max_concurrency = 8;

                        loop {
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
                        restricted_files = final_state.restricted_files.clone();
                    }

                    if restricted_files.is_empty() {
                        let _ = tx_check.send(AppEvent::PermissionCheckPassed {
                            src: src_clone,
                            dest: dest_clone,
                            is_dir: is_dir_clone,
                            use_checksum,
                        });
                    } else {
                        let _ = tx_check.send(AppEvent::PermissionErrorDetected {
                            src: src_clone,
                            dest: dest_clone,
                            is_dir: is_dir_clone,
                            restricted_files,
                            use_checksum,
                        });
                    }
                });
            } else {
                let mut options = Vec::new();
                let mut actions = Vec::new();

                options.push("Tải về máy rồi Upload lên đích (Local Transfer - Rất chậm)".to_string());
                actions.push(FallbackAction::CopyLocalTransfer {
                    src: src.clone(),
                    dest: dest.clone(),
                    use_checksum,
                });

                options.push("Hủy bỏ tác vụ".to_string());
                actions.push(FallbackAction::Cancel);

                self.explorer_state.popup = ExplorerPopup::ConfirmFallback {
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
        action: FallbackAction,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        match action {
            FallbackAction::MoveNative { src, dest }
            | FallbackAction::MoveLocalTransfer { src, dest } => {
                self.explorer_state.popup = ExplorerPopup::MoveProgress {
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
                        Some(tx_move.clone()), None).await;
                    let _ = tx_move.send(AppEvent::ExplorerOperationFinished {
                        pane: ActivePane::Left,
                        op_name: "di chuyển (move)".to_string(),
                        result: res,
                    });
                });
            }
            FallbackAction::MoveCopyDelete { src, dest } => {
                self.explorer_state.popup = ExplorerPopup::MoveProgress {
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
                            Some(tx_move.clone()), None).await
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
                                pane: ActivePane::Left,
                                op_name: "Move Copy-Delete (Purge)".to_string(),
                                result: outcome,
                            });
                        }
                        Err(e) => {
                            let _ = tx_move.send(AppEvent::ExplorerOperationFinished {
                                pane: ActivePane::Left,
                                op_name: "Move Copy-Delete (Copy failed)".to_string(),
                                result: Err(e),
                            });
                        }
                    }
                });
            }
            FallbackAction::CopyNative { src, dest, use_checksum }
            | FallbackAction::CopyLocalTransfer { src, dest, use_checksum } => {
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
                        Some(tx_copy.clone()), None).await;
                    let _ = tx_copy.send(AppEvent::ExplorerOperationFinished {
                        pane: ActivePane::Left,
                        op_name: "sao chép (copy)".to_string(),
                        result: res,
                    });
                });
            }
            FallbackAction::DeleteNative { target, is_dir } => {
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
            FallbackAction::DeleteIndividual { target } => {
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
            FallbackAction::RenameCopyDelete { src, dest, is_dir } => {
                self.explorer_state.popup = ExplorerPopup::MoveProgress {
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
                                Some(tx_move.clone()), None).await
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
                                pane: ActivePane::Left,
                                op_name: "đổi tên dự phòng (copy+delete)".to_string(),
                                result: del_res,
                            });
                        }
                        Err(e) => {
                            let _ = tx_move.send(AppEvent::ExplorerOperationFinished {
                                pane: ActivePane::Left,
                                op_name: "đổi tên dự phòng (copy+delete)".to_string(),
                                result: Err(e),
                            });
                        }
                    }
                });
            }
            FallbackAction::RenameLocalTransfer { src, dest, is_dir } => {
                self.explorer_state.popup = ExplorerPopup::MoveProgress {
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
                                Some(tx_move.clone()), None).await;
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
                        pane: ActivePane::Left,
                        op_name: "đổi tên dự phòng (local transfer)".to_string(),
                        result: res,
                    });
                });
            }
            FallbackAction::CleanupCloud { fs } => {
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
            FallbackAction::Rmdir { fs, remote } => {
                let tx_clone = tx.clone();
                tokio::spawn(async move {
                    let res = run_rpc_job_async("operations/rmdir".to_string(), json!({ "fs": fs, "remote": remote })).await;
                    let msg = match &res {
                        Ok(_) => "Xóa thư mục rỗng thành công!".to_string(),
                        Err(e) => format!("Lỗi khi xóa: {}", e),
                    };
                    let _ = tx_clone.send(AppEvent::ExplorerOperationFinished {
                        pane: ActivePane::Left,
                        op_name: "xóa thư mục rỗng (rmdir)".to_string(),
                        result: res.clone(),
                    });
                    let _ = tx_clone.send(AppEvent::CryptdecodeResult { result: Ok(msg) });
                });
            }
            FallbackAction::Rmdirs { fs, remote } => {
                let tx_clone = tx.clone();
                tokio::spawn(async move {
                    let res = run_rpc_job_async("operations/rmdirs".to_string(), json!({ "fs": fs, "remote": remote })).await;
                    let msg = match &res {
                        Ok(_) => "Xóa đệ quy các thư mục rỗng thành công!".to_string(),
                        Err(e) => format!("Lỗi khi xóa: {}", e),
                    };
                    let _ = tx_clone.send(AppEvent::ExplorerOperationFinished {
                        pane: ActivePane::Left,
                        op_name: "xóa đệ quy thư mục rỗng (rmdirs)".to_string(),
                        result: res.clone(),
                    });
                    let _ = tx_clone.send(AppEvent::CryptdecodeResult { result: Ok(msg) });
                });
            }
            FallbackAction::PermissionCancel => {}
            FallbackAction::PermissionCopyAsMuchAsPossible { src, dest, is_dir, restricted_files: _, use_checksum } => {
                self.explorer_state.popup = ExplorerPopup::CopyProgress {
                    src: src.clone(),
                    dest: dest.clone(),
                    pct: 0.0,
                    job_id: None,
                };
                let tx_copy = tx.clone();
                let src_clone = src.clone();
                let dest_clone = dest.clone();
                let op_id = format!("copy_as_much_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                let op = ActiveOperation {
                    id: op_id.clone(),
                    action_type: "copy".to_string(),
                    src: src_clone.clone(),
                    dest: dest_clone.clone(),
                    items: vec![src_clone.clone()],
                    is_dir,
                    use_checksum,
                    is_copy: true,
                    completed_items: Some(Vec::new()),
                    tasks: Some(Vec::new()),
                    transfers: None,
                    checkers: None,
                };
                crate::app::save_active_operation(&op);

                let skip_flag = self.skip_permission_precheck.clone();
                tokio::spawn(async move {
                    crate::app::start_async_checker_and_transfer::start_async_checker_and_transfer(
                        op_id,
                        src_clone,
                        dest_clone,
                        is_dir,
                        use_checksum,
                        true,
                        None,
                        skip_flag,
                        tx_copy,
                    ).await;
                });
            }
            FallbackAction::PermissionRestrictedCopy { src, dest, is_dir, restricted_files: _, use_checksum } => {
                self.explorer_state.popup = ExplorerPopup::CopyProgress {
                    src: src.clone(),
                    dest: dest.clone(),
                    pct: 0.0,
                    job_id: None,
                };
                let tx_copy = tx.clone();
                let src_clone = src.clone();
                let dest_clone = dest.clone();
                let op_id = format!("restr_copy_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                let op = ActiveOperation {
                    id: op_id.clone(),
                    action_type: "copy".to_string(),
                    src: src_clone.clone(),
                    dest: dest_clone.clone(),
                    items: vec![src_clone.clone()],
                    is_dir,
                    use_checksum,
                    is_copy: true,
                    completed_items: Some(Vec::new()),
                    tasks: Some(Vec::new()),
                    transfers: None,
                    checkers: None,
                };
                crate::app::save_active_operation(&op);

                let skip_flag = self.skip_permission_precheck.clone();
                tokio::spawn(async move {
                    crate::app::start_async_checker_and_transfer::start_async_checker_and_transfer(
                        op_id,
                        src_clone,
                        dest_clone,
                        is_dir,
                        use_checksum,
                        true,
                        None,
                        skip_flag,
                        tx_copy,
                    ).await;
                });
            }
            FallbackAction::MultiPermissionCopyAsMuchAsPossible { items, dest_remote, dest_path, restricted_files: _, use_checksum } => {
                let dest_full = if dest_remote.is_empty() { dest_path.clone() } else { format!("{}:{}", dest_remote, dest_path) };
                self.explorer_state.popup = ExplorerPopup::CopyProgress {
                    src: format!("({} mục)", items.len()),
                    dest: dest_full.clone(),
                    pct: 0.0,
                    job_id: None,
                };
                let tx_op = tx.clone();
                let dest_remote_clone = dest_remote.clone();
                let dest_path_clone = dest_path.clone();
                let items_clone = items.clone();
                let op_id = format!("multi_copy_as_much_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                
                let src_base = if items_clone.is_empty() {
                    String::new()
                } else {
                    let item = &items_clone[0];
                    if item.remote.is_empty() {
                        item.path.clone()
                    } else {
                        format!("{}:{}", item.remote.trim_end_matches(':'), item.path)
                    }
                };
                let op = ActiveOperation {
                    id: op_id.clone(),
                    action_type: "copy".to_string(),
                    src: src_base.clone(),
                    dest: dest_full.clone(),
                    items: items_clone.iter().map(|item| item.name.clone()).collect(),
                    is_dir: true,
                    use_checksum,
                    is_copy: true,
                    completed_items: Some(Vec::new()),
                    tasks: Some(Vec::new()),
                    transfers: None,
                    checkers: None,
                };
                crate::app::save_active_operation(&op);

                let skip_flag = self.skip_permission_precheck.clone();
                tokio::spawn(async move {
                    crate::app::start_async_checker_and_transfer::start_async_checker_and_transfer(
                        op_id,
                        src_base,
                        dest_full,
                        true,
                        use_checksum,
                        true,
                        Some(items_clone),
                        skip_flag,
                        tx_op,
                    ).await;
                });
            }
            FallbackAction::MultiPermissionRestrictedCopy { items, dest_remote, dest_path, restricted_files: _, use_checksum } => {
                let dest_full = if dest_remote.is_empty() { dest_path.clone() } else { format!("{}:{}", dest_remote, dest_path) };
                self.explorer_state.popup = ExplorerPopup::CopyProgress {
                    src: format!("({} mục)", items.len()),
                    dest: dest_full.clone(),
                    pct: 0.0,
                    job_id: None,
                };
                let tx_op = tx.clone();
                let dest_remote_clone = dest_remote.clone();
                let dest_path_clone = dest_path.clone();
                let items_clone = items.clone();
                let op_id = format!("multi_restr_copy_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                
                let src_base = if items_clone.is_empty() {
                    String::new()
                } else {
                    let item = &items_clone[0];
                    if item.remote.is_empty() {
                        item.path.clone()
                    } else {
                        format!("{}:{}", item.remote.trim_end_matches(':'), item.path)
                    }
                };
                let op = ActiveOperation {
                    id: op_id.clone(),
                    action_type: "copy".to_string(),
                    src: src_base.clone(),
                    dest: dest_full.clone(),
                    items: items_clone.iter().map(|item| item.name.clone()).collect(),
                    is_dir: true,
                    use_checksum,
                    is_copy: true,
                    completed_items: Some(Vec::new()),
                    tasks: Some(Vec::new()),
                    transfers: None,
                    checkers: None,
                };
                crate::app::save_active_operation(&op);

                let skip_flag = self.skip_permission_precheck.clone();
                tokio::spawn(async move {
                    crate::app::start_async_checker_and_transfer::start_async_checker_and_transfer(
                        op_id,
                        src_base,
                        dest_full,
                        true,
                        use_checksum,
                        true,
                        Some(items_clone),
                        skip_flag,
                        tx_op,
                    ).await;
                });
            }
            FallbackAction::Cancel => {}
        }
    }

}
