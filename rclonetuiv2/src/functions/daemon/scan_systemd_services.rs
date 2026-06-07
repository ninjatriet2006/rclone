use crate::functions::SystemdServiceInfo;

#[cfg(all(unix, not(target_os = "macos")))]
pub fn scan_systemd_services() -> Vec<SystemdServiceInfo> {
    let mut services_map = std::collections::HashMap::new();

    let system_dir = "/etc/systemd/system";
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/home/bimatkeo".to_string());
    let user_dir = format!("{}/.config/systemd/user", home_dir);

    let mut scan_dir = |dir_path: &str, is_user: bool| {
        if let Ok(entries) = std::fs::read_dir(dir_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    if filename.ends_with(".service") {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if content.to_lowercase().contains("rclone") || filename.to_lowercase().contains("rclone") {
                                let mut desc = String::new();
                                for line in content.lines() {
                                    let trimmed = line.trim();
                                    if trimmed.starts_with("Description=") {
                                        desc = trimmed["Description=".len()..].trim().to_string();
                                        break;
                                    }
                                }
                                let name = filename.clone();
                                services_map.insert(
                                    (name.clone(), is_user),
                                    SystemdServiceInfo {
                                        name,
                                        file_path: path.to_string_lossy().to_string(),
                                        is_user,
                                        active_status: "inactive".to_string(),
                                        sub_state: "dead".to_string(),
                                        description: desc,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
    };

    scan_dir(system_dir, false);
    scan_dir(&user_dir, true);

    if let Ok(output) = std::process::Command::new("systemctl")
        .args(["list-units", "--type=service", "--all", "--no-legend"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let unit_name = parts[0];
                if unit_name.ends_with(".service") {
                    let name = unit_name.to_string();
                    let key = (name.clone(), false);
                    if services_map.contains_key(&key) || name.to_lowercase().contains("rclone") {
                        let active_status = parts[2].to_string();
                        let sub_state = parts[3].to_string();
                        let description = parts[4..].join(" ");

                        if let Some(service) = services_map.get_mut(&key) {
                            service.active_status = active_status;
                            service.sub_state = sub_state;
                            if !description.is_empty() {
                                service.description = description;
                            }
                        } else {
                            services_map.insert(
                                key,
                                SystemdServiceInfo {
                                    name,
                                    file_path: format!("/etc/systemd/system/{}", unit_name),
                                    is_user: false,
                                    active_status,
                                    sub_state,
                                    description,
                                },
                            );
                        }
                    }
                }
            }
        }
    }

    if let Ok(output) = std::process::Command::new("systemctl")
        .args(["--user", "list-units", "--type=service", "--all", "--no-legend"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let unit_name = parts[0];
                if unit_name.ends_with(".service") {
                    let name = unit_name.to_string();
                    let key = (name.clone(), true);
                    if services_map.contains_key(&key) || name.to_lowercase().contains("rclone") {
                        let active_status = parts[2].to_string();
                        let sub_state = parts[3].to_string();
                        let description = parts[4..].join(" ");

                        if let Some(service) = services_map.get_mut(&key) {
                            service.active_status = active_status;
                            service.sub_state = sub_state;
                            if !description.is_empty() {
                                service.description = description;
                            }
                        } else {
                            services_map.insert(
                                key,
                                SystemdServiceInfo {
                                    name,
                                    file_path: format!("{}/{}", user_dir, unit_name),
                                    is_user: true,
                                    active_status,
                                    sub_state,
                                    description,
                                },
                            );
                        }
                    }
                }
            }
        }
    }

    let mut services: Vec<SystemdServiceInfo> = services_map.into_values().collect();
    services.sort_by(|a, b| a.name.cmp(&b.name));
    services
}

#[cfg(any(not(unix), target_os = "macos"))]
pub fn scan_systemd_services() -> Vec<SystemdServiceInfo> {
    Vec::new()
}
