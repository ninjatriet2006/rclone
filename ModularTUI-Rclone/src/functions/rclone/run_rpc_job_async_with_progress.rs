use crate::functions::rclone::rpc_async::rpc_async;
use crate::functions::rclone::get_job_description::register_job_description;
use crate::functions::rclone::job_direction::{register_job_direction, register_job_real_size, JobDirection};
use crate::functions::rclone::thread_optimizer::inject_optimal_thread_config;
use crate::app::AppEvent;
use crate::functions::app_config::AppConfig;
use crate::functions::widgets::structs::ActiveOperation;
use serde_json::json;

pub async fn run_rpc_job_async_with_progress(
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
            let config = AppConfig::load();
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
            let config = AppConfig::load();
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

    let max_attempts = AppConfig::load().retries.max(1);
    let mut attempt = 0;

    loop {
        attempt += 1;
        let op_res = rpc_async(method.clone(), param_str.clone()).await;
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
                    tasks: None,
                    transfers: None,
                    checkers: None,
                };
                crate::app::save_active_operation(&op);
            }

            let mut status = "running".to_string();
            let mut err_msg = String::new();
            while status == "running" {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                if let Some((ref src, ref dest, is_copy)) = progress_info {
                    if let Some(ref tx_sender) = tx {
                        if let Ok(stats_res) = rpc_async("core/stats".to_string(), json!({ "group": format!("job/{}", id) }).to_string()).await {
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

                let status_res = rpc_async(
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
                crate::app::remove_active_operation(&op_id);
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
                crate::functions::log_info(&format!(
                    "[Auto-Retry] Job {} thất bại ở lần thử {}/{}: {}. Chuẩn bị thử lại...",
                    id, attempt, max_attempts, err_msg
                ));
                if attempt >= max_attempts {
                    return Err(err_msg);
                }
            }
        } else {
            let err_msg = "Không lấy được Job ID từ Rclone".to_string();
            crate::functions::log_info(&format!(
                "[Auto-Retry] Lần thử {}/{} thất bại: {}. Chuẩn bị thử lại...",
                attempt, max_attempts, err_msg
            ));
            if attempt >= max_attempts {
                return Err(err_msg);
            }
        }
    }
}
