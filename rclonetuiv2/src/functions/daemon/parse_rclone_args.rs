use crate::functions::ActiveService;
use crate::functions::daemon::detect_systemd_service::detect_systemd_service;
use crate::functions::rclone::get_underlying_remote::get_underlying_remote;
use crate::functions::fs::parse_cmdline::parse_cmdline;
use std::collections::HashMap;

pub fn parse_rclone_args(
    pid: u32,
    args: &[String],
    profiles: &HashMap<String, String>,
    active_profile: &str,
) -> Option<ActiveService> {
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

    let systemd_info = detect_systemd_service(pid);

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
            for (name, path) in profiles {
                if path == &config_path {
                    profile_name = name.clone();
                    break;
                }
            }
        } else {
            profile_name = active_profile.to_string();
        }
        let profile_prefix = if profile_name == "default" {
            String::new()
        } else {
            format!("{}: -> ", profile_name)
        };
        let details = if remote.is_empty() {
            format!("{}{}", profile_prefix, local_mnt)
        } else {
            let display_remote = if let Some(und) = get_underlying_remote(&config_path, &remote) {
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

        Some(ActiveService {
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
            for (name, path) in profiles {
                if path == &config_path {
                    profile_name = name.clone();
                    break;
                }
            }
        } else {
            profile_name = active_profile.to_string();
        }
        let profile_prefix = if profile_name == "default" {
            String::new()
        } else {
            format!("{}: -> ", profile_name)
        };
        let details = if remote_path.is_empty() {
            format!("{}{}{}", profile_prefix, proto, addr)
        } else {
            let display_remote = if let Some(und) = get_underlying_remote(&config_path, &remote_path) {
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

        Some(ActiveService {
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
            for (name, path) in profiles {
                if path == &config_path {
                    profile_name = name.clone();
                    break;
                }
            }
        } else {
            profile_name = active_profile.to_string();
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

        Some(ActiveService {
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
            Some(ActiveService {
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
