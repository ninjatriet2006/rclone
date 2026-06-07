use crossterm::event::{KeyEvent, KeyCode};
use crate::app::{App, AppEvent, MultiScanState};
use crate::functions::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use serde_json::json;
use tokio::sync::Notify;

pub async fn handle_copy_mode_keys(
    app: &mut App,
    key: KeyEvent,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    src: String,
    dest: String,
    is_dir: bool,
    is_multi: bool,
    clipboard_items: Option<Vec<ClipboardItem>>,
    action_type: String,
    mut selected_idx: usize,
) {
    match key.code {
        KeyCode::Esc => {
            app.explorer_state.popup = ExplorerPopup::None;
        }
        KeyCode::Up => {
            selected_idx = if selected_idx == 0 { 1 } else { selected_idx - 1 };
            app.explorer_state.popup = ExplorerPopup::CopyModeSelect {
                src, dest, is_dir, is_multi, clipboard_items, action_type, selected_idx
            };
        }
        KeyCode::Down => {
            selected_idx = (selected_idx + 1) % 2;
            app.explorer_state.popup = ExplorerPopup::CopyModeSelect {
                src, dest, is_dir, is_multi, clipboard_items, action_type, selected_idx
            };
        }
        KeyCode::Enter => {
            let use_checksum = selected_idx == 1;
            app.explorer_state.popup = ExplorerPopup::None;
            if action_type == "sync" {
                tokio::spawn(async move {
                    let mut param = json!({
                        "srcFs": src,
                        "dstFs": dest,
                    });
                    if use_checksum {
                        if let Some(obj) = param.as_object_mut() {
                            obj.insert("_config".to_string(), json!({ "checksum": true }));
                        }
                    }
                    let _ = run_rpc_job_async("sync/sync".to_string(), param).await;
                });
            } else {
                if is_multi {
                    if let Some(items) = clipboard_items {
                        let dest_full = dest.clone();
                        app.explorer_state.popup = ExplorerPopup::PermissionScanning {
                            src: src.clone(),
                            dest: dest_full.clone(),
                            is_dir: true,
                            scanned_count: 0,
                            total_files: 0,
                            restricted_count: 0,
                        };

                        let tx_check = tx.clone();
                        let items_clone = items.clone();
                        let (dest_remote, dest_path) = if let Some(idx) = dest.find(':') {
                            (dest[..idx].to_string(), dest[idx+1..].to_string())
                        } else {
                            (String::new(), dest.clone())
                        };
                        let dest_remote_clone = dest_remote.clone();
                        let dest_path_clone = dest_path.clone();

                        tokio::spawn(async move {
                            let mut scanned_count = 0;
                            let mut restricted_files = Vec::new();

                            let mut dirs_to_walk = Vec::new();
                            let mut files_to_check = Vec::new();

                            for clip_item in &items_clone {
                                let clip_src = if clip_item.remote.is_empty() {
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

                                if clip_item.is_dir {
                                    dirs_to_walk.push(clip_src);
                                } else {
                                    files_to_check.push((clip_src, clip_item.name.clone()));
                                }
                            }

                            let files_state = Arc::new(Mutex::new(restricted_files));
                            let mut file_tasks = Vec::new();

                            for (src_file, _) in files_to_check {
                                let files_state_clone = Arc::clone(&files_state);

                                let task = tokio::spawn(async move {
                                    let (src_fs, fname) = parse_parent_and_child(&src_file);
                                    let list_param = json!({
                                        "fs": src_fs,
                                        "remote": fname,
                                        "opt": {
                                            "recurse": false,
                                            "metadata": true
                                        }
                                    }).to_string();

                                    let mut is_rest = false;
                                    if let Ok(res) = rpc_async("operations/list".to_string(), list_param).await {
                                        if res.status == 200 {
                                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&res.output) {
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
                                                            is_rest = true;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    if is_rest {
                                        let mut s = files_state_clone.lock().unwrap();
                                        s.push(src_file.clone());
                                    }
                                });
                                file_tasks.push(task);
                            }

                            for task in file_tasks {
                                let _ = task.await;
                            }

                            restricted_files = Arc::try_unwrap(files_state).unwrap().into_inner().unwrap();
                            scanned_count += items_clone.len() - dirs_to_walk.len();

                            let _ = tx_check.send(AppEvent::PermissionScanProgress {
                                src: src.clone(),
                                dest: dest_full.clone(),
                                is_dir: true,
                                scanned_count,
                                total_files: 0,
                                restricted_count: restricted_files.len(),
                            });

                            if !dirs_to_walk.is_empty() {
                                let mut initial_queue = Vec::new();
                                for dir_fs in dirs_to_walk {
                                    initial_queue.push((dir_fs, "".to_string()));
                                }

                                let state = Arc::new(Mutex::new(MultiScanState {
                                    queue: initial_queue,
                                    active_tasks: 0,
                                    files_count: 0,
                                    restricted: restricted_files,
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
                                        let (fs_root, dir_rel) = {
                                            let mut s = state.lock().unwrap();
                                            s.active_tasks += 1;
                                            s.queue.remove(0)
                                        };

                                        let state_clone = Arc::clone(&state);
                                        let notify_clone = Arc::clone(&notify);
                                        let tx_progress = tx_check.clone();
                                        let items_len = items_clone.len();
                                        let dest_r_c = dest_remote_clone.clone();
                                        let dest_p_c = dest_path_clone.clone();
                                        let scanned_base = scanned_count;

                                        tokio::spawn(async move {
                                            let list_param = json!({
                                                "fs": fs_root,
                                                "remote": dir_rel,
                                                "opt": {
                                                    "recurse": false,
                                                    "metadata": true
                                                }
                                            }).to_string();

                                            let mut new_dirs = Vec::new();
                                            let mut new_files_count = 0;
                                            let mut new_restricted = Vec::new();

                                            if let Ok(res) = rpc_async("operations/list".to_string(), list_param).await {
                                                if res.status == 200 {
                                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&res.output) {
                                                        if let Some(list_arr) = val.get("list").and_then(|l| l.as_array()) {
                                                            for item in list_arr {
                                                                let is_item_dir = item.get("IsDir").and_then(|d| d.as_bool()).unwrap_or(false);
                                                                let path = item.get("Path").and_then(|p| p.as_str()).unwrap_or("");

                                                                if is_item_dir {
                                                                    new_dirs.push((fs_root.clone(), path.to_string()));
                                                                } else {
                                                                    new_files_count += 1;
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
                                                                        let full_item_path = if dir_rel.is_empty() {
                                                                            format!("{}/{}", fs_root, path)
                                                                        } else {
                                                                            format!("{}/{}/{}", fs_root, dir_rel, path)
                                                                        };
                                                                        new_restricted.push(full_item_path);
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
                                                s.files_count += new_files_count;
                                                s.active_tasks -= 1;
                                                (s.files_count, s.restricted.clone())
                                            };

                                            let _ = tx_progress.send(AppEvent::PermissionScanProgress {
                                                src: format!("({} mục)", items_len),
                                                dest: if dest_r_c.is_empty() { dest_p_c.clone() } else { format!("{}:{}", dest_r_c, dest_p_c) },
                                                is_dir: true,
                                                scanned_count: scanned_base + current_scanned,
                                                total_files: 0,
                                                restricted_count: current_restricted.len(),
                                            });

                                            notify_clone.notify_one();
                                        });
                                    }
                                }

                                let final_state = state.lock().unwrap();
                                restricted_files = final_state.restricted.clone();
                            }

                            if restricted_files.is_empty() {
                                let _ = tx_check.send(AppEvent::MultiPermissionCheckPassed {
                                    items: items_clone,
                                    dest_remote: dest_remote_clone,
                                    dest_path: dest_path_clone,
                                    use_checksum,
                                });
                            } else {
                                let _ = tx_check.send(AppEvent::MultiPermissionErrorDetected {
                                    items: items_clone,
                                    dest_remote: dest_remote_clone,
                                    dest_path: dest_path_clone,
                                    restricted_files,
                                    use_checksum,
                                });
                            }
                        });
                    }
                } else {
                    app.check_features_and_execute("copy", src, dest, is_dir, use_checksum, tx.clone());
                }
            }
        }
        _ => {}
    }
}
