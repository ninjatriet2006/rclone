use std::env;
use std::io::{self, IsTerminal};
use std::process::Command;

pub fn check_terminal_wrapping() {
    if env::var("RCLONE_TUI_WRAPPED").is_ok() {
        return;
    }

    if !io::stdout().is_terminal() {
        #[cfg(target_os = "macos")]
        {
            let current_exe = env::current_exe().unwrap();
            let current_exe_str = current_exe.to_str().unwrap();
            let args: Vec<String> = env::args().skip(1).collect();
            let args_str = if args.is_empty() {
                String::new()
            } else {
                format!(" {}", args.join(" "))
            };
            let script = format!(
                "tell application \"Terminal\" to do script \"export RCLONE_TUI_WRAPPED=1 && exec '{}'{}\"",
                current_exe_str, args_str
            );
            let status = Command::new("osascript")
                .args(["-e", &script])
                .spawn();
            if status.is_ok() {
                std::process::exit(0);
            }
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let terminals = [
                "gnome-terminal",
                "konsole",
                "xfce4-terminal",
                "alacritty",
                "kitty",
                "xterm",
            ];
            for term in terminals {
                if which::which(term).is_ok() {
                    let current_exe = env::current_exe().unwrap();
                    let status = match term {
                        "gnome-terminal" => Command::new(term)
                            .env("RCLONE_TUI_WRAPPED", "1")
                            .args(["--", current_exe.to_str().unwrap()])
                            .spawn(),
                        _ => Command::new(term)
                            .env("RCLONE_TUI_WRAPPED", "1")
                            .args(["-e", current_exe.to_str().unwrap()])
                            .spawn(),
                    };
                    if status.is_ok() {
                        std::process::exit(0);
                    }
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            let terminals = ["wt", "cmd"];
            for term in terminals {
                if which::which(term).is_ok() {
                    let current_exe = env::current_exe().unwrap();
                    let status = match term {
                        "wt" => Command::new(term)
                            .env("RCLONE_TUI_WRAPPED", "1")
                            .args([current_exe.to_str().unwrap()])
                            .spawn(),
                        _ => Command::new(term)
                            .env("RCLONE_TUI_WRAPPED", "1")
                            .args(["/c", "start", "", current_exe.to_str().unwrap()])
                            .spawn(),
                    };
                    if status.is_ok() {
                        std::process::exit(0);
                    }
                }
            }
        }

        eprintln!("Cảnh báo: Không phát hiện TTY và không tìm thấy Terminal Emulator tương thích.");
        std::process::exit(1);
    }
}
