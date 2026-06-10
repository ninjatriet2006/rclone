use crate::functions::rclone::rpc_async::rpc_async;
use crate::functions::rclone::get_job_description::register_job_description;
use crate::functions::rclone::thread_optimizer::inject_optimal_thread_config;
use crate::functions::app_config::AppConfig;
use serde_json::json;

pub async fn run_rpc_job_async(
    method: String,
    param: serde_json::Value,
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
        let desc_str = match method.as_str() {
            "sync/copy" => {
                let src = param_obj.get("srcFs").and_then(|s| s.as_str()).unwrap_or("");
                let dst = param_obj.get("dstFs").and_then(|d| d.as_str()).unwrap_or("");
                format!("Sao chép thư mục: {} -> {}", src, dst)
            }
            "sync/move" => {
                let src = param_obj.get("srcFs").and_then(|s| s.as_str()).unwrap_or("");
                let dst = param_obj.get("dstFs").and_then(|d| d.as_str()).unwrap_or("");
                format!("Di chuyển thư mục: {} -> {}", src, dst)
            }
            "sync/sync" => {
                let src = param_obj.get("srcFs").and_then(|s| s.as_str()).unwrap_or("");
                let dst = param_obj.get("dstFs").and_then(|d| d.as_str()).unwrap_or("");
                format!("Đồng bộ thư mục: {} -> {}", src, dst)
            }
            "operations/copyfile" => {
                let remote = param_obj.get("srcRemote").and_then(|r| r.as_str()).unwrap_or("");
                format!("Sao chép tệp: {}", remote)
            }
            "operations/movefile" => {
                let remote = param_obj.get("srcRemote").and_then(|r| r.as_str()).unwrap_or("");
                format!("Di chuyển tệp: {}", remote)
            }
            "operations/deletefile" => {
                let remote = param_obj.get("remote").and_then(|r| r.as_str()).unwrap_or("");
                format!("Xóa tệp: {}", remote)
            }
            "operations/purge" => {
                let fs = param_obj.get("fs").and_then(|r| r.as_str()).unwrap_or("");
                format!("Xóa thư mục: {}", fs)
            }
            "operations/mkdir" => {
                let fs = param_obj.get("fs").and_then(|r| r.as_str()).unwrap_or("");
                format!("Tạo thư mục: {}", fs)
            }
            "operations/rmdir" => {
                let fs = param_obj.get("fs").and_then(|r| r.as_str()).unwrap_or("");
                format!("Xóa thư mục rỗng: {}", fs)
            }
            "operations/rmdirs" => {
                let fs = param_obj.get("fs").and_then(|r| r.as_str()).unwrap_or("");
                format!("Xóa các thư mục rỗng đệ quy: {}", fs)
            }
            "operations/cleanup" => {
                let fs = param_obj.get("fs").and_then(|r| r.as_str()).unwrap_or("");
                format!("Dọn dẹp: {}", fs)
            }
            _ => format!("Tác vụ: {}", method),
        };
        param_obj.insert("_description".to_string(), serde_json::Value::String(desc_str.clone()));
        desc_str
    };
    let param_str = serde_json::Value::Object(param_obj).to_string();

    let op_res = rpc_async(method, param_str).await;
    let mut job_id = None;
    if let Ok(r) = op_res {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&r.output) {
            job_id = val.get("jobid").and_then(|j| j.as_i64());
        }
    }

    if let Some(id) = job_id {
        register_job_description(id, desc);
        let mut status = "running".to_string();
        let mut err_msg = String::new();
        while status == "running" {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
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
        if status == "success" {
            Ok(())
        } else {
            Err(err_msg)
        }
    } else {
        Err("Không lấy được Job ID từ Rclone".to_string())
    }
}
