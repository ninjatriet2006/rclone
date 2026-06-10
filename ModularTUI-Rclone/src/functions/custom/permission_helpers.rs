use crate::app::AppEvent;
use crate::functions::*;

pub async fn execute_restricted_copy(
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

        let res = rpc_async("operations/copyfile".to_string(), param_str).await;
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
        let _ = rpc_async("operations/mkdir".to_string(), mkdir_param).await;

        let list_param = serde_json::json!({
            "fs": src,
            "remote": "",
            "opt": {
                "recurse": true
            }
        }).to_string();

        let list_res = rpc_async("operations/list".to_string(), list_param).await?;
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
                let mkdir_res = rpc_async("operations/mkdir".to_string(), mkdir_param).await?;
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
            let mkdir_res = rpc_async("operations/mkdir".to_string(), mkdir_param).await?;
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

            let copy_res = rpc_async("operations/copyfile".to_string(), copy_param_str).await;
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
                            // Skip restricted
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
            let _ = rpc_async("operations/mkdir".to_string(), mkdir_param).await;
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

pub async fn create_all_source_directories(src: &str, dest: &str) -> Result<(), String> {
    let mkdir_res = rpc_async(
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

    let list_param = serde_json::json!({
        "fs": src,
        "remote": "",
        "opt": {
            "recurse": true
        }
    })
    .to_string();

    if let Ok(list_res) = rpc_async("operations/list".to_string(), list_param).await {
        if list_res.status == 200 {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&list_res.output) {
                if let Some(list_arr) = val.get("list").and_then(|l| l.as_array()) {
                    for item in list_arr {
                        let is_item_dir = item.get("IsDir").and_then(|d| d.as_bool()).unwrap_or(false);
                        if is_item_dir {
                            if let Some(path) = item.get("Path").and_then(|p| p.as_str()) {
                                if !path.is_empty() {
                                    let _ = rpc_async(
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
