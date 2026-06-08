use crate::app::{App, AppEvent};
use crate::functions::*;
use serde_json::json;
use std::path::PathBuf;

pub async fn execute_fallback_action(
    app: &mut App,
    action: FallbackAction,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
) {
    match action {
        FallbackAction::MoveNative { src, dest }
        | FallbackAction::MoveLocalTransfer { src, dest } => {
            app.explorer_state.popup = ExplorerPopup::MoveProgress {
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
            app.explorer_state.popup = ExplorerPopup::MoveProgress {
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
            app.explorer_state.popup = ExplorerPopup::CopyProgress {
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
            let pane_type = app.explorer_state.active_pane.clone();
            let tx_del = tx.clone();
            let op_id = format!("del_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
            let op = ActiveOperation {
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
        FallbackAction::DeleteIndividual { target } => {
            let pane_type = app.explorer_state.active_pane.clone();
            let tx_del = tx.clone();
            let op_id = format!("del_indiv_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
            let op = ActiveOperation {
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
        FallbackAction::RenameCopyDelete { src, dest, is_dir } => {
            app.explorer_state.popup = ExplorerPopup::MoveProgress {
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
            app.explorer_state.popup = ExplorerPopup::MoveProgress {
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
            app.explorer_state.popup = ExplorerPopup::CopyProgress {
                src: src.clone(),
                dest: dest.clone(),
                pct: 0.0,
                job_id: None,
            };
            let tx_copy = tx.clone();
            let src_clone = src.clone();
            let dest_clone = dest.clone();
            tokio::spawn(async move {
                if is_dir {
                    let _ = create_all_source_directories(&src_clone, &dest_clone).await;
                }
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
                    pane: ActivePane::Left,
                    op_name: "sao chép nhiều nhất có thể (copy)".to_string(),
                    result,
                });
            });
        }
        FallbackAction::PermissionRestrictedCopy { src, dest, is_dir, restricted_files: _, use_checksum } => {
            app.explorer_state.popup = ExplorerPopup::CopyProgress {
                src: src.clone(),
                dest: dest.clone(),
                pct: 0.0,
                job_id: None,
            };
            let tx_copy = tx.clone();
            let src_clone = src.clone();
            let dest_clone = dest.clone();
            tokio::spawn(async move {
                let res = execute_restricted_copy(src_clone, dest_clone, is_dir, use_checksum, tx_copy.clone()).await;
                let _ = tx_copy.send(AppEvent::ExplorerOperationFinished {
                    pane: ActivePane::Left,
                    op_name: "sao chép hạn chế (restricted copy)".to_string(),
                    result: res,
                });
            });
        }
        FallbackAction::MultiPermissionCopyAsMuchAsPossible { items, dest_remote, dest_path, restricted_files: _, use_checksum } => {
            let dest_full = if dest_remote.is_empty() { dest_path.clone() } else { format!("{}:{}", dest_remote, dest_path) };
            app.explorer_state.popup = ExplorerPopup::CopyProgress {
                src: format!("({} mục)", items.len()),
                dest: dest_full.clone(),
                pct: 0.0,
                job_id: None,
            };
            let tx_op = tx.clone();
            let dest_remote_clone = dest_remote.clone();
            let dest_path_clone = dest_path.clone();
            let items_clone = items.clone();
            let pane_type = app.explorer_state.active_pane.clone();
            let op_id = format!("multi_copy_as_much_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
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
                    match &res {
                        Ok(_) => {
                            crate::app::complete_item_in_active_operation(&op_id, &clip_item.name);
                        }
                        Err(e) => {
                            last_err = Some(e.clone());
                        }
                    }
                    if let Err(e) = res {
                        let err_lower = e.to_lowercase();
                        if err_lower.contains("restrictedlink")
                            || err_lower.contains("download")
                            || err_lower.contains("forbidden")
                            || err_lower.contains("only the owner")
                        {
                            // Skip
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
                crate::app::remove_active_operation(&op_id);
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
        FallbackAction::MultiPermissionRestrictedCopy { items, dest_remote, dest_path, restricted_files: _, use_checksum } => {
            let dest_full = if dest_remote.is_empty() { dest_path.clone() } else { format!("{}:{}", dest_remote, dest_path) };
            app.explorer_state.popup = ExplorerPopup::CopyProgress {
                src: format!("({} mục)", items.len()),
                dest: dest_full.clone(),
                pct: 0.0,
                job_id: None,
            };
            let tx_op = tx.clone();
            let dest_remote_clone = dest_remote.clone();
            let dest_path_clone = dest_path.clone();
            let items_clone = items.clone();
            let pane_type = app.explorer_state.active_pane.clone();
            let op_id = format!("multi_restr_copy_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
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
                    match &res {
                        Ok(_) => {
                            crate::app::complete_item_in_active_operation(&op_id, &clip_item.name);
                        }
                        Err(e) => {
                            last_err = Some(e.clone());
                        }
                    }
                }
                let _ = tx_op.send(AppEvent::CopyProgress {
                    src: format!("({} mục)", total),
                    dest: String::new(),
                    pct: 100.0,
                    job_id: None,
                });
                crate::app::remove_active_operation(&op_id);
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
        FallbackAction::Cancel => {}
    }
}
