use std::sync::RwLock;

lazy_static::lazy_static! {
    pub static ref DYNAMIC_THREAD_STATE: RwLock<DynamicThreadState> = RwLock::new(DynamicThreadState::new());
}

#[derive(Debug, Clone)]
pub struct DynamicThreadState {
    pub current_transfers_multiplier: f64,
    pub last_bottleneck_time: Option<std::time::Instant>,
    pub consecutive_success_ticks: u32,
}

impl DynamicThreadState {
    pub fn new() -> Self {
        Self {
            current_transfers_multiplier: 1.0,
            last_bottleneck_time: None,
            consecutive_success_ticks: 0,
        }
    }
}

pub async fn get_directory_stats(src: &str) -> Option<(u64, u64)> {
    let param = serde_json::json!({
        "fs": src,
    }).to_string();
    if let Ok(res) = crate::functions::rclone::rpc_async("operations/size".to_string(), param).await {
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

pub fn extract_remote_name(path: &str) -> Option<String> {
    if let Some(colon_idx) = path.find(':') {
        let remote_part = &path[..colon_idx];
        if let Some(comma_idx) = remote_part.find(',') {
            Some(remote_part[..comma_idx].to_string())
        } else {
            Some(remote_part.to_string())
        }
    } else {
        None
    }
}

pub async fn get_remote_type(remote_name: &str) -> Option<String> {
    if remote_name.is_empty() {
        return None;
    }
    let param = serde_json::json!({ "name": remote_name }).to_string();
    if let Ok(res) = crate::functions::rclone::rpc_async("config/get".to_string(), param).await {
        if res.status == 200 {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&res.output) {
                if let Some(t) = val.get("type").and_then(|t| t.as_str()) {
                    return Some(t.to_string());
                }
            }
        }
    }
    None
}

pub fn get_concurrency_limit(src_type: Option<&str>, dst_type: Option<&str>) -> (u64, u64) {
    let mut limit_transfers = 64; // default max
    let mut limit_checkers = 128; // default max

    let types = [src_type, dst_type];
    for t in types.iter().flatten() {
        let (t_limit, c_limit) = match *t {
            "drive" => (2, 4),      // Google Drive: strict limit
            "onedrive" => (2, 4),   // OneDrive: aggressive throttling
            "box" => (2, 4),
            "dropbox" => (3, 6),
            "s3" | "google cloud storage" | "azureblob" => (8, 16),
            _ => (4, 8),
        };
        if t_limit < limit_transfers {
            limit_transfers = t_limit;
        }
        if c_limit < limit_checkers {
            limit_checkers = c_limit;
        }
    }
    (limit_transfers, limit_checkers)
}

pub fn calculate_optimal_threads(total_files: u64, total_size: u64, max_bandwidth: u64) -> (u64, u64) {
    calculate_optimal_threads_v2(total_files, total_size, max_bandwidth, None, None)
}

pub fn calculate_optimal_threads_v2(
    total_files: u64,
    total_size: u64,
    max_bandwidth: u64,
    src_type: Option<&str>,
    dst_type: Option<&str>,
) -> (u64, u64) {
    let config = crate::functions::AppConfig::load();

    let (cloud_max_transfers, cloud_max_checkers) = get_concurrency_limit(src_type, dst_type);

    let multiplier = if let Ok(state) = DYNAMIC_THREAD_STATE.read() {
        state.current_transfers_multiplier
    } else {
        1.0
    };

    let base_transfers = if total_files == 0 {
        config.min_transfers
    } else {
        let avg_file_size_bytes = total_size / total_files;

        if avg_file_size_bytes >= 10_000_000 {
            let max_large = config.max_transfers.max(config.min_transfers);
            (total_files / 2).clamp(config.min_transfers, max_large)
        } else {
            let latency_secs = 1.0;
            let single_thread_throughput = (avg_file_size_bytes as f64) / latency_secs;
            if single_thread_throughput <= 0.0 {
                config.max_transfers
            } else {
                let required_transfers = (max_bandwidth as f64) / single_thread_throughput;
                (required_transfers.round() as u64).clamp(config.min_transfers, config.max_transfers)
            }
        }
    };

    let base_checkers = (base_transfers * 2).clamp(config.min_checkers, config.max_checkers);

    let mut transfers = (base_transfers as f64 * multiplier).round() as u64;
    let mut checkers = (base_checkers as f64 * multiplier).round() as u64;

    transfers = transfers.clamp(config.min_transfers, config.max_transfers);
    checkers = checkers.clamp(config.min_checkers, config.max_checkers);

    if let Some(t_fixed) = config.transfers_prior_fixed {
        transfers = transfers.min(t_fixed);
    } else {
        transfers = transfers.min(cloud_max_transfers);
    }

    if let Some(c_fixed) = config.checkers_prior_fixed {
        checkers = checkers.min(c_fixed);
    } else {
        checkers = checkers.min(cloud_max_checkers);
    }

    (transfers.max(1), checkers.max(1))
}

pub async fn inject_optimal_thread_config(
    param: &mut serde_json::Value,
    src: &str,
    is_dir: bool,
    max_bandwidth: u64,
) -> (u64, u64) {
    let config = crate::functions::AppConfig::load();
    let dst = param.get("dstFs").and_then(|d| d.as_str()).unwrap_or("");

    let src_remote = extract_remote_name(src);
    let dst_remote = extract_remote_name(dst);

    let src_type = if let Some(ref r) = src_remote {
        get_remote_type(r).await
    } else {
        None
    };

    let dst_type = if let Some(ref r) = dst_remote {
        get_remote_type(r).await
    } else {
        None
    };

    let (cloud_max_transfers, cloud_max_checkers) = get_concurrency_limit(src_type.as_deref(), dst_type.as_deref());

    let multiplier = if let Ok(state) = DYNAMIC_THREAD_STATE.read() {
        state.current_transfers_multiplier
    } else {
        1.0
    };

    let mut transfers = config.transfers_prior_fixed.unwrap_or(config.min_transfers);
    let mut checkers = config.checkers_prior_fixed.unwrap_or(config.min_checkers);

    if config.transfers_prior_fixed.is_none() || config.checkers_prior_fixed.is_none() {
        if is_dir {
            if let Some((count, bytes)) = get_directory_stats(src).await {
                let (opt_t, opt_c) = calculate_optimal_threads_v2(count, bytes, max_bandwidth, src_type.as_deref(), dst_type.as_deref());
                if config.transfers_prior_fixed.is_none() {
                    transfers = opt_t;
                }
                if config.checkers_prior_fixed.is_none() {
                    checkers = opt_c;
                }
                crate::functions::log_info(&format!(
                    "[Thread Optimizer] Thư mục: {} | Số file: {} | Tổng size: {} bytes | Băng thông: {} bytes/s | Loại: Src={:?}, Dst={:?} | Hệ số nhân: {} -> Luồng tối ưu: Transfers={}, Checkers={}",
                    src, count, bytes, max_bandwidth, src_type, dst_type, multiplier, transfers, checkers
                ));
            } else {
                let base_t = config.min_transfers * 2;
                let base_c = config.min_checkers * 2;

                transfers = (base_t as f64 * multiplier).round() as u64;
                checkers = (base_c as f64 * multiplier).round() as u64;

                transfers = transfers.clamp(config.min_transfers, config.max_transfers).min(cloud_max_transfers);
                checkers = checkers.clamp(config.min_checkers, config.max_checkers).min(cloud_max_checkers);

                crate::functions::log_info(&format!(
                    "[Thread Optimizer] Thất bại khi lấy size thư mục: {}. Sử dụng luồng mặc định điều phối động: Transfers={}, Checkers={}",
                    src, transfers, checkers
                ));
            }
        } else {
            let base_t = 4.min(config.min_transfers).max(1);
            let base_c = 8.min(config.min_checkers).max(1);

            transfers = (base_t as f64 * multiplier).round() as u64;
            checkers = (base_c as f64 * multiplier).round() as u64;

            transfers = transfers.clamp(config.min_transfers.min(base_t), config.max_transfers).min(cloud_max_transfers);
            checkers = checkers.clamp(config.min_checkers.min(base_c), config.max_checkers).min(cloud_max_checkers);
        }
    } else {
        let final_t = (transfers as f64 * multiplier).round() as u64;
        let final_c = (checkers as f64 * multiplier).round() as u64;

        transfers = final_t.min(transfers).clamp(config.min_transfers, config.max_transfers);
        checkers = final_c.min(checkers).clamp(config.min_checkers, config.max_checkers);
    }

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

    (transfers.max(1), checkers.max(1))
}

pub async fn check_and_apply_rate_limiting(res: &crate::functions::SafeRpcResult) -> bool {
    let output_lower = res.output.to_lowercase();
    let is_throttled = res.status == 429
        || output_lower.contains("429")
        || output_lower.contains("ratelimitexceeded")
        || output_lower.contains("toomanyrequests")
        || output_lower.contains("resourceexhausted")
        || output_lower.contains("quota exceeded")
        || output_lower.contains("slow down")
        || output_lower.contains("throttling")
        || output_lower.contains("user rate limit");

    if is_throttled {
        if let Ok(mut state) = DYNAMIC_THREAD_STATE.write() {
            let config = crate::functions::AppConfig::load();
            state.current_transfers_multiplier = (state.current_transfers_multiplier - 0.5).max(config.min_multiplier);
            state.last_bottleneck_time = Some(std::time::Instant::now());
            state.consecutive_success_ticks = 0;
            crate::functions::log_info(&format!(
                "[Dynamic Thread Control] Phát hiện nghẽn/giới hạn API trong checker! Giảm hệ số nhân xuống còn {}, tạm nghỉ 2 giây.",
                state.current_transfers_multiplier
            ));
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    is_throttled
}
