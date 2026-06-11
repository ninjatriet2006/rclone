use crate::app::{App, AppEvent, Screen, DeleteTarget, ScanState, MultiScanState};
use crate::functions::*;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

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
        let cache_path = AppConfig::config_dir().join("features_cache.json");
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
                        
                        self.explorer_state.popup = ExplorerPopup::SpecialActionMessage {
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
                        self.explorer_state.popup = ExplorerPopup::ChecksumTypeSelect {
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
                self.explorer_state.popup = ExplorerPopup::ConfirmFallback {
                    title: "Xác nhận Cleanup".to_string(),
                    options: vec!["[Có] Thực hiện".to_string(), "[Không] Hủy bỏ".to_string()],
                    selected_idx: 1,
                    actions: vec![
                        FallbackAction::CleanupCloud { fs: fs_target },
                        FallbackAction::Cancel,
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
                    FallbackAction::Rmdir { fs: target_fs, remote: target_remote }
                } else {
                    FallbackAction::Rmdirs { fs: target_fs, remote: target_remote }
                };

                self.explorer_state.popup = ExplorerPopup::ConfirmFallback {
                    title: title.to_string(),
                    options: vec!["[Có] Thực hiện".to_string(), "[Không] Hủy bỏ".to_string()],
                    selected_idx: 1,
                    actions: vec![
                        action,
                        FallbackAction::Cancel,
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
                self.explorer_state.popup = ExplorerPopup::CryptdecodeForm {
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
                        self.explorer_state.popup = ExplorerPopup::DecompressModeSelect {
                            archive_path: full_path,
                            selected_idx: 0,
                        };
                    }
                }
            }
            7 => {
                self.explorer_state.popup = ExplorerPopup::DedupeModeSelect {
                    by_hash: false,
                    selected_idx: 0,
                };
            }
            8 => {
                let selected_dirs: Vec<FileItem> = active_pane.items.iter()
                    .filter(|item| item.is_dir && item.name != ".." && active_pane.selected_names.contains(&item.name))
                    .cloned()
                    .collect();

                if selected_dirs.len() >= 2 {
                    self.explorer_state.popup = ExplorerPopup::MergeSimilarDestinationSelect {
                        folders: selected_dirs,
                        selected_idx: 0,
                    };
                } else {
                    let mut groups: std::collections::HashMap<String, Vec<FileItem>> = std::collections::HashMap::new();
                    for item in &active_pane.items {
                        if item.is_dir && item.name != ".." {
                            let normalized = item.name.trim().to_lowercase();
                            groups.entry(normalized).or_default().push(item.clone());
                        }
                    }
                    
                    let merge_groups: Vec<(String, Vec<FileItem>)> = groups
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
                            translate("exp_no_similar_dirs"),
                        ));
                    } else {
                        // Take the first found group
                        let folders = merge_groups[0].1.clone();
                        self.explorer_state.popup = ExplorerPopup::MergeSimilarDestinationSelect {
                            folders,
                            selected_idx: 0,
                        };
                    }
                }
            }
            9 => {
                let pane_type = self.explorer_state.active_pane.clone();
                {
                    let pane = match pane_type {
                        ActivePane::Left => &mut self.explorer_state.left_pane,
                        ActivePane::Right => &mut self.explorer_state.right_pane,
                    };
                    pane.remote = format!("{},trashed_only=true:", pane.remote.trim_end_matches(':'));
                    pane.path = String::new();
                    pane.selected_names.clear();
                    pane.selected_idx = 0;
                    pane.scroll_offset = 0;
                }
                self.refresh_explorer_pane(pane_type, tx.clone()).await;
            }
            11 => {
                let mut selected_items = Vec::new();
                let pane = match self.explorer_state.active_pane {
                    ActivePane::Left => &self.explorer_state.left_pane,
                    ActivePane::Right => &self.explorer_state.right_pane,
                };
                for item in &pane.items {
                    if item.name != ".." && pane.selected_names.contains(&item.name) {
                        let rel_path = if pane.path.is_empty() {
                            item.name.clone()
                        } else {
                            format!("{}/{}", pane.path.trim_start_matches('/'), item.name)
                        };
                        selected_items.push(rel_path);
                    }
                }
                if selected_items.is_empty() {
                    if let Some(item) = selected_item {
                        if item.name != ".." {
                            let rel_path = if pane.path.is_empty() {
                                item.name.clone()
                            } else {
                                format!("{}/{}", pane.path.trim_start_matches('/'), item.name)
                            };
                            selected_items.push(rel_path);
                        }
                    }
                }
                
                if !selected_items.is_empty() {
                    let base_remote = if let Some(idx) = pane.remote.find(",trashed_only=true") {
                        format!("{}:", &pane.remote[..idx])
                    } else {
                        pane.remote.clone()
                    };
                    
                    self.explorer_state.popup = ExplorerPopup::SpecialActionMessage {
                        title: "Khôi phục mục đã chọn".to_string(),
                        message: "Đang khôi phục...".to_string(),
                    };
                    
                    let tx_clone = tx.clone();
                    let pane_type = self.explorer_state.active_pane.clone();
                    tokio::spawn(async move {
                        let param = json!({
                            "command": "untrash",
                            "fs": base_remote,
                            "arg": selected_items,
                        }).to_string();
                        let res = rclone::rpc_async("backend/command".to_string(), param).await;
                        let result = match res {
                            Ok(rpc_res) if rpc_res.status == 200 => Ok(()),
                            Ok(rpc_res) => Err(format!("Lỗi RPC: {}", rpc_res.output)),
                            Err(e) => Err(e),
                        };
                        let _ = tx_clone.send(AppEvent::ExplorerOperationFinished {
                            pane: pane_type,
                            op_name: "Khôi phục từ thùng rác (untrash)".to_string(),
                            result,
                        });
                    });
                }
            }
            12 => {
                let pane_type = self.explorer_state.active_pane.clone();
                {
                    let pane = match pane_type {
                        ActivePane::Left => &mut self.explorer_state.left_pane,
                        ActivePane::Right => &mut self.explorer_state.right_pane,
                    };
                    if let Some(idx) = pane.remote.find(",trashed_only=true") {
                        pane.remote = format!("{}:", &pane.remote[..idx]);
                    }
                    pane.path = String::new();
                    pane.selected_names.clear();
                    pane.selected_idx = 0;
                    pane.scroll_offset = 0;
                }
                self.refresh_explorer_pane(pane_type, tx.clone()).await;
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
        
        self.explorer_state.popup = ExplorerPopup::SpecialActionMessage {
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
                self.explorer_state.popup = ExplorerPopup::None;
                self.execute_archive_decompress(archive_path, parent_fs, tx.clone()).await;
            }
            1 => {
                self.explorer_state.popup = ExplorerPopup::None;
                let folder_name = strip_archive_extensions(&archive_name);
                let dest_fs = if is_remote_empty {
                    PathBuf::from(&parent_fs).join(&folder_name).to_string_lossy().to_string()
                } else {
                    format!("{}/{}", parent_fs.trim_end_matches('/'), folder_name)
                };
                self.execute_archive_decompress(archive_path, dest_fs, tx.clone()).await;
            }
            2 => {
                self.explorer_state.popup = ExplorerPopup::DecompressPathInput {
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
        
        self.explorer_state.popup = ExplorerPopup::CopyProgress {
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
                Some(tx_clone.clone()), None).await;
            
            let _ = tx_clone.send(AppEvent::ExplorerOperationFinished {
                pane: ActivePane::Left,
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

        self.explorer_state.popup = ExplorerPopup::CopyProgress {
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
                Some(tx_clone.clone()), None).await;

            let _ = tx_clone.send(AppEvent::ExplorerOperationFinished {
                pane: ActivePane::Left,
                op_name: format!("Lọc trùng (dedupe: {})", mode_clone),
                result: res,
            });
        });
    }

    pub(crate) async fn execute_merge_similar(
        &mut self,
        folders: Vec<FileItem>,
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

        self.explorer_state.popup = ExplorerPopup::CopyProgress {
            src: "Đang gộp thư mục tương tự...".to_string(),
            dest: String::new(),
            pct: 0.0,
            job_id: None,
        };

        let tx_clone = tx.clone();
        tokio::spawn(async move {
            let mut errors = Vec::new();
            let dest_folder = &folders[destination_idx];

            log_info(&format!(
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
                        
                        log_info(&format!(
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
                            log_info(&format!("[Google Drive ID Flow] {}", err_msg));
                            errors.push(err_msg);
                        } else {
                            log_info(&format!(
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
                            log_info(&format!(
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
                                                    log_info(&format!(
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
                                log_info(&format!("[Google Drive ID Flow] {}", warn_msg));
                                errors.push(warn_msg);
                            } else {
                                log_info(&format!(
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
                                    log_info(&format!("[Google Drive ID Flow] {}", err_msg));
                                    errors.push(err_msg);
                                } else {
                                    log_info(&format!(
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
                        pane: ActivePane::Left,
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

                    log_info(&format!(
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
                    log_info(&format!(
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
                                            log_info(&format!(
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
                        log_info(&format!("[Fallback Flow] {}", warn_msg));
                        errors.push(warn_msg);
                    } else {
                        log_info(&format!(
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
                            log_info(&format!("[Fallback Flow] {}", err_msg));
                            errors.push(err_msg);
                        } else {
                            log_info(&format!(
                                "[Fallback Flow] Đã xóa thành công thư mục nguồn '{}' (parent_path: '{}')",
                                folder.name, parent_path
                            ));
                        }
                    }
                }
            }

            let final_res = if errors.is_empty() {
                log_info("[execute_merge_similar] Hoàn tất gộp thành công không có lỗi.");
                Ok(())
            } else {
                let errs = errors.join("\n");
                log_info(&format!("[execute_merge_similar] Hoàn tất gộp có lỗi: {}", errs));
                Err(errs)
            };

            let _ = tx_clone.send(AppEvent::ExplorerOperationFinished {
                pane: ActivePane::Left,
                op_name: "Gộp thư mục".to_string(),
                result: final_res,
            });
        });
    }

    pub(crate) async fn execute_merge_similar_scan(
        &mut self,
        folders: Vec<FileItem>,
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

            let mut tree_root = TreeNode::new(dest_folder.name.clone(), "".to_string(), true);

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
                let size_str = format_size(size);

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

    pub fn scan_running_services(&mut self) {
        let active = crate::functions::scan_running_services(&self.config.profiles, &self.config.active_profile);
        self.services_state.active_services = active;
    }

    /// Quét các dịch vụ systemd (rclone) cấp hệ thống và cấp cá nhân
    #[cfg(all(unix, not(target_os = "macos")))]
        pub fn scan_systemd_services(&mut self) {
        let systemd = crate::functions::scan_systemd_services();
        self.services_state.systemd_services = systemd;
    }

pub(crate) fn parse_exec_start_full(
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

    pub(crate) fn parse_systemd_file(&self, file_path: &str) -> std::io::Result<Vec<(String, String)>> {
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

    pub(crate) fn load_systemd_service_fields(
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

    pub(crate) fn init_create_systemd_service_fields(&self) -> Vec<(String, String, String, Vec<String>)> {
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

    pub(crate) fn save_systemd_service_file(
        &mut self,
        _is_create: bool,
        _service_name: &str,
        file_path: &str,
        is_user: bool,
        fields: &[(String, String, String, Vec<String>)],
    ) -> std::io::Result<()> {
        let mut final_fields = fields.to_vec();

        let active_tab = match &self.services_state.wizard {
            ServicesWizardState::EditSystemdService { active_tab, .. } => *active_tab,
            ServicesWizardState::CreateSystemdService { active_tab, .. } => *active_tab,
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
                        .args([
                            "sh",
                            "-c",
                            "mkdir -p \"$1\" && chown -R \"$2\" \"$1\"",
                            "_",
                            &mount_path,
                            &owner_arg,
                        ])
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

        let home_dir = get_home_dir();
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
            let status = Command::new("pkexec")
                .args([
                    "sh",
                    "-c",
                    "mkdir -p \"$1\" && mv \"$2\" \"$3\" && chown root:root \"$3\" && chmod 644 \"$3\" && systemctl daemon-reload",
                    "_",
                    &parent_dir,
                    &temp_file_path,
                    file_path,
                ])
                .status()?;

            if !status.success() {
                return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "pkexec commands failed"));
            }
        }

        Ok(())
    }

    pub(crate) fn ensure_mount_point_exists_from_service_file(&self, file_path: &str) {
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
                                        .args([
                                            "sh",
                                            "-c",
                                            "mkdir -p \"$1\" && chown -R \"$2\" \"$1\"",
                                            "_",
                                            mount_path,
                                            &owner_arg,
                                        ])
                                        .status();
                                }
                            }
                        }
                    }
                }
            }
        }

    pub(crate) fn get_systemd_error_logs(&self, service_name: &str, is_user: bool) -> String {
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

    pub(crate) fn execute_launch_service(
        &mut self,
        service_type: ServiceType,
        remote: String,
        path: String,
        protocol: Option<String>,
        flags: Vec<(String, String, String, String)>,
    ) {
        self.services_state.wizard = ServicesWizardState::None;

        let config_path = self.config.get_active_profile_path();

        let mut args = vec![];

        match service_type {
            ServiceType::Mount | ServiceType::NfsMount => {
                let cmd_name = if service_type == ServiceType::NfsMount {
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
                        .args([
                            "sh",
                            "-c",
                            "mkdir -p \"$1\" && chown -R \"$2\" \"$1\"",
                            "_",
                            &local_mnt,
                            &owner_arg,
                        ])
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
                            .push(ActiveService {
                                service_type_str: if service_type == ServiceType::NfsMount {
                                    "NfsMount".to_string()
                                } else {
                                    "Mount".to_string()
                                },
                                remote,
                                path: local_mnt.clone(),
                                pid,
                                details: format!("{} -> {}", if service_type == ServiceType::NfsMount { "NfsMount" } else { "Mount" }, local_mnt),
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
            ServiceType::WebGui => {
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
                            .push(ActiveService {
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
            ServiceType::Serve => {
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
                            .push(ActiveService {
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



pub(crate) fn execute_import_profile(&mut self, name: String, src: String, import_type: usize) {
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
                    self.profile_state.wizard = ImportWizardState::None;
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
                self.profile_state.wizard = ImportWizardState::None;
            } else {
                self.profile_state.error_message =
                    Some("Tải cấu hình từ URL thất bại.".to_string());
            }
        }
    }
}
