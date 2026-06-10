pub fn detect_systemd_service(_pid: u32) -> Option<(String, bool)> {
    #[cfg(unix)]
    {
        if let Ok(content) = std::fs::read_to_string(format!("/proc/{}/cgroup", _pid)) {
            for line in content.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 {
                    let cgroup_path = parts[2];
                    if cgroup_path.contains(".service") {
                        let segments: Vec<&str> = cgroup_path.split('/').collect();
                        let mut service_unit = None;
                        for seg in segments.iter().rev() {
                            if seg.ends_with(".service") && !seg.starts_with("user@") && !seg.starts_with("user-") && *seg != "init.service" {
                                service_unit = Some(seg.to_string());
                                            break;
                            }
                        }
                        if let Some(unit) = service_unit {
                            let is_user = cgroup_path.contains("/user.slice/") || cgroup_path.contains("user@");
                            return Some((unit, is_user));
                        }
                    }
                }
            }
        }
    }
    None
}
