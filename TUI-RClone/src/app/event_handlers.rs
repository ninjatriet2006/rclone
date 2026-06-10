use crate::rclone;
use crate::ui;
use serde_json::{json, Value};
use std::process::Command;

use crate::app::{
    App, AppEvent, Screen, get_job_description
};

impl App {

    /// Định kỳ cập nhật thông tin Stats và trạng thái các Service chạy ngầm
    pub(crate) async fn handle_tick_event(&mut self, tx: tokio::sync::mpsc::UnboundedSender<AppEvent>) {
        if self.screen == Screen::ServicesAndMounts {
            // Throttle: chỉ quét tối đa mỗi services_scan_secs giây từ cấu hình
            if self.last_services_scan.elapsed() >= std::time::Duration::from_secs(self.config.services_scan_secs) {
                self.scan_running_services();
                self.scan_systemd_services();
                self.last_services_scan = std::time::Instant::now();
            }
        }
        if self.screen == Screen::JobMonitor {
            if self.stats_scan_in_progress {
                return;
            }
            // Throttle: chỉ cập nhật stats tối đa mỗi stats_refresh_ms mili giây từ cấu hình
            if self.last_stats_scan.elapsed() < std::time::Duration::from_millis(self.config.stats_refresh_ms) {
                return;
            }
            self.last_stats_scan = std::time::Instant::now();
            self.stats_scan_in_progress = true;

            let tx_clone = tx.clone();
            tokio::spawn(async move {
                // Lấy danh sách Job ID trước để kiểm tra xem có background job nào chạy không
                let list_res = rclone::rpc_async("job/list".to_string(), "{}".to_string()).await;
                
                let mut has_running_jobs = false;
                let mut ids_to_check = Vec::new();
                if let Ok(list_rpc) = &list_res {
                    if let Ok(list_val) = serde_json::from_str::<Value>(&list_rpc.output) {
                        let ids = if let Some(r_ids) = list_val.get("runningIds").and_then(|j| j.as_array()) {
                            r_ids.clone()
                        } else if let Some(job_ids) = list_val.get("jobids").and_then(|j| j.as_array()) {
                            job_ids.clone()
                        } else {
                            Vec::new()
                        };
                        ids_to_check = ids;
                        has_running_jobs = !ids_to_check.is_empty();
                    }
                }

                // Lấy Stats từ core Rclone RPC
                let res = rclone::rpc_async("core/stats".to_string(), "{}".to_string()).await;
                
                let mut active = Vec::new();
                let mut speed = 0.0;
                let mut upload_speed = 0.0;
                let mut download_speed = 0.0;
                let mut transferred = 0;
                let mut total = 0;
                let mut active_transfers = 0;
                let mut active_checks = 0;
                let mut upload_transfers = 0;
                let mut download_transfers = 0;
                let mut source_remotes = Vec::new();
                let mut dest_remotes = Vec::new();

                if let Ok(rpc_res) = &res {
                    if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                        if let Some(transfers) = val.get("transferring").and_then(|t| t.as_array()) {
                            active_transfers = transfers.len();
                        }
                        if let Some(checking) = val.get("checking").and_then(|c| c.as_array()) {
                            active_checks = checking.len();
                        }
                    }
                }

                if !has_running_jobs {
                    // Nếu không có job chạy ngầm, sử dụng stats toàn cục của rclone
                    if let Ok(rpc_res) = res {
                        if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                            speed = val.get("speed").and_then(|s| s.as_f64()).unwrap_or(0.0);
                            transferred = val.get("bytes").and_then(|b| b.as_u64()).unwrap_or(0);
                            total = val.get("totalBytes").and_then(|t| t.as_u64()).unwrap_or(0);

                            let mut files = Vec::new();

                            if let Some(transfers) = val.get("transferring").and_then(|t| t.as_array()) {
                                for t_val in transfers {
                                    let name = t_val.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                                    let size = t_val.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                                    let bytes = t_val.get("bytes").and_then(|b| b.as_u64()).unwrap_or(0);
                                    let speed_t = t_val.get("speed").and_then(|s| s.as_f64()).unwrap_or(0.0) as u64;
                                    let percentage = t_val.get("percentage").and_then(|p| p.as_u64()).unwrap_or(0) as u16;
                                    let eta = t_val.get("eta").and_then(|e| e.as_i64()).unwrap_or(-1);

                                    files.push(ui::monitor::JobFile {
                                        path: name,
                                        size,
                                        bytes,
                                        speed: speed_t,
                                        percentage,
                                        eta,
                                        status: "running".to_string(),
                                        error: String::new(),
                                    });
                                }
                            }

                            if let Some(checking) = val.get("checking").and_then(|c| c.as_array()) {
                                for c_val in checking {
                                    let name_opt = if let Some(name) = c_val.as_str() {
                                        Some(name.to_string())
                                    } else if let Some(name) = c_val.get("name").and_then(|n| n.as_str()) {
                                        Some(name.to_string())
                                    } else {
                                        None
                                    };
                                    if let Some(name) = name_opt {
                                        files.push(ui::monitor::JobFile {
                                            path: name,
                                            size: 0,
                                            bytes: 0,
                                            speed: 0,
                                            percentage: 0,
                                            eta: -1,
                                            status: "checking".to_string(),
                                            error: String::new(),
                                        });
                                    }
                                }
                            }

                            // Tải thêm completed/failed files toàn cục
                            let transferred_res = rclone::rpc_async(
                                "core/transferred".to_string(),
                                "{}".to_string(),
                            )
                            .await;
                            if let Ok(tr_res) = transferred_res {
                                if let Ok(tr_val) = serde_json::from_str::<Value>(&tr_res.output) {
                                    if let Some(transferred_arr) = tr_val.get("transferred").and_then(|t| t.as_array()) {
                                        for item in transferred_arr {
                                            let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                                            let size = item.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                                            let bytes = item.get("bytes").and_then(|b| b.as_u64()).unwrap_or(0);
                                            let error = item.get("error").and_then(|e| e.as_str()).unwrap_or("").to_string();
                                            let status = if error.is_empty() { "completed".to_string() } else { "failed".to_string() };

                                            files.push(ui::monitor::JobFile {
                                                path: name,
                                                size,
                                                bytes,
                                                speed: 0,
                                                percentage: 100,
                                                eta: 0,
                                                status,
                                                error,
                                            });
                                        }
                                    }
                                }
                            }

                            if !files.is_empty() || speed > 0.0 {
                                active.push(ui::monitor::TransferJob {
                                    name: "Tác vụ đồng bộ toàn cục".to_string(),
                                    size: total,
                                    bytes: transferred,
                                    speed: speed as u64,
                                    percentage: if total > 0 { ((transferred as f64 / total as f64) * 100.0) as u16 } else { 0 },
                                    eta: -1,
                                    job_id: None,
                                    start_time: String::new(),
                                    duration: 0.0,
                                    group: String::new(),
                                    description: "Tác vụ đồng bộ toàn cục của rclone".to_string(),
                                    files,
                                });
                            }
                        }
                    }
                } else {
                    // Nếu có job chạy ngầm, chúng ta sẽ chỉ lấy thông tin và tính tổng từ các job này
                    // Điều này giúp tránh bị nhân đôi (double-counting) stats và trùng lặp tệp tin hiển thị
                    for id_val in ids_to_check {
                        if let Some(id) = id_val.as_i64() {
                            // Lấy thông tin chi tiết từng Job
                            let status_res = rclone::rpc_async(
                                "job/status".to_string(),
                                json!({ "jobid": id }).to_string(),
                            )
                            .await;
                            if let Ok(sr) = status_res {
                                if let Ok(sval) = serde_json::from_str::<Value>(&sr.output) {
                                    let finished = sval.get("finished").and_then(|f| f.as_bool()).unwrap_or(false);
                                    if !finished {
                                        let desc_opt = get_job_description(id);
                                        let desc = desc_opt.as_deref().unwrap_or_else(|| {
                                            sval.get("description").and_then(|d| d.as_str()).unwrap_or("Tác vụ nền")
                                        });
                                        let duration = sval.get("duration").and_then(|d| d.as_f64()).unwrap_or(0.0);
                                        
                                        // Dự đoán hướng của Job
                                        let mut direction = crate::app::get_job_direction(id);
                                        if direction.is_none() {
                                            if let Some(arrow_idx) = desc.find("->") {
                                                let src_part = &desc[..arrow_idx];
                                                let dest_part = &desc[arrow_idx + 2..];
                                                let src_remote = src_part.contains(':');
                                                let dest_remote = dest_part.contains(':');
                                                if src_remote && !dest_remote {
                                                    direction = Some(crate::app::JobDirection::Download);
                                                } else if !src_remote && dest_remote {
                                                    direction = Some(crate::app::JobDirection::Upload);
                                                } else if src_remote && dest_remote {
                                                    direction = Some(crate::app::JobDirection::RemoteToRemote);
                                                } else {
                                                    direction = Some(crate::app::JobDirection::Local);
                                                }
                                            }
                                        }

                                        // Trích xuất tên remote từ mô tả công việc
                                        let (src_rem, dst_rem) = parse_remotes_from_description(&desc);
                                        if let Some(r) = src_rem {
                                            if !source_remotes.contains(&r) {
                                                source_remotes.push(r);
                                            }
                                        }
                                        if let Some(r) = dst_rem {
                                            if !dest_remotes.contains(&r) {
                                                dest_remotes.push(r);
                                            }
                                        }

                                        let mut speed_job = 0;
                                        let mut bytes_job = 0;
                                        let mut size_job = 0;
                                        let mut pct_job = 0;
                                        let mut eta_job = -1;
                                        let mut job_files = Vec::new();

                                        let group_stats_res = rclone::rpc_async(
                                            "core/stats".to_string(),
                                            json!({ "group": format!("job/{}", id) }).to_string(),
                                        )
                                        .await;
                                        if let Ok(st_res) = group_stats_res {
                                            if let Ok(st_val) = serde_json::from_str::<Value>(&st_res.output) {
                                                let speed_job_f64 = st_val.get("speed").and_then(|s| s.as_f64()).unwrap_or(0.0);
                                                speed_job = speed_job_f64 as u64;
                                                bytes_job = st_val.get("bytes").and_then(|b| b.as_u64()).unwrap_or(0);
                                                size_job = st_val.get("totalBytes").and_then(|b| b.as_u64()).unwrap_or(0);
                                                
                                                let job_transfers = st_val.get("transferring").and_then(|t| t.as_array()).map(|t| t.len()).unwrap_or(0);

                                                if let Some(dir) = direction {
                                                    match dir {
                                                        crate::app::JobDirection::Upload => {
                                                            upload_speed += speed_job_f64;
                                                            upload_transfers += job_transfers;
                                                        }
                                                        crate::app::JobDirection::Download => {
                                                            download_speed += speed_job_f64;
                                                            download_transfers += job_transfers;
                                                        }
                                                        crate::app::JobDirection::RemoteToRemote => {
                                                            upload_speed += speed_job_f64;
                                                            download_speed += speed_job_f64;
                                                            upload_transfers += job_transfers;
                                                            download_transfers += job_transfers;
                                                        }
                                                        crate::app::JobDirection::Local => {}
                                                    }
                                                }

                                                // Override size_job nếu có kích thước thực tế pre-scan/folder size (giải quyết phần trăm nhảy lung tung)
                                                if let Some(real_size) = crate::app::get_job_real_size(id) {
                                                    size_job = real_size;
                                                }

                                                // Cộng dồn vào global stats của màn hình monitor
                                                speed += speed_job_f64;
                                                transferred += bytes_job;
                                                total += size_job;

                                                if size_job > 0 {
                                                    pct_job = ((bytes_job as f64 / size_job as f64) * 100.0) as u16;
                                                }
                                                eta_job = st_val.get("eta").and_then(|e| e.as_i64()).unwrap_or(-1);

                                                // Thêm các file đang truyền của job này vào danh sách active
                                                if let Some(transfers) = st_val.get("transferring").and_then(|t| t.as_array()) {
                                                    active_transfers += transfers.len();
                                                    for t_val in transfers {
                                                        let name = t_val.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                                                        let size = t_val.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                                                        let bytes = t_val.get("bytes").and_then(|b| b.as_u64()).unwrap_or(0);
                                                        let speed_t = t_val.get("speed").and_then(|s| s.as_f64()).unwrap_or(0.0) as u64;
                                                        let percentage = t_val.get("percentage").and_then(|p| p.as_u64()).unwrap_or(0) as u16;
                                                        let eta = t_val.get("eta").and_then(|e| e.as_i64()).unwrap_or(-1);

                                                        job_files.push(ui::monitor::JobFile {
                                                            path: name,
                                                            size,
                                                            bytes,
                                                            speed: speed_t,
                                                            percentage,
                                                            eta,
                                                            status: "running".to_string(),
                                                            error: String::new(),
                                                        });
                                                    }
                                                }

                                                // Thêm các file đang checking
                                                if let Some(checking) = st_val.get("checking").and_then(|c| c.as_array()) {
                                                    active_checks += checking.len();
                                                    for c_val in checking {
                                                        let name_opt = if let Some(name) = c_val.as_str() {
                                                            Some(name.to_string())
                                                        } else if let Some(name) = c_val.get("name").and_then(|n| n.as_str()) {
                                                            Some(name.to_string())
                                                        } else {
                                                            None
                                                        };
                                                        if let Some(name) = name_opt {
                                                            job_files.push(ui::monitor::JobFile {
                                                                path: name,
                                                                size: 0,
                                                                bytes: 0,
                                                                speed: 0,
                                                                percentage: 0,
                                                                eta: -1,
                                                                status: "checking".to_string(),
                                                                error: String::new(),
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // Thêm các file đã hoàn thành/lỗi của job này từ core/transferred
                                        let transferred_res = rclone::rpc_async(
                                            "core/transferred".to_string(),
                                            json!({ "group": format!("job/{}", id) }).to_string(),
                                        )
                                        .await;
                                        if let Ok(tr_res) = transferred_res {
                                            if let Ok(tr_val) = serde_json::from_str::<Value>(&tr_res.output) {
                                                if let Some(transferred_arr) = tr_val.get("transferred").and_then(|t| t.as_array()) {
                                                    for item in transferred_arr {
                                                        let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                                                        let size = item.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                                                        let bytes = item.get("bytes").and_then(|b| b.as_u64()).unwrap_or(0);
                                                        let error = item.get("error").and_then(|e| e.as_str()).unwrap_or("").to_string();
                                                        let status = if error.is_empty() { "completed".to_string() } else { "failed".to_string() };

                                                        job_files.push(ui::monitor::JobFile {
                                                            path: name,
                                                            size,
                                                            bytes,
                                                            speed: 0,
                                                            percentage: 100,
                                                            eta: 0,
                                                            status,
                                                            error,
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                        
                                        // Thêm vào danh sách active
                                        active.push(ui::monitor::TransferJob {
                                            name: format!("[Job {}] {} (Chạy {:.1}s)", id, desc, duration),
                                            size: size_job,
                                            bytes: bytes_job,
                                            speed: speed_job,
                                            percentage: pct_job,
                                            eta: eta_job,
                                            job_id: Some(id),
                                            start_time: sval.get("startTime").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                                            duration,
                                            group: sval.get("group").and_then(|g| g.as_str()).unwrap_or("").to_string(),
                                            description: desc.to_string(),
                                            files: job_files,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }

                let mut bottleneck_reason = "Tốc độ tối ưu / Bình thường (Optimal)".to_string();
                let mut is_throttled = false;

                if speed > 0.0 {
                    let config = crate::app_config::AppConfig::load();
                    let max_bw = config.max_bandwidth_bytes_per_sec as f64;
                    
                    if max_bw > 0.0 && speed >= max_bw * 0.90 {
                        bottleneck_reason = "Đạt giới hạn băng thông tối đa thiết lập (Bandwidth Limit)".to_string();
                    } else {
                        let avg_speed_per_transfer = if active_transfers > 0 {
                            speed / (active_transfers as f64)
                        } else {
                            0.0
                        };

                        let queued_count = active.iter().flat_map(|job| &job.files).filter(|f| f.status == "queued").count();

                        if active_transfers >= 16 && avg_speed_per_transfer < 30_000.0 && speed < 1_500_000.0 {
                            let side = if upload_transfers > 0 && download_transfers == 0 {
                                let rem_list = if dest_remotes.is_empty() {
                                    "Local".to_string()
                                } else {
                                    format!("{}(đích)", dest_remotes.join(", "))
                                };
                                format!(" Tải lên [{}]", rem_list)
                            } else if download_transfers > 0 && upload_transfers == 0 {
                                let rem_list = if source_remotes.is_empty() {
                                    "Local".to_string()
                                } else {
                                    format!("{}(nguồn)", source_remotes.join(", "))
                                };
                                format!(" Tải xuống [{}]", rem_list)
                            } else if upload_transfers > 0 && download_transfers > 0 {
                                let src_list = if source_remotes.is_empty() {
                                    "Local".to_string()
                                } else {
                                    format!("{}(nguồn)", source_remotes.join(", "))
                                };
                                let dst_list = if dest_remotes.is_empty() {
                                    "Local".to_string()
                                } else {
                                    format!("{}(đích)", dest_remotes.join(", "))
                                };
                                format!(" [{} -> {}]", src_list, dst_list)
                            } else {
                                "".to_string()
                            };
                            bottleneck_reason = format!("Bị giới hạn API Cloud{} (Throttling / Rate Limit - Mở quá nhiều luồng)", side);
                            is_throttled = true;
                        } else if active_transfers > 0 && active_transfers <= 3 && queued_count > 5 && speed < 2_000_000.0 {
                            let side = if upload_transfers > 0 && download_transfers == 0 {
                                let rem_list = if dest_remotes.is_empty() {
                                    "Local".to_string()
                                } else {
                                    format!("{}(đích)", dest_remotes.join(", "))
                                };
                                format!(" Tải lên [{}]", rem_list)
                            } else if download_transfers > 0 && upload_transfers == 0 {
                                let rem_list = if source_remotes.is_empty() {
                                    "Local".to_string()
                                } else {
                                    format!("{}(nguồn)", source_remotes.join(", "))
                                };
                                format!(" Tải xuống [{}]", rem_list)
                            } else if upload_transfers > 0 && download_transfers > 0 {
                                let src_list = if source_remotes.is_empty() {
                                    "Local".to_string()
                                } else {
                                    format!("{}(nguồn)", source_remotes.join(", "))
                                };
                                let dst_list = if dest_remotes.is_empty() {
                                    "Local".to_string()
                                } else {
                                    format!("{}(đích)", dest_remotes.join(", "))
                                };
                                format!(" [{} -> {}]", src_list, dst_list)
                            } else {
                                "".to_string()
                            };
                            bottleneck_reason = format!("Nghẽn do thiếu luồng cho nhiều file nhỏ{} (Low Threads)", side);
                        }
                    }
                } else if has_running_jobs {
                    bottleneck_reason = "Đang kết nối hoặc chờ phản hồi từ Cloud (Connecting / Latency)".to_string();
                } else {
                    bottleneck_reason = "Không có truyền tải dữ liệu (Idle)".to_string();
                }

                // Cập nhật bộ điều tiết luồng động (Adaptive Thread Controller)
                let config_thread = crate::app_config::AppConfig::load();
                if is_throttled {
                    // Giảm nhẹ hệ số nhân đi -0.5 khi gặp nghẽn API
                    if let Ok(mut state) = crate::app::operations::DYNAMIC_THREAD_STATE.write() {
                        state.current_transfers_multiplier = (state.current_transfers_multiplier - 0.5).max(config_thread.min_multiplier);
                        state.last_bottleneck_time = Some(std::time::Instant::now());
                        state.consecutive_success_ticks = 0;
                        crate::app_config::log_info(&format!(
                            "[Dynamic Thread Control] Phát hiện nghẽn API! Giảm hệ số nhân xuống còn {}",
                            state.current_transfers_multiplier
                        ));
                    }
                } else if bottleneck_reason.contains("Bình thường") || bottleneck_reason.contains("Optimal") {
                    // Dần dần tăng hệ số nhân để dò tìm Rate Limit
                    if let Ok(mut state) = crate::app::operations::DYNAMIC_THREAD_STATE.write() {
                        let time_since_bottleneck = state.last_bottleneck_time.map(|t| t.elapsed().as_secs()).unwrap_or(999);
                        if time_since_bottleneck >= 12 {
                            state.consecutive_success_ticks += 1;
                            if state.consecutive_success_ticks >= 8 {
                                state.current_transfers_multiplier = (state.current_transfers_multiplier + 0.25).min(config_thread.max_multiplier);
                                state.consecutive_success_ticks = 0;
                                crate::app_config::log_info(&format!(
                                    "[Dynamic Thread Control] Hệ thống ổn định. Tăng hệ số nhân lên {} để thử nghiệm giới hạn API",
                                    state.current_transfers_multiplier
                                ));
                            }
                        }
                    }
                }

                // Cộng dồn active_transfers và active_checks cho các active operations nội bộ
                let active_ops_local = crate::app::load_active_operations();
                let pre_ops_local = crate::app::load_pre_operations();
                let local_stats_guard = crate::app::operations::LOCAL_TRANSFER_STATS.lock().ok();

                for op in &active_ops_local {
                    let is_scanning = pre_ops_local.iter().any(|po| po.id == op.id && po.status == "scanning");
                    if is_scanning {
                        active_checks += 4; // giả lập 4 checkers trong quá trình kiểm tra/quét
                    } else if let Some(ref tasks) = op.tasks {
                        let transferring_count = tasks.iter().filter(|t| t.status == crate::app::TaskStatus::Transferring).count();
                        active_transfers += transferring_count;
                        if transferring_count > 0 {
                            active_checks += 4; // giả lập 4 checkers trong quá trình copy
                        }

                        let op_stats = local_stats_guard.as_ref().and_then(|map| map.get(&op.id));

                        if let Some(stats) = op_stats {
                            let speed_op_f64 = stats.total_speed as f64;
                            speed += speed_op_f64;

                            let src_remote = op.src.contains(':');
                            let dest_remote = op.dest.contains(':');
                            let direction = if src_remote && !dest_remote {
                                Some(crate::app::JobDirection::Download)
                            } else if !src_remote && dest_remote {
                                Some(crate::app::JobDirection::Upload)
                            } else if src_remote && dest_remote {
                                Some(crate::app::JobDirection::RemoteToRemote)
                            } else {
                                Some(crate::app::JobDirection::Local)
                            };

                            if let Some(dir) = direction {
                                match dir {
                                    crate::app::JobDirection::Upload => {
                                        upload_speed += speed_op_f64;
                                    }
                                    crate::app::JobDirection::Download => {
                                        download_speed += speed_op_f64;
                                    }
                                    crate::app::JobDirection::RemoteToRemote => {
                                        upload_speed += speed_op_f64;
                                        download_speed += speed_op_f64;
                                    }
                                    crate::app::JobDirection::Local => {}
                                }
                            }
                        }

                        for task in tasks {
                            total += task.size;
                            if task.status == crate::app::TaskStatus::Completed || task.status == crate::app::TaskStatus::Skipped {
                                transferred += task.size;
                            } else if task.status == crate::app::TaskStatus::Transferring {
                                if let Some(stats) = op_stats.and_then(|os| os.files.get(&task.name)) {
                                    transferred += stats.bytes;
                                }
                            }
                        }
                    }
                }

                let config = crate::app_config::AppConfig::load();
                let multiplier = if let Ok(dt_state) = crate::app::operations::DYNAMIC_THREAD_STATE.read() {
                    dt_state.current_transfers_multiplier
                } else {
                    1.0
                };
                
                // Calculate transfers limit
                let transfers_limit = if let Some(t_fixed) = config.transfers_prior_fixed {
                    ((t_fixed as f64 * multiplier).round() as usize).clamp(1, config.max_transfers as usize)
                } else {
                    ((config.min_transfers as f64 * multiplier).round() as usize).clamp(config.min_transfers as usize, config.max_transfers as usize)
                };

                // Calculate checkers limit
                let checkers_limit = if let Some(c_fixed) = config.checkers_prior_fixed {
                    ((c_fixed as f64 * multiplier).round() as usize).clamp(1, config.max_checkers as usize)
                } else {
                    let base_c = config.min_checkers * 2;
                    ((base_c as f64 * multiplier).round() as usize).clamp(config.min_checkers as usize, config.max_checkers as usize)
                };

                let _ = tx_clone.send(AppEvent::JobStatsUpdate {
                    speed,
                    upload_speed,
                    download_speed,
                    transferred,
                    total,
                    active,
                    active_transfers,
                    active_checks,
                    transfers_limit,
                    checkers_limit,
                    bottleneck_reason,
                });
            });
        }
    }

    /// Diệt sạch các tiến trình dịch vụ ngầm khi đóng app (Bug 54, 100)
    #[allow(dead_code)]
    pub(crate) fn kill_all_active_services(&mut self) {
        for s in &self.services_state.active_services {
            // Gửi tín hiệu kill PID
            #[cfg(unix)]
            {
                let _ = Command::new("kill").arg(s.pid.to_string()).status();
                // Nếu là mount, cố unmount point cưỡng chế (Bug 95)
                if s.service_type_str == "Mount" {
                    let _ = Command::new("fusermount").args(["-uz", &s.path]).status();
                }
            }
            #[cfg(not(unix))]
            {
                let _ = Command::new("taskkill").args(["/F", "/PID", &s.pid.to_string()]).status();
            }
        }
        self.services_state.active_services.clear();
        self.save_active_services_to_file();
    }

    /// Từng bước thiết lập các cờ trong connection Wizard
    pub(crate) async fn advance_connection_wizard(
        &mut self,
        mut remaining_providers: Vec<String>,
        _tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        if remaining_providers.is_empty() {
            self.connection_state.wizard = ui::connection::WizardState::None;
            return;
        }

        let provider = remaining_providers.remove(0);
        self.connection_state.wizard = ui::connection::WizardState::InputRemoteName {
            provider,
            input_buffer: String::new(),
            selected_providers: remaining_providers,
        };
    }
    pub fn refresh_tui_selector_list(
        &mut self,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        if let ui::explorer::ExplorerPopup::TuiExplorerSelector {
            ref mut loading,
            ref remote,
            ref path,
            ..
        } = self.explorer_state.popup
        {
            *loading = true;
            let remote = remote.clone();
            let path = path.clone();
            let tx_clone = tx.clone();
            let timeout_secs = self.config.cloud_list_timeout_secs;

            tokio::spawn(async move {
                if remote.is_empty() && path.is_empty() {
                    let res = rclone::rpc_async("config/listremotes".to_string(), "{}".to_string()).await;
                    match res {
                        Ok(rpc_res) => {
                            if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                                if let Some(remotes_arr) = val.get("remotes").and_then(|r| r.as_array()) {
                                    let mut items = vec![ui::explorer::FileItem {
                                        name: "[Local System]".to_string(),
                                        size: 0,
                                        is_dir: true,
                                        mod_time: "---".to_string(),
                                        id: None,
                                    }];
                                    for r_val in remotes_arr {
                                        if let Some(r_str) = r_val.as_str() {
                                            items.push(ui::explorer::FileItem {
                                                name: r_str.to_string(),
                                                size: 0,
                                                is_dir: true,
                                                mod_time: "---".to_string(),
                                                id: None,
                                            });
                                        }
                                    }
                                    let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                                        result: Ok(items),
                                    });
                                    return;
                                }
                            }
                            let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                                result: Err("Không thể phân tích danh sách remote".to_string()),
                            });
                        }
                        Err(e) => {
                            let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                                result: Err(e),
                            });
                        }
                    }
                } else {
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

                    let list_future = rclone::rpc_async("operations/list".to_string(), input_param);
                    let res = match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), list_future).await {
                        Ok(inner_res) => inner_res,
                        Err(_) => Err("Hết thời gian chờ phản hồi từ Cloud (Timeout)".to_string()),
                    };

                    match res {
                        Ok(rpc_res) => {
                            if rpc_res.status == 200 {
                                if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                                    if let Some(err_str) = val.get("error").and_then(|e| e.as_str()) {
                                        let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
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
                                                items.push(ui::explorer::FileItem {
                                                    name,
                                                    size,
                                                    is_dir: true,
                                                    mod_time: cleaned_time,
                                                    id,
                                                });
                                            }
                                        }
                                        items.sort_by(|a, b| a.name.cmp(&b.name));

                                        let at_root = if remote.is_empty() {
                                            path == "/" || path.is_empty()
                                        } else {
                                            path.is_empty()
                                        };
                                        if !at_root {
                                            items.insert(
                                                0,
                                                ui::explorer::FileItem {
                                                    name: "..".to_string(),
                                                    size: 0,
                                                    is_dir: true,
                                                    mod_time: "---".to_string(),
                                                    id: None,
                                                },
                                            );
                                        }

                                        let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                                            result: Ok(items),
                                        });
                                    }
                                }
                            } else {
                                let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                                    result: Err(rpc_res.output),
                                });
                            }
                        }
                        Err(e) => {
                            let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                                result: Err(e),
                            });
                        }
                    }
                }
            });
        }
    }
}

fn parse_remotes_from_description(desc: &str) -> (Option<String>, Option<String>) {
    if let Some(arrow_idx) = desc.find("->") {
        let src_part = desc[..arrow_idx].trim();
        let dest_part = desc[arrow_idx + 2..].trim();

        let src_token = src_part.split_whitespace().last().unwrap_or("");
        let dest_token = dest_part.split_whitespace().next().unwrap_or("");

        let src_remote = if let Some(colon_idx) = src_token.find(':') {
            let r_name = &src_token[..colon_idx];
            if !r_name.is_empty() {
                Some(r_name.to_string())
            } else {
                None
            }
        } else {
            None
        };

        let dest_remote = if let Some(colon_idx) = dest_token.find(':') {
            let r_name = &dest_token[..colon_idx];
            if !r_name.is_empty() {
                Some(r_name.to_string())
            } else {
                None
            }
        } else {
            None
        };

        (src_remote, dest_remote)
    } else {
        (None, None)
    }
}
