use crate::app::{App, AppEvent, Screen, DeleteTarget, ScanState, MultiScanState};
use crate::functions::*;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

impl App {
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


}
