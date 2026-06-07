use crate::functions::ActiveService;
use crate::functions::daemon::parse_rclone_args::parse_rclone_args;
use crate::functions::fs::parse_cmdline::parse_cmdline;
use std::collections::HashMap;
use std::fs;

pub fn scan_running_services(
    profiles: &HashMap<String, String>,
    active_profile: &str,
) -> Vec<ActiveService> {
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
                                        if let Some(service) = parse_rclone_args(pid, &args, profiles, active_profile) {
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
                for line in stdout.lines().skip(1) {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    if let Some(pos) = line.find(' ') {
                        let pid_str = &line[..pos];
                        let cmdline = &line[pos..].trim();
                        if let Ok(pid) = pid_str.parse::<u32>() {
                            let args = parse_cmdline(cmdline);
                            if let Some(service) = parse_rclone_args(pid, &args, profiles, active_profile) {
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
                        if let Some(service) = parse_rclone_args(pid, &args, profiles, active_profile) {
                            scanned_services.push(service);
                        }
                        current_cmdline.clear();
                        current_pid = None;
                    }
                }
            }
        }

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
                            if let Some(service) = parse_rclone_args(pid, &args, profiles, active_profile) {
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

    scanned_services
}
