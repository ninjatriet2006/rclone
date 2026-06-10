use crate::app::AppEvent;
use crate::functions::*;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::path::PathBuf;

pub async fn start_async_checker_and_transfer(
    op_id: String,
    src: String,
    dest: String,
    is_dir: bool,
    use_checksum: bool,
    is_copy: bool,
    items: Option<Vec<ClipboardItem>>,
    skip_flag: Arc<AtomicBool>,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
) {
    let op_id_clone = op_id.clone();
    let src_clone = src.clone();
    let dest_clone = dest.clone();
    let tx_clone = tx.clone();
    let config_path = crate::functions::AppConfig::load().get_active_profile_path();

    let checker_running = Arc::new(AtomicBool::new(true));
    let checker_running_clone = checker_running.clone();

    let items_to_save = items.clone();

    // 1. Spawning Checker Task
    tokio::spawn(async move {
        // Initialize PreOperation
        let pre_op = PreOperation {
            id: op_id_clone.clone(),
            action_type: if is_copy { "copy".to_string() } else { "move".to_string() },
            src: src_clone.clone(),
            dest: dest_clone.clone(),
            is_dir,
            use_checksum,
            items: items_to_save,
            scanned_count: 0,
            total_files: 0,
            restricted_count: 0,
            status: "scanning".to_string(),
        };
        crate::app::save_pre_operation(&pre_op);

        let mut dest_files = std::collections::HashMap::new();
        let list_dest = is_dir || items.is_some();
        if list_dest {
            let list_param = json!({
                "fs": dest_clone,
                "remote": "",
                "opt": { "recurse": true }
            }).to_string();
            if let Ok(res) = rpc_async("operations/list".to_string(), list_param).await {
                let _ = check_and_apply_rate_limiting(&res).await;
                if res.status == 200 {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&res.output) {
                        if let Some(arr) = val.get("list").and_then(|l| l.as_array()) {
                            for item in arr {
                                if let Some(path) = item.get("Path").and_then(|p| p.as_str()) {
                                    let size = item.get("Size").and_then(|s| s.as_u64()).unwrap_or(0);
                                    let mod_time = item.get("ModTime").and_then(|m| m.as_str()).unwrap_or("").to_string();
                                    dest_files.insert(path.to_string(), (size, mod_time));
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut scanned_count = 0;
        let mut restricted_count = 0;

        let mut dirs_to_walk = Vec::new();
        let mut files_to_check = Vec::new();

        if let Some(ref list) = items {
            for clip_item in list {
                if clip_item.is_dir {
                    dirs_to_walk.push(clip_item.name.clone());
                } else {
                    files_to_check.push(clip_item.name.clone());
                }
            }
        } else {
            if is_dir {
                dirs_to_walk.push("".to_string());
            } else {
                let (_, filename) = parse_parent_and_child(&src_clone);
                files_to_check.push(filename);
            }
        }

        let mut batch = Vec::new();

        // Check individual files
        for filename in files_to_check {
            let list_param = json!({
                "fs": src_clone,
                "remote": filename,
                "opt": { "recurse": false, "metadata": true }
            }).to_string();

            let mut is_restricted = false;
            let mut file_size = 0;

            if let Ok(res) = rpc_async("operations/list".to_string(), list_param).await {
                let _ = check_and_apply_rate_limiting(&res).await;
                if res.status == 200 {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&res.output) {
                        if let Some(arr) = val.get("list").and_then(|l| l.as_array()) {
                            for item in arr {
                                file_size = item.get("Size").and_then(|s| s.as_u64()).unwrap_or(0);
                                if !skip_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                    let is_rest = if let Some(meta) = item.get("Metadata") {
                                        meta.get("copy-requires-writer-permission").and_then(|v| v.as_str()) == Some("true")
                                    } else {
                                        false
                                    };
                                    let mime = item.get("MimeType").and_then(|m| m.as_str()).unwrap_or("");
                                    if is_rest || mime.contains("shortcut.dangling") {
                                        is_restricted = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let status = if is_restricted {
                restricted_count += 1;
                TaskStatus::Failed
            } else {
                let mut matches = false;
                if dest_files.contains_key(&filename) {
                    let (d_size, _) = dest_files.get(&filename).unwrap();
                    if *d_size == file_size {
                        matches = true;
                    }
                }
                if matches {
                    TaskStatus::Skipped
                } else {
                    TaskStatus::Pending
                }
            };

            batch.push(FileTask {
                name: filename.clone(),
                size: file_size,
                status,
                error: if is_restricted { Some("Restricted link/dangling file skipped".to_string()) } else { None },
            });
            scanned_count += 1;

            if batch.len() >= 20 {
                let _ = append_tasks_to_active_operation(&op_id_clone, &batch);
                batch.clear();
            }

            let _ = tx_clone.send(AppEvent::PermissionScanProgress {
                src: src_clone.clone(),
                dest: dest_clone.clone(),
                is_dir: list_dest,
                scanned_count,
                total_files: 0,
                restricted_count,
            });

            let mut pre_ops = crate::app::load_pre_operations();
            if let Some(pos) = pre_ops.iter().position(|o| o.id == op_id_clone) {
                pre_ops[pos].scanned_count = scanned_count;
                pre_ops[pos].restricted_count = restricted_count;
                if skip_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    pre_ops[pos].status = "bypassed".to_string();
                }
                crate::app::save_pre_operation(&pre_ops[pos]);
            }
        }

        // Walk directories
        let mut queue = dirs_to_walk;
        while !queue.is_empty() {
            let dir = queue.remove(0);
            let list_param = json!({
                "fs": src_clone,
                "remote": dir,
                "opt": { "recurse": false, "metadata": true }
            }).to_string();

            if let Ok(res) = rpc_async("operations/list".to_string(), list_param).await {
                let _ = check_and_apply_rate_limiting(&res).await;
                if res.status == 200 {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&res.output) {
                        if let Some(arr) = val.get("list").and_then(|l| l.as_array()) {
                            for item in arr {
                                let is_item_dir = item.get("IsDir").and_then(|d| d.as_bool()).unwrap_or(false);
                                let path = item.get("Path").and_then(|p| p.as_str()).unwrap_or("").to_string();

                                if is_item_dir {
                                    queue.push(path);
                                } else {
                                    scanned_count += 1;
                                    let size = item.get("Size").and_then(|s| s.as_u64()).unwrap_or(0);
                                    let mut is_restricted = false;
                                    
                                    if !skip_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                        let is_rest = if let Some(meta) = item.get("Metadata") {
                                            meta.get("copy-requires-writer-permission").and_then(|v| v.as_str()) == Some("true")
                                        } else {
                                            false
                                        };
                                        let mime = item.get("MimeType").and_then(|m| m.as_str()).unwrap_or("");
                                        is_restricted = is_rest || mime.contains("shortcut.dangling");
                                    }

                                    let status = if is_restricted {
                                        restricted_count += 1;
                                        TaskStatus::Failed
                                    } else {
                                        let mut matches = false;
                                        if dest_files.contains_key(&path) {
                                            let (d_size, _) = dest_files.get(&path).unwrap();
                                            if *d_size == size {
                                                matches = true;
                                            }
                                        }
                                        if matches {
                                            TaskStatus::Skipped
                                        } else {
                                            TaskStatus::Pending
                                        }
                                    };

                                    batch.push(FileTask {
                                        name: path,
                                        size: size,
                                        status,
                                        error: if is_restricted { Some("Restricted/dangling file skipped".to_string()) } else { None },
                                    });

                                    if batch.len() >= 20 {
                                        let _ = append_tasks_to_active_operation(&op_id_clone, &batch);
                                        batch.clear();
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let _ = tx_clone.send(AppEvent::PermissionScanProgress {
                src: src_clone.clone(),
                dest: dest_clone.clone(),
                is_dir: list_dest,
                scanned_count,
                total_files: 0,
                restricted_count,
            });

            let mut pre_ops = crate::app::load_pre_operations();
            if let Some(pos) = pre_ops.iter().position(|o| o.id == op_id_clone) {
                pre_ops[pos].scanned_count = scanned_count;
                pre_ops[pos].restricted_count = restricted_count;
                if skip_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    pre_ops[pos].status = "bypassed".to_string();
                }
                crate::app::save_pre_operation(&pre_ops[pos]);
            }
        }

        if !batch.is_empty() {
            let _ = append_tasks_to_active_operation(&op_id_clone, &batch);
        }

        crate::app::remove_pre_operation(&op_id_clone);
        checker_running_clone.store(false, std::sync::atomic::Ordering::Relaxed);
    });

    // 2. Spawning Transfer Task
    tokio::spawn(async move {
        let op_id_transfer = op_id.clone();
        let src_transfer = src.clone();
        let dest_transfer = dest.clone();
        let tx_transfer = tx.clone();
        let config_path_transfer = config_path.clone();

        loop {
            let ops = load_active_operations().unwrap_or_default();
            let op_opt = ops.into_iter().find(|o| o.id == op_id_transfer);

            if op_opt.is_none() {
                break;
            }

            let op = op_opt.unwrap();
            let mut pending_tasks = Vec::new();
            if let Some(ref tasks) = op.tasks {
                for task in tasks {
                    if task.status == TaskStatus::Pending {
                        pending_tasks.push(task.clone());
                    }
                }
            }

            if pending_tasks.is_empty() {
                if !checker_running.load(std::sync::atomic::Ordering::Relaxed) {
                    let _ = remove_active_operation(&op_id_transfer);
                    
                    let progress_event = if is_copy {
                        AppEvent::CopyProgress {
                            src: src_transfer.clone(),
                            dest: dest_transfer.clone(),
                            pct: 100.0,
                            job_id: None,
                        }
                    } else {
                        AppEvent::MoveProgress {
                            src: src_transfer.clone(),
                            dest: dest_transfer.clone(),
                            pct: 100.0,
                            job_id: None,
                        }
                    };
                    let _ = tx_transfer.send(progress_event);

                    let _ = tx_transfer.send(AppEvent::ExplorerOperationFinished {
                        pane: ActivePane::Left,
                        op_name: if is_copy { "sao chép (copy)".to_string() } else { "di chuyển (move)".to_string() },
                        result: Ok(()),
                    });
                    break;
                } else {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    continue;
                }
            }

            let batch: Vec<FileTask> = pending_tasks.into_iter().take(50).collect();
            let batch_names: Vec<String> = batch.iter().map(|t| t.name.clone()).collect();

            let _ = update_tasks_status_in_active_operation(&op_id_transfer, &batch_names, TaskStatus::Transferring, None);

            let temp_dir = std::env::temp_dir();
            let temp_file_path = temp_dir.join(format!(
                "rclone_batch_{}_{}.txt",
                op_id_transfer,
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()
            ));
            let file_content = batch_names.join("\n");
            let _ = std::fs::write(&temp_file_path, file_content);

            let mut cmd = tokio::process::Command::new("rclone");
            let cmd_name = if is_copy { "copy" } else { "move" };
            cmd.args(&[
                cmd_name,
                "--files-from",
                temp_file_path.to_string_lossy().as_ref(),
                &src_transfer,
                &dest_transfer,
                "--config",
                &config_path_transfer,
                "--transfers",
                "4",
                "--checkers",
                "4"
            ]);

            let run_res = cmd.spawn();
            if let Ok(mut child) = run_res {
                let status = child.wait().await;
                let success = status.is_ok() && status.unwrap().success();

                let new_status = if success {
                    TaskStatus::Completed
                } else {
                    TaskStatus::Failed
                };
                let err_msg = if success {
                    None
                } else {
                    Some("Transfer subprocess returned non-zero status".to_string())
                };

                let _ = update_tasks_status_in_active_operation(&op_id_transfer, &batch_names, new_status, err_msg);
            } else {
                let _ = update_tasks_status_in_active_operation(
                    &op_id_transfer,
                    &batch_names,
                    TaskStatus::Failed,
                    Some("Failed to spawn rclone CLI process".to_string()),
                );
            }

            let _ = std::fs::remove_file(&temp_file_path);

            let ops = load_active_operations().unwrap_or_default();
            if let Some(op) = ops.into_iter().find(|o| o.id == op_id_transfer) {
                if let Some(ref tasks) = op.tasks {
                    let total = tasks.len();
                    let completed = tasks.iter().filter(|t| t.status == TaskStatus::Completed || t.status == TaskStatus::Skipped).count();
                    let pct = if total > 0 {
                        (completed as f64 / total as f64) * 100.0
                    } else {
                        0.0
                    };
                    let progress_event = if is_copy {
                        AppEvent::CopyProgress {
                            src: src_transfer.clone(),
                            dest: dest_transfer.clone(),
                            pct,
                            job_id: None,
                        }
                    } else {
                        AppEvent::MoveProgress {
                            src: src_transfer.clone(),
                            dest: dest_transfer.clone(),
                            pct,
                            job_id: None,
                        }
                    };
                    let _ = tx_transfer.send(progress_event);
                }
            }
        }
    });
}
