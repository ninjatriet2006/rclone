use crate::ui;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::app::{
    App,
    get_rclone_cmd
};

fn get_rclone_exec_path() -> String {
    if let Ok(output) = std::process::Command::new("which").arg("rclone").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return path;
            }
        }
    }
    for path in &["/usr/bin/rclone", "/usr/local/bin/rclone", "/bin/rclone", "/usr/sbin/rclone"] {
        if std::path::Path::new(path).exists() {
            return path.to_string();
        }
    }
    let cmd = get_rclone_cmd();
    if cmd != "rclone" && std::path::Path::new(&cmd).is_absolute() && std::path::Path::new(&cmd).exists() {
        return cmd;
    }
    if let Ok(current_dir) = std::env::current_dir() {
        let parent_rclone_bin = current_dir.join("../rclone_bin");
        if parent_rclone_bin.exists() {
            if let Ok(abs) = std::fs::canonicalize(parent_rclone_bin) {
                return abs.to_string_lossy().to_string();
            }
        }
        let local_rclone_bin = current_dir.join("rclone_bin");
        if local_rclone_bin.exists() {
            if let Ok(abs) = std::fs::canonicalize(local_rclone_bin) {
                return abs.to_string_lossy().to_string();
            }
        }
        let parent_rclone = current_dir.join("../rclone");
        if parent_rclone.exists() {
            if let Ok(abs) = std::fs::canonicalize(parent_rclone) {
                return abs.to_string_lossy().to_string();
            }
        }
        let local_rclone = current_dir.join("rclone");
        if local_rclone.exists() {
            if let Ok(abs) = std::fs::canonicalize(local_rclone) {
                return abs.to_string_lossy().to_string();
            }
        }
    }
    "/usr/bin/rclone".to_string()
}

fn get_fusermount_exec_path() -> String {
    if let Ok(output) = std::process::Command::new("which").arg("fusermount").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return path;
            }
        }
    }
    for path in &["/usr/bin/fusermount", "/bin/fusermount"] {
        if std::path::Path::new(path).exists() {
            return path.to_string();
        }
    }
    "/bin/fusermount".to_string()
}

impl App {

    pub(crate) fn detect_systemd_service(_pid: u32) -> Option<(String, bool)> {
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

    pub(crate) fn parse_rclone_args(&self, pid: u32, args: &[String]) -> Option<ui::services::ActiveService> {
        if args.is_empty() {
            return None;
        }
        let exe = &args[0];
        let is_rclone = exe == "rclone" 
            || exe.ends_with("/rclone") 
            || exe == "rclone.exe" 
            || exe.to_lowercase().ends_with("\\rclone.exe");

        if !is_rclone {
            return None;
        }

        let systemd_info = Self::detect_systemd_service(pid);

        if args.contains(&"mount".to_string()) || args.contains(&"nfsmount".to_string()) {
            let is_nfs = args.contains(&"nfsmount".to_string());
            let cmd_name = if is_nfs { "nfsmount" } else { "mount" };
            let mut non_flags = Vec::new();
            for arg in args.iter().skip(1) {
                if !arg.starts_with('-') && arg != cmd_name {
                    non_flags.push(arg.clone());
                }
            }
            let (remote, local_mnt) = if non_flags.len() >= 2 {
                (non_flags[0].clone(), non_flags[1].clone())
            } else if non_flags.len() == 1 {
                (String::new(), non_flags[0].clone())
            } else {
                (String::new(), String::new())
            };
            let mut config_path = String::new();
            for arg in args {
                if arg.starts_with("--config=") {
                    config_path = arg.trim_start_matches("--config=").to_string();
                }
            }
            if config_path.is_empty() {
                if let Some(pos) = args.iter().position(|r| r == "--config") {
                    if pos + 1 < args.len() {
                        config_path = args[pos + 1].clone();
                    }
                }
            }
            let mut profile_name = "default".to_string();
            if !config_path.is_empty() {
                for (name, path) in &self.config.profiles {
                    if path == &config_path {
                        profile_name = name.clone();
                        break;
                    }
                }
            } else {
                profile_name = self.config.active_profile.clone();
            }
            let profile_prefix = if profile_name == "default" {
                String::new()
            } else {
                format!("{}: -> ", profile_name)
            };
            let details = if remote.is_empty() {
                format!("{}{}", profile_prefix, local_mnt)
            } else {
                let display_remote = if let Some(und) = Self::get_underlying_remote(&config_path, &remote) {
                    let base = und.split(':').next().unwrap_or(&und);
                    format!("{}: -> {}", base, remote)
                } else {
                    remote.clone()
                };
                format!("{}{}{} -> {}", profile_prefix, if display_remote.ends_with(':') || display_remote.contains("->") { "" } else { "" }, display_remote, local_mnt)
            };
            let details = if is_nfs { format!("NfsMount: {}", details) } else { details };

            let (service_type_str, final_details) = if let Some((unit_name, is_user)) = &systemd_info {
                let lvl = if *is_user { "Cá nhân" } else { "Hệ thống" };
                (format!("Service ({})", lvl), format!("Dịch vụ: {} | {}", unit_name, details))
            } else {
                ("Mount (Tạm thời)".to_string(), details)
            };

            Some(ui::services::ActiveService {
                service_type_str,
                remote,
                path: local_mnt,
                pid,
                details: final_details,
            })
        } else if args.contains(&"serve".to_string()) {
            let mut proto = "http".to_string();
            let mut addr = ":8080".to_string();
            let mut remote_path = String::new();
            
            if let Some(pos) = args.iter().position(|r| r == "serve") {
                if pos + 1 < args.len() {
                    proto = args[pos + 1].clone();
                }
            }

            for i in 0..args.len() {
                if args[i].starts_with("--addr=") {
                    addr = args[i]["--addr=".len()..].to_string();
                } else if args[i] == "--addr" && i + 1 < args.len() {
                    addr = args[i+1].clone();
                } else if !args[i].starts_with('-') && args[i] != "serve" && args[i] != proto && i > 0 && args[i-1] != "--addr" && args[i-1] != "--user" && args[i-1] != "--pass" {
                    remote_path = args[i].clone();
                }
            }
            let mut config_path = String::new();
            for arg in args {
                if arg.starts_with("--config=") {
                    config_path = arg.trim_start_matches("--config=").to_string();
                }
            }
            if config_path.is_empty() {
                if let Some(pos) = args.iter().position(|r| r == "--config") {
                    if pos + 1 < args.len() {
                        config_path = args[pos + 1].clone();
                    }
                }
            }
            let mut profile_name = "default".to_string();
            if !config_path.is_empty() {
                for (name, path) in &self.config.profiles {
                    if path == &config_path {
                        profile_name = name.clone();
                        break;
                    }
                }
            } else {
                profile_name = self.config.active_profile.clone();
            }
            let profile_prefix = if profile_name == "default" {
                String::new()
            } else {
                format!("{}: -> ", profile_name)
            };
            let details = if remote_path.is_empty() {
                format!("{}{}{}", profile_prefix, proto, addr)
            } else {
                let display_remote = if let Some(und) = Self::get_underlying_remote(&config_path, &remote_path) {
                    let base = und.split(':').next().unwrap_or(&und);
                    format!("{}: -> {}", base, remote_path)
                } else {
                    remote_path.clone()
                };
                format!("{}{}{} -> {}://{}", profile_prefix, display_remote, if display_remote.is_empty() { "" } else { " -> " }, proto, addr)
            };

            let (service_type_str, final_details) = if let Some((unit_name, is_user)) = &systemd_info {
                let lvl = if *is_user { "Cá nhân" } else { "Hệ thống" };
                (format!("Service ({})", lvl), format!("Dịch vụ: {} | {}", unit_name, details))
            } else {
                ("Serve (Tạm thời)".to_string(), details)
            };

            Some(ui::services::ActiveService {
                service_type_str,
                remote: remote_path,
                path: addr,
                pid,
                details: final_details,
            })
        } else if args.contains(&"rcd".to_string()) {
            let mut rc_addr = "localhost:5572".to_string();
            for i in 0..args.len() {
                if args[i].starts_with("--rc-addr=") {
                    rc_addr = args[i]["--rc-addr=".len()..].to_string();
                } else if args[i] == "--rc-addr" && i + 1 < args.len() {
                    rc_addr = args[i+1].clone();
                }
            }
            let mut config_path = String::new();
            for arg in args {
                if arg.starts_with("--config=") {
                    config_path = arg.trim_start_matches("--config=").to_string();
                }
            }
            if config_path.is_empty() {
                if let Some(pos) = args.iter().position(|r| r == "--config") {
                    if pos + 1 < args.len() {
                        config_path = args[pos + 1].clone();
                    }
                }
            }
            let mut profile_name = "default".to_string();
            if !config_path.is_empty() {
                for (name, path) in &self.config.profiles {
                    if path == &config_path {
                        profile_name = name.clone();
                        break;
                    }
                }
            } else {
                profile_name = self.config.active_profile.clone();
            }
            let profile_prefix = if profile_name == "default" {
                String::new()
            } else {
                format!("{}: -> ", profile_name)
            };
            let details = format!("{}Cổng Web: {}", profile_prefix, rc_addr);

            let (service_type_str, final_details) = if let Some((unit_name, is_user)) = &systemd_info {
                let lvl = if *is_user { "Cá nhân" } else { "Hệ thống" };
                (format!("Service ({})", lvl), format!("Dịch vụ: {} | {}", unit_name, details))
            } else {
                ("WebGui (Tạm thời)".to_string(), details)
            };

            Some(ui::services::ActiveService {
                service_type_str,
                remote: String::new(),
                path: rc_addr,
                pid,
                details: final_details,
            })
        } else {
            if let Some((unit_name, is_user)) = systemd_info {
                let lvl = if is_user { "Cá nhân" } else { "Hệ thống" };
                let service_type_str = format!("Service ({})", lvl);
                let details = format!("Dịch vụ: {} | Lệnh: {}", unit_name, args.join(" "));
                Some(ui::services::ActiveService {
                    service_type_str,
                    remote: String::new(),
                    path: String::new(),
                    pid,
                    details,
                })
            } else {
                None
            }
        }
    }

    /// Quét các tiến trình rclone đang chạy thực tế trên hệ thống (Linux /proc, Windows wmic)
    pub fn scan_running_services(&mut self) {
        let mut scanned_services = Vec::new();

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            if let Ok(entries) = fs::read_dir("/proc") {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        if path.is_dir() {
                            if let Some(pid_str) = path.file_name().and_then(|s| s.to_str()) {
                                if let Ok(pid) = pid_str.parse::<u32>() {
                                    let cmdline_path = path.join("cmdline");
                                    if let Ok(mut file) = fs::File::open(cmdline_path) {
                                        use std::io::Read;
                                        let mut buffer = Vec::new();
                                        if file.read_to_end(&mut buffer).is_ok() {
                                            let args: Vec<String> = buffer
                                                .split(|&b| b == 0)
                                                .filter_map(|slice| {
                                                    if slice.is_empty() {
                                                        None
                                                    } else {
                                                        Some(String::from_utf8_lossy(slice).trim().to_string())
                                                    }
                                                })
                                                .collect();
                                            if let Some(service) = self.parse_rclone_args(pid, &args) {
                                                scanned_services.push(service);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = std::process::Command::new("ps")
                .args(["-ax", "-o", "pid,command"])
                .output()
            {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    for line in stdout.lines().skip(1) { // Skip header line
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        if let Some(pos) = line.find(' ') {
                            let pid_str = &line[..pos];
                            let cmdline = &line[pos..].trim();
                            if let Ok(pid) = pid_str.parse::<u32>() {
                                let args = parse_cmdline(cmdline);
                                if let Some(service) = self.parse_rclone_args(pid, &args) {
                                    scanned_services.push(service);
                                }
                            }
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            let mut wmic_success = false;
            if let Ok(output) = std::process::Command::new("wmic")
                .args(["process", "where", "name='rclone.exe'", "get", "CommandLine,ProcessId", "/FORMAT:list"])
                .output()
            {
                if output.status.success() {
                    wmic_success = true;
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let mut current_cmdline = String::new();
                    let mut current_pid: Option<u32> = None;

                    for line in stdout.lines() {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        if line.starts_with("CommandLine=") {
                            current_cmdline = line["CommandLine=".len()..].trim().to_string();
                        } else if line.starts_with("ProcessId=") {
                            if let Ok(pid) = line["ProcessId=".len()..].trim().parse::<u32>() {
                                current_pid = Some(pid);
                            }
                        }

                        if !current_cmdline.is_empty() && current_pid.is_some() {
                            let pid = current_pid.unwrap();
                            let args = parse_cmdline(&current_cmdline);
                            if let Some(service) = self.parse_rclone_args(pid, &args) {
                                scanned_services.push(service);
                            }
                            current_cmdline.clear();
                            current_pid = None;
                        }
                    }
                }
            }

            // Fallback sang PowerShell nếu WMIC thất bại hoặc không có sẵn
            if !wmic_success {
                if let Ok(output) = std::process::Command::new("powershell")
                    .args([
                        "-NoProfile",
                        "-Command",
                        "Get-CimInstance Win32_Process -Filter \"name = 'rclone.exe'\" | Select-Object CommandLine, ProcessId | Format-List"
                    ])
                    .output()
                {
                    if output.status.success() {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let mut current_cmdline = String::new();
                        let mut current_pid: Option<u32> = None;

                        for line in stdout.lines() {
                            let line = line.trim();
                            if line.is_empty() {
                                continue;
                            }
                            if line.starts_with("CommandLine") {
                                if let Some(pos) = line.find(':') {
                                    current_cmdline = line[pos + 1..].trim().to_string();
                                }
                            } else if line.starts_with("ProcessId") {
                                if let Some(pos) = line.find(':') {
                                    if let Ok(pid) = line[pos + 1..].trim().parse::<u32>() {
                                        current_pid = Some(pid);
                                    }
                                }
                            }

                            if !current_cmdline.is_empty() && current_pid.is_some() {
                                let pid = current_pid.unwrap();
                                let args = parse_cmdline(&current_cmdline);
                                if let Some(service) = self.parse_rclone_args(pid, &args) {
                                    scanned_services.push(service);
                                }
                                current_cmdline.clear();
                                current_pid = None;
                            }
                        }
                    }
                }
            }
        }

        self.services_state.active_services = scanned_services;
    }

    /// Quét các dịch vụ systemd (rclone) cấp hệ thống và cấp cá nhân
    #[cfg(all(unix, not(target_os = "macos")))]
    pub fn scan_systemd_services(&mut self) {
        let mut services_map = std::collections::HashMap::new();

        // 1. Quét tệp trên đĩa cứng
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
                                        ui::services::SystemdServiceInfo {
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

        // 2. Chạy systemctl để lấy thông tin trạng thái hoạt động cấp hệ thống
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
                            let _load_state = parts[1];
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
                                      ui::services::SystemdServiceInfo {
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

          // 3. Chạy systemctl --user để lấy thông tin trạng thái cấp cá nhân
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
                              let _load_state = parts[1];
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
                                      ui::services::SystemdServiceInfo {
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

          let mut services: Vec<ui::services::SystemdServiceInfo> = services_map.into_values().collect();
          services.sort_by(|a, b| a.name.cmp(&b.name));
          self.services_state.systemd_services = services;
      }

    /// Quét các dịch vụ systemd (rclone) cấp hệ thống và cấp cá nhân (không hoạt động trên Windows/macOS)
    #[cfg(any(not(unix), target_os = "macos"))]
    pub fn scan_systemd_services(&mut self) {
        self.services_state.systemd_services.clear();
    }

    pub(crate) fn parse_exec_start_full(
        &self,
        exec_start: &str,
    ) -> (String, String, std::collections::HashMap<String, String>) {
        let words: Vec<&str> = exec_start.split_whitespace().collect();
        let mut remote = String::new();
        let mut mount_path = String::new();
        let mut flags = std::collections::HashMap::new();
        
        if let Some(mount_pos) = words.iter().position(|&w| w == "mount") {
            let mut non_flags = Vec::new();
            let mut i = mount_pos + 1;
            while i < words.len() {
                let w = words[i];
                if w.starts_with("--") {
                    if let Some(eq_pos) = w.find('=') {
                        let key = w[..eq_pos].to_string();
                        let val = w[eq_pos + 1..].to_string();
                        flags.insert(key, val);
                    } else {
                        if i + 1 < words.len() && !words[i + 1].starts_with("--") {
                            flags.insert(w.to_string(), words[i + 1].to_string());
                            i += 1;
                        } else {
                            flags.insert(w.to_string(), "true".to_string());
                        }
                    }
                } else {
                    non_flags.push(w.to_string());
                }
                i += 1;
            }
            if non_flags.len() >= 2 {
                remote = non_flags[0].clone();
                mount_path = non_flags[1].clone();
            } else if non_flags.len() == 1 {
                mount_path = non_flags[0].clone();
            }
        }
        (remote, mount_path, flags)
    }

    pub(crate) fn parse_systemd_file(&self, file_path: &str) -> std::io::Result<Vec<(String, String)>> {
        let content = std::fs::read_to_string(file_path)?;
        let mut fields = Vec::new();
        let mut current_section = String::new();

        let mut lines = content.lines().peekable();
        while let Some(line) = lines.next() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
                continue;
            }

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                current_section = trimmed[1..trimmed.len() - 1].to_string();
                continue;
            }

            if let Some(pos) = trimmed.find('=') {
                let key = trimmed[..pos].trim();
                let mut val = trimmed[pos + 1..].trim().to_string();

                while val.ends_with('\\') {
                    val.pop();
                    if let Some(next_line) = lines.next() {
                        val.push_str(" ");
                        val.push_str(next_line.trim());
                    } else {
                        break;
                    }
                }

                let full_key = if current_section.is_empty() {
                    key.to_string()
                } else {
                    format!("[{}]{}", current_section, key)
                };
                fields.push((full_key, val));
            }
        }

        Ok(fields)
    }

    pub(crate) fn load_systemd_service_fields(
        &self,
        file_path: &str,
        is_user: bool,
    ) -> std::io::Result<Vec<(String, String, String, Vec<String>)>> {
        let raw_fields = self.parse_systemd_file(file_path)?;
        let mut fields = Vec::new();

        let filename = std::path::Path::new(file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let service_name = filename.strip_suffix(".service").unwrap_or(&filename).to_string();

        let desc = raw_fields
            .iter()
            .find(|(k, _)| k == "[Unit]Description")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();

        let user = raw_fields
            .iter()
            .find(|(k, _)| k == "[Service]User")
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| std::env::var("USER").unwrap_or_else(|_| "bimatkeo".to_string()));

        let exec_start = raw_fields
            .iter()
            .find(|(k, _)| k == "[Service]ExecStart")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();

        let (remote, mount_path, parsed_flags) = self.parse_exec_start_full(&exec_start);

        fields.push((
            "_service_name".to_string(),
            "Tên dịch vụ".to_string(),
            service_name,
            Vec::new(),
        ));
        fields.push((
            "_service_level".to_string(),
            "Cấp chạy".to_string(),
            if is_user { "User (Cá nhân)".to_string() } else { "System (Hệ thống)".to_string() },
            vec!["User (Cá nhân)".to_string(), "System (Hệ thống)".to_string()],
        ));
        fields.push((
            "_remote".to_string(),
            "Cloud Remote".to_string(),
            remote,
            self.connection_state.remotes.clone(),
        ));
        fields.push((
            "_mount_path".to_string(),
            "Thư mục Mount".to_string(),
            mount_path,
            Vec::new(),
        ));
        fields.push((
            "_description".to_string(),
            "Mô tả".to_string(),
            desc,
            Vec::new(),
        ));
        fields.push((
            "_user".to_string(),
            "Tài khoản chạy".to_string(),
            user,
            Vec::new(),
        ));

        let get_flag = |key: &str| parsed_flags.get(key).cloned().unwrap_or_default();
        fields.push((
            "_flag_vfs_cache_mode".to_string(),
            "Chế độ Cache VFS".to_string(),
            get_flag("--vfs-cache-mode"),
            vec!["".to_string(), "off".to_string(), "minimal".to_string(), "writes".to_string(), "full".to_string()],
        ));
        fields.push((
            "_flag_vfs_cache_max_size".to_string(),
            "Dung lượng Cache tối đa".to_string(),
            get_flag("--vfs-cache-max-size"),
            Vec::new(),
        ));
        fields.push((
            "_flag_vfs_cache_max_age".to_string(),
            "Thời gian Cache tối đa".to_string(),
            get_flag("--vfs-cache-max-age"),
            Vec::new(),
        ));
        fields.push((
            "_flag_vfs_read_chunk_size".to_string(),
            "Kích thước đoạn đọc".to_string(),
            get_flag("--vfs-read-chunk-size"),
            Vec::new(),
        ));
        fields.push((
            "_flag_vfs_read_chunk_size_limit".to_string(),
            "Giới hạn đoạn đọc".to_string(),
            get_flag("--vfs-read-chunk-size-limit"),
            Vec::new(),
        ));
        fields.push((
            "_flag_dir_cache_time".to_string(),
            "Thời gian Cache thư mục".to_string(),
            get_flag("--dir-cache-time"),
            Vec::new(),
        ));
        fields.push((
            "_flag_attr_timeout".to_string(),
            "Timeout thuộc tính".to_string(),
            get_flag("--attr-timeout"),
            Vec::new(),
        ));
        fields.push((
            "_flag_buffer_size".to_string(),
            "Kích thước buffer RAM".to_string(),
            get_flag("--buffer-size"),
            Vec::new(),
        ));
        
        let allow_other_val = if parsed_flags.contains_key("--allow-other") { "Có (yes)".to_string() } else { "".to_string() };
        fields.push((
            "_flag_allow_other".to_string(),
            "Cho phép User khác".to_string(),
            allow_other_val,
            vec!["".to_string(), "Có (yes)".to_string()],
        ));

        let read_only_val = if parsed_flags.contains_key("--read-only") { "Có (yes)".to_string() } else { "".to_string() };
        fields.push((
            "_flag_read_only".to_string(),
            "Chế độ chỉ đọc".to_string(),
            read_only_val,
            vec!["".to_string(), "Có (yes)".to_string()],
        ));

        let allow_non_empty_val = if parsed_flags.contains_key("--allow-non-empty") { "Có (yes)".to_string() } else { "".to_string() };
        fields.push((
            "_flag_allow_non_empty".to_string(),
            "Cho phép thư mục chứa file".to_string(),
            allow_non_empty_val,
            vec!["".to_string(), "Có (yes)".to_string()],
        ));

        for (k, v) in raw_fields {
            let choices = if k == "[Service]Type" {
                vec!["simple".to_string(), "forking".to_string(), "oneshot".to_string(), "dbus".to_string(), "notify".to_string(), "idle".to_string()]
            } else if k == "[Service]Restart" {
                vec!["no".to_string(), "on-success".to_string(), "on-failure".to_string(), "on-abnormal".to_string(), "on-watchdog".to_string(), "on-abort".to_string(), "always".to_string()]
            } else {
                Vec::new()
            };
            fields.push((k, String::new(), v, choices));
        }

        Ok(fields)
    }

    pub(crate) fn init_create_systemd_service_fields(&self) -> Vec<(String, String, String, Vec<String>)> {
        let mut fields = Vec::new();
        let user = std::env::var("USER").unwrap_or_else(|_| "bimatkeo".to_string());
        let default_remote = self.connection_state.remotes.first().cloned().unwrap_or_default();

        fields.push((
            "_service_name".to_string(),
            "Tên dịch vụ".to_string(),
            "rclone-torrent".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_service_level".to_string(),
            "Cấp chạy".to_string(),
            "User (Cá nhân)".to_string(),
            vec!["User (Cá nhân)".to_string(), "System (Hệ thống)".to_string()],
        ));
        fields.push((
            "_remote".to_string(),
            "Cloud Remote".to_string(),
            default_remote,
            self.connection_state.remotes.clone(),
        ));
        fields.push((
            "_mount_path".to_string(),
            "Thư mục Mount".to_string(),
            "/media/bimatkeo/DATA/Torrents".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_description".to_string(),
            "Mô tả".to_string(),
            "Rclone Mount Service".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_user".to_string(),
            "Tài khoản chạy".to_string(),
            user,
            Vec::new(),
        ));

        // Rclone cờ ảo mặc định khi tạo mới
        fields.push((
            "_flag_vfs_cache_mode".to_string(),
            "Chế độ Cache VFS".to_string(),
            "full".to_string(),
            vec!["".to_string(), "off".to_string(), "minimal".to_string(), "writes".to_string(), "full".to_string()],
        ));
        fields.push((
            "_flag_vfs_cache_max_size".to_string(),
            "Dung lượng Cache tối đa".to_string(),
            "10G".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_flag_vfs_cache_max_age".to_string(),
            "Thời gian Cache tối đa".to_string(),
            "72h".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_flag_vfs_read_chunk_size".to_string(),
            "Kích thước đoạn đọc".to_string(),
            "32M".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_flag_vfs_read_chunk_size_limit".to_string(),
            "Giới hạn đoạn đọc".to_string(),
            "off".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_flag_dir_cache_time".to_string(),
            "Thời gian Cache thư mục".to_string(),
            "72h".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_flag_attr_timeout".to_string(),
            "Timeout thuộc tính".to_string(),
            "72h".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_flag_buffer_size".to_string(),
            "Kích thước buffer RAM".to_string(),
            "64M".to_string(),
            Vec::new(),
        ));
        fields.push((
            "_flag_allow_other".to_string(),
            "Cho phép User khác".to_string(),
            "".to_string(),
            vec!["".to_string(), "Có (yes)".to_string()],
        ));
        fields.push((
            "_flag_read_only".to_string(),
            "Chế độ chỉ đọc".to_string(),
            "".to_string(),
            vec!["".to_string(), "Có (yes)".to_string()],
        ));
        fields.push((
            "_flag_allow_non_empty".to_string(),
            "Cho phép thư mục chứa file".to_string(),
            "Có (yes)".to_string(),
            vec!["".to_string(), "Có (yes)".to_string()],
        ));

        fields.push(("[Unit]Description".to_string(), String::new(), "Rclone Mount Service".to_string(), Vec::new()));
        fields.push(("[Unit]After".to_string(), String::new(), "network-online.target".to_string(), Vec::new()));
        fields.push(("[Unit]Wants".to_string(), String::new(), "network-online.target".to_string(), Vec::new()));
        fields.push(("[Service]Type".to_string(), String::new(), "notify".to_string(), vec!["simple".to_string(), "forking".to_string(), "oneshot".to_string(), "dbus".to_string(), "notify".to_string(), "idle".to_string()]));
        fields.push(("[Service]ExecStart".to_string(), String::new(), String::new(), Vec::new()));
        fields.push(("[Service]ExecStop".to_string(), String::new(), String::new(), Vec::new()));
        fields.push(("[Service]Restart".to_string(), String::new(), "on-failure".to_string(), vec!["no".to_string(), "on-success".to_string(), "on-failure".to_string(), "on-abnormal".to_string(), "on-watchdog".to_string(), "on-abort".to_string(), "always".to_string()]));
        fields.push(("[Service]RestartSec".to_string(), String::new(), "10s".to_string(), Vec::new()));
        fields.push(("[Install]WantedBy".to_string(), String::new(), "default.target".to_string(), Vec::new()));

        fields
    }

    pub(crate) fn save_systemd_service_file(
        &mut self,
        _is_create: bool,
        _service_name: &str,
        file_path: &str,
        is_user: bool,
        fields: &[(String, String, String, Vec<String>)],
    ) -> std::io::Result<()> {
        let mut final_fields = fields.to_vec();

        let active_tab = match &self.services_state.wizard {
            ui::services::ServicesWizardState::EditSystemdService { active_tab, .. } => *active_tab,
            ui::services::ServicesWizardState::CreateSystemdService { active_tab, .. } => *active_tab,
            _ => 0,
        };

        if active_tab == 0 {
            let get_val = |key: &str| {
                final_fields
                    .iter()
                    .find(|(k, _, _, _)| k == key)
                    .map(|(_, _, v, _)| v.clone())
                    .unwrap_or_default()
            };

            let remote = get_val("_remote");
            let mount_path = get_val("_mount_path");
            let desc = get_val("_description");
            let user = get_val("_user");

            // Ensure mount point directory exists and is writable
            if !mount_path.is_empty() {
                let mut need_escalation = false;
                let is_drive = cfg!(windows) && (
                    (mount_path.len() == 2 && mount_path.ends_with(':'))
                    || (mount_path.len() == 3 && mount_path.ends_with(":\\"))
                    || (mount_path.len() == 3 && mount_path.ends_with(":/"))
                );

                if !is_drive {
                    if std::fs::create_dir_all(&mount_path).is_err() {
                        need_escalation = true;
                    } else {
                        let temp_file = std::path::Path::new(&mount_path).join(".rclone_tui_temp");
                        if std::fs::write(&temp_file, "").is_err() {
                            need_escalation = true;
                        } else {
                            let _ = std::fs::remove_file(temp_file);
                        }
                    }
                }

                #[cfg(unix)]
                if need_escalation {
                    self.services_state.info_message = Some("Yêu cầu xác thực quyền root để tạo và phân quyền thư mục mount...".to_string());
                    let _ = Command::new("pkexec")
                        .args(&["mkdir", "-p", &mount_path])
                        .status();

                    let username = std::process::Command::new("id")
                        .args(&["-u", "-n"])
                        .output()
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                        .unwrap_or_else(|_| "bimatkeo".to_string());

                    let groupname = std::process::Command::new("id")
                        .args(&["-g", "-n"])
                        .output()
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                        .unwrap_or_else(|_| "bimatkeo".to_string());

                    let owner_arg = format!("{}:{}", username, groupname);
                    let _ = Command::new("pkexec")
                        .args(&["chown", "-R", &owner_arg, &mount_path])
                        .status();
                }

                #[cfg(windows)]
                if need_escalation {
                    self.services_state.info_message = Some("Yêu cầu xác thực Administrator để tạo thư mục mount...".to_string());
                    let _ = Command::new("powershell")
                        .args(&[
                            "-NoProfile",
                            "-Command",
                            &format!("Start-Process cmd -ArgumentList '/c mkdir \"{}\"' -Verb RunAs -WindowStyle Hidden -Wait", mount_path)
                        ])
                        .status();
                }
            }

            let config_path = self.config.get_active_profile_path();
            let rclone_path = get_rclone_exec_path();

            let mut exec_start = format!(
                "{} mount {} {} --config {}",
                rclone_path, remote, mount_path, config_path
            );
            
            let vfs_cache_mode = get_val("_flag_vfs_cache_mode");
            if !vfs_cache_mode.is_empty() {
                exec_start.push_str(&format!(" --vfs-cache-mode {}", vfs_cache_mode));
            }
            let vfs_cache_max_size = get_val("_flag_vfs_cache_max_size");
            if !vfs_cache_max_size.is_empty() {
                exec_start.push_str(&format!(" --vfs-cache-max-size {}", vfs_cache_max_size));
            }
            let vfs_cache_max_age = get_val("_flag_vfs_cache_max_age");
            if !vfs_cache_max_age.is_empty() {
                exec_start.push_str(&format!(" --vfs-cache-max-age {}", vfs_cache_max_age));
            }
            let vfs_read_chunk_size = get_val("_flag_vfs_read_chunk_size");
            if !vfs_read_chunk_size.is_empty() {
                exec_start.push_str(&format!(" --vfs-read-chunk-size {}", vfs_read_chunk_size));
            }
            let vfs_read_chunk_size_limit = get_val("_flag_vfs_read_chunk_size_limit");
            if !vfs_read_chunk_size_limit.is_empty() {
                exec_start.push_str(&format!(" --vfs-read-chunk-size-limit {}", vfs_read_chunk_size_limit));
            }
            let dir_cache_time = get_val("_flag_dir_cache_time");
            if !dir_cache_time.is_empty() {
                exec_start.push_str(&format!(" --dir-cache-time {}", dir_cache_time));
            }
            let attr_timeout = get_val("_flag_attr_timeout");
            if !attr_timeout.is_empty() {
                exec_start.push_str(&format!(" --attr-timeout {}", attr_timeout));
            }
            let buffer_size = get_val("_flag_buffer_size");
            if !buffer_size.is_empty() {
                exec_start.push_str(&format!(" --buffer-size {}", buffer_size));
            }
            let allow_other = get_val("_flag_allow_other");
            if !allow_other.is_empty() {
                exec_start.push_str(" --allow-other");
            }
            let read_only = get_val("_flag_read_only");
            if !read_only.is_empty() {
                exec_start.push_str(" --read-only");
            }
            let allow_non_empty = get_val("_flag_allow_non_empty");
            if !allow_non_empty.is_empty() {
                exec_start.push_str(" --allow-non-empty");
            }
            let fusermount_path = get_fusermount_exec_path();
            let exec_stop = format!("{} -uz {}", fusermount_path, mount_path);

            let mut update_raw = |key: &str, val: &str| {
                if let Some(item) = final_fields.iter_mut().find(|(k, _, _, _)| k == key) {
                    item.2 = val.to_string();
                } else {
                    final_fields.push((key.to_string(), String::new(), val.to_string(), Vec::new()));
                }
            };

            update_raw("[Unit]Description", &desc);
            update_raw("[Service]ExecStart", &exec_start);
            update_raw("[Service]ExecStop", &exec_stop);

            if is_user {
                update_raw("[Install]WantedBy", "default.target");
                final_fields.retain(|(k, _, _, _)| k != "[Service]User" && k != "[Service]Group");
            } else {
                update_raw("[Service]User", &user);
                update_raw("[Service]Group", &user);
                update_raw("[Install]WantedBy", "multi-user.target");
            }
        }

        let home_dir = crate::app_config::get_home_dir();
        let temp_file_path = format!("{}/.config/rclone-tui-temp.service", home_dir);

        let mut sections: std::collections::BTreeMap<String, Vec<(String, String)>> = std::collections::BTreeMap::new();
        for (name, _, value, _) in &final_fields {
            if name.starts_with('_') {
                continue;
            }
            if name.starts_with('[') {
                if let Some(pos) = name.find(']') {
                    let sec = name[1..pos].to_string();
                    let key = name[pos + 1..].to_string();
                    sections.entry(sec).or_default().push((key, value.clone()));
                }
            }
        }

        let mut content = String::new();
        let ordered_sections = vec!["Unit", "Service", "Install"];
        for sec in ordered_sections {
            if let Some(keys) = sections.remove(sec) {
                content.push_str(&format!("[{}]\n", sec));
                for (k, v) in keys {
                    content.push_str(&format!("{}={}\n", k, v));
                }
                content.push_str("\n");
            }
        }
        for (sec, keys) in sections {
            content.push_str(&format!("[{}]\n", sec));
            for (k, v) in keys {
                content.push_str(&format!("{}={}\n", k, v));
            }
            content.push_str("\n");
        }

        std::fs::create_dir_all(format!("{}/.config", home_dir))?;
        std::fs::write(&temp_file_path, content)?;

        if is_user {
            let parent_dir = std::path::Path::new(file_path).parent().unwrap();
            std::fs::create_dir_all(parent_dir)?;
            std::fs::copy(&temp_file_path, file_path)?;
            let _ = std::fs::remove_file(&temp_file_path);

            let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).status();
        } else {
            let parent_dir = std::path::Path::new(file_path).parent().unwrap().to_string_lossy().to_string();
            let _ = Command::new("pkexec").args(["mkdir", "-p", &parent_dir]).status();

            let status = Command::new("pkexec")
                .args(["mv", &temp_file_path, file_path])
                .status()?;

            if !status.success() {
                return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "pkexec mv failed"));
            }

            let _ = Command::new("pkexec").args(["chown", "root:root", file_path]).status();
            let _ = Command::new("pkexec").args(["chmod", "644", file_path]).status();
            let _ = Command::new("pkexec").args(["systemctl", "daemon-reload"]).status();
        }

        Ok(())
    }

    pub(crate) fn ensure_mount_point_exists_from_service_file(&self, file_path: &str) {
        if let Ok(content) = std::fs::read_to_string(file_path) {
            for line in content.lines() {
                let is_mount = line.contains(" mount ");
                let is_nfsmount = line.contains(" nfsmount ");
                if line.starts_with("ExecStart=") && (is_mount || is_nfsmount) {
                    let mount_pos = if is_mount {
                        line.find(" mount ").unwrap()
                    } else {
                        line.find(" nfsmount ").unwrap()
                    };
                    let mount_word_len = if is_mount { 7 } else { 10 };
                    let after_mount = &line[mount_pos + mount_word_len..];
                        let mut args = Vec::new();
                        let mut current = String::new();
                        let mut in_quotes = false;
                        let mut quote_char = ' ';
                        for c in after_mount.chars() {
                            if (c == '"' || c == '\'') && !in_quotes {
                                in_quotes = true;
                                quote_char = c;
                            } else if c == quote_char && in_quotes {
                                in_quotes = false;
                            } else if c == ' ' && !in_quotes {
                                if !current.is_empty() {
                                    args.push(current.clone());
                                    current.clear();
                                }
                            } else {
                                current.push(c);
                            }
                        }
                        if !current.is_empty() {
                            args.push(current);
                        }

                        if args.len() >= 2 {
                            let mount_path = &args[1];
                            if mount_path.starts_with('/') {
                                let mut need_sudo = false;
                                if std::fs::create_dir_all(mount_path).is_err() {
                                    need_sudo = true;
                                } else {
                                    let temp_file = std::path::Path::new(mount_path).join(".rclone_tui_temp");
                                    if std::fs::write(&temp_file, "").is_err() {
                                        need_sudo = true;
                                    } else {
                                        let _ = std::fs::remove_file(temp_file);
                                    }
                                }

                                if need_sudo {
                                    let _ = Command::new("pkexec")
                                        .args(&["mkdir", "-p", mount_path])
                                        .status();

                                    let username = std::process::Command::new("id")
                                        .args(&["-u", "-n"])
                                        .output()
                                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                                        .unwrap_or_else(|_| "bimatkeo".to_string());

                                    let groupname = std::process::Command::new("id")
                                        .args(&["-g", "-n"])
                                        .output()
                                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                                        .unwrap_or_else(|_| "bimatkeo".to_string());

                                    let owner_arg = format!("{}:{}", username, groupname);
                                    let _ = Command::new("pkexec")
                                        .args(&["chown", "-R", &owner_arg, mount_path])
                                        .status();
                                }
                            }
                        }
                    }
                }
            }
        }

    pub(crate) fn get_systemd_error_logs(&self, service_name: &str, is_user: bool) -> String {
        let mut cmd = Command::new("journalctl");
        if is_user {
            cmd.args(["--user", "-u", service_name, "-n", "10", "--no-pager"]);
        } else {
            cmd.args(["-u", service_name, "-n", "10", "--no-pager"]);
        }
        if let Ok(output) = cmd.output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut lines = Vec::new();
            for line in stdout.lines() {
                if line.contains("error") || line.contains("Error") || line.contains("Fatal") || line.contains("failed") {
                    lines.push(line.trim().to_string());
                }
            }
            if !lines.is_empty() {
                return lines.join("\n");
            }
            let all_lines: Vec<&str> = stdout.lines().collect();
            if all_lines.len() >= 3 {
                return all_lines[all_lines.len() - 3..].join("\n");
            } else {
                return stdout.into_owned();
            }
        }
        String::new()
    }

    /// Đọc danh sách tiến trình chạy ngầm bằng cách quét tiến trình thực tế
    pub fn load_active_services_from_file(&mut self) {
        self.scan_running_services();
        self.scan_systemd_services();
    }

    /// Lưu danh sách tiến trình chạy ngầm (không cần thiết nữa vì quét động)
    #[allow(dead_code)]
    pub fn save_active_services_to_file(&self) {
        // Trống
    }

    pub(crate) fn execute_launch_service(
        &mut self,
        service_type: ui::services::ServiceType,
        remote: String,
        path: String,
        protocol: Option<String>,
        flags: Vec<(String, String, String, String)>,
    ) {
        self.services_state.wizard = ui::services::ServicesWizardState::None;

        let config_path = self.config.get_active_profile_path();

        let mut args = vec![];

        match service_type {
            ui::services::ServiceType::Mount | ui::services::ServiceType::NfsMount => {
                let cmd_name = if service_type == ui::services::ServiceType::NfsMount {
                    "nfsmount"
                } else {
                    "mount"
                };
                args.push(cmd_name.to_string());
                
                // remote + path
                let r_path = format!("{}{}", remote, path);
                args.push(r_path);
 
                // Mount point
                let local_mnt = flags[0].3.clone();
                let mut need_escalation = false;
                let is_drive = cfg!(windows) && (
                    (local_mnt.len() == 2 && local_mnt.ends_with(':'))
                    || (local_mnt.len() == 3 && local_mnt.ends_with(":\\"))
                    || (local_mnt.len() == 3 && local_mnt.ends_with(":/"))
                );

                if !is_drive {
                    if fs::create_dir_all(&local_mnt).is_err() {
                        need_escalation = true;
                    } else {
                        let temp_file = Path::new(&local_mnt).join(".rclone_tui_temp");
                        if fs::write(&temp_file, "").is_err() {
                            need_escalation = true;
                        } else {
                            let _ = fs::remove_file(temp_file);
                        }
                    }
                }
 
                #[cfg(unix)]
                if need_escalation {
                    self.services_state.info_message = Some("Yêu cầu xác thực quyền root để tạo và phân quyền thư mục mount...".to_string());
                    let _ = Command::new("pkexec")
                        .args(&["mkdir", "-p", &local_mnt])
                        .status();
 
                    let username = std::process::Command::new("id")
                        .args(&["-u", "-n"])
                        .output()
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                        .unwrap_or_else(|_| "bimatkeo".to_string());
 
                    let groupname = std::process::Command::new("id")
                        .args(&["-g", "-n"])
                        .output()
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                        .unwrap_or_else(|_| "bimatkeo".to_string());
 
                    let owner_arg = format!("{}:{}", username, groupname);
                    let _ = Command::new("pkexec")
                        .args(&["chown", "-R", &owner_arg, &local_mnt])
                        .status();
                }

                #[cfg(windows)]
                if need_escalation {
                    self.services_state.info_message = Some("Yêu cầu xác thực Administrator để tạo thư mục mount...".to_string());
                    let _ = Command::new("powershell")
                        .args(&[
                            "-NoProfile",
                            "-Command",
                            &format!("Start-Process cmd -ArgumentList '/c mkdir \"{}\"' -Verb RunAs -WindowStyle Hidden -Wait", local_mnt)
                        ])
                        .status();
                }
 
                args.push(local_mnt.clone());
 
                args.push(format!("--config={}", config_path));
 
                // --vfs-cache-mode
                let vfs_cache = if flags.len() > 1 { flags[1].3.clone() } else { self.config.default_vfs_cache_mode.clone() };
                if !vfs_cache.is_empty() {
                    args.push(format!("--vfs-cache-mode={}", vfs_cache));
                }
 
                // --dir-cache-time
                let dir_cache = if flags.len() > 2 { flags[2].3.clone() } else { self.config.default_dir_cache_time.clone() };
                if !dir_cache.is_empty() {
                    args.push(format!("--dir-cache-time={}", dir_cache));
                }
 
                // --read-only
                let read_only = if flags.len() > 3 { flags[3].3.to_lowercase() } else { "n".to_string() };
                if read_only == "y" {
                    args.push("--read-only".to_string());
                }
 
                // --daemon
                #[cfg(unix)]
                {
                    let daemon = if flags.len() > 4 { flags[4].3.to_lowercase() } else { "y".to_string() };
                    if daemon == "y" {
                        args.push("--daemon".to_string());
                    }
                }
 
                // Chạy lệnh mount độc lập dưới quyền user hiện tại
                let rclone_cmd = get_rclone_cmd();
                #[cfg(unix)]
                let child = Command::new("setsid").arg(&rclone_cmd).args(&args).spawn();
                #[cfg(not(unix))]
                let child = Command::new(&rclone_cmd).args(&args).spawn();
                match child {
                    Ok(c) => {
                        let pid = c.id();
                        self.services_state
                            .active_services
                            .push(ui::services::ActiveService {
                                service_type_str: if service_type == ui::services::ServiceType::NfsMount {
                                    "NfsMount".to_string()
                                } else {
                                    "Mount".to_string()
                                },
                                remote,
                                path: local_mnt.clone(),
                                pid,
                                details: format!("{} -> {}", if service_type == ui::services::ServiceType::NfsMount { "NfsMount" } else { "Mount" }, local_mnt),
                            });
                        // Chờ một chút để tiến trình daemon khởi chạy (nếu có)
                        std::thread::sleep(std::time::Duration::from_millis(150));
                        self.scan_running_services();
                        self.services_state.info_message =
                            Some(format!("Khởi chạy {} thành công. PID: {}", cmd_name, pid));
                    }
                    Err(e) => {
                        self.services_state.error_message = Some(format!("Không thể {}: {}", cmd_name, e));
                    }
                }
            }
            ui::services::ServiceType::WebGui => {
                args.push("rcd".to_string());
                args.push("--rc-web-gui".to_string());
                args.push(format!("--config={}", config_path));

                let rc_addr = flags[0].3.clone();
                args.push(format!("--rc-addr={}", rc_addr));

                if flags[1].3.to_lowercase() == "y" {
                    args.push("--rc-no-auth".to_string());
                }

                let rclone_cmd = get_rclone_cmd();
                let child = Command::new(&rclone_cmd).args(&args).spawn();
                match child {
                    Ok(c) => {
                        let pid = c.id();
                        self.services_state
                            .active_services
                            .push(ui::services::ActiveService {
                                service_type_str: "WebGui".to_string(),
                                remote: String::new(),
                                path: rc_addr.clone(),
                                pid,
                                details: format!("Cổng Web: {}", rc_addr),
                            });
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        self.scan_running_services();
                        self.services_state.info_message = Some(format!(
                            "Web GUI hoạt động trên cổng {}. PID: {}",
                            rc_addr, pid
                        ));
                        // Tự động mở trình duyệt Web GUI
                        let _ = webbrowser::open(&format!("http://{}", rc_addr));
                    }
                    Err(e) => {
                        self.services_state.error_message = Some(format!("Lỗi bật Web GUI: {}", e));
                    }
                }
            }
            ui::services::ServiceType::Serve => {
                let proto = protocol.unwrap_or_else(|| "http".to_string());
                args.push("serve".to_string());
                args.push(proto.clone());
                args.push(format!("{}{}", remote, path));
                args.push(format!("--config={}", config_path));

                let addr = flags[0].3.clone();
                args.push(format!("--addr={}", addr));

                let user = flags[1].3.clone();
                if !user.is_empty() {
                    args.push(format!("--user={}", user));
                }

                let pass = flags[2].3.clone();
                if !pass.is_empty() {
                    args.push(format!("--pass={}", pass));
                }

                if flags[3].3.to_lowercase() == "y" {
                    args.push("--read-only".to_string());
                }

                let rclone_cmd = get_rclone_cmd();
                let child = Command::new(&rclone_cmd).args(&args).spawn();
                match child {
                    Ok(c) => {
                        let pid = c.id();
                        self.services_state
                            .active_services
                            .push(ui::services::ActiveService {
                                service_type_str: "Serve".to_string(),
                                remote,
                                path: addr.clone(),
                                pid,
                                details: format!("{}://{}", proto, addr),
                            });
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        self.scan_running_services();
                        self.services_state.info_message = Some(format!(
                            "Máy chủ chia sẻ {} đang chạy tại {}. PID: {}",
                            proto, addr, pid
                        ));
                    }
                    Err(e) => {
                        self.services_state.error_message =
                            Some(format!("Lỗi chia sẻ server: {}", e));
                    }
                }
            }
        }
    }
}
