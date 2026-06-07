mod app;
mod app_config;
mod lang;
mod rclone;
mod ui;

use std::env;
use std::io::{self, IsTerminal, Write};
use std::panic;
use std::process::Command;

fn check_terminal_wrapping() {
    // Nếu biến môi trường RCLONE_TUI_WRAPPED=1 đã được gán, không chạy lặp lại (Tránh Bug 3, 51)
    if env::var("RCLONE_TUI_WRAPPED").is_ok() {
        return;
    }

    // Kiểm tra xem stdout có phải là TTY không
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
            // Tìm các terminal emulator phổ biến trên Linux/Unix
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
            // Tìm Windows Terminal hoặc cmd trên Windows
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

        // Nếu không tìm thấy emulator nào
        eprintln!("Cảnh báo: Không phát hiện TTY và không tìm thấy Terminal Emulator tương thích.");
        std::process::exit(1);
    }
}

fn check_fuse_dependency() -> bool {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        which::which("fusermount3").is_ok() || which::which("fusermount").is_ok()
    }
    #[cfg(target_os = "macos")]
    {
        std::path::Path::new("/Library/Filesystems/macfuse.fs").exists()
            || std::path::Path::new("/Library/Filesystems/osxfuse.fs").exists()
            || which::which("fuse-t").is_ok()
    }
    #[cfg(windows)]
    {
        std::env::var("WinfspDir").is_ok()
            || std::path::Path::new("C:\\Program Files (x86)\\WinFsp").exists()
            || std::path::Path::new("C:\\Program Files\\WinFsp").exists()
    }
}

fn ensure_dependencies() {
    if !check_fuse_dependency() {
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            println!("------------------------------------------------------------------");
            println!("CẢNH BÁO: Không phát hiện công cụ FUSE (fusermount3/fusermount) trên hệ thống.");
            println!("FUSE là bắt buộc để sử dụng chức năng Mount ổ đĩa ảo.");
            print!("Bạn có muốn tự động cài đặt fuse3 ngay bây giờ? (Cần mật khẩu sudo) [y/N]: ");
            let _ = io::stdout().flush();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_ok() {
                let choice = input.trim().to_lowercase();
                if choice == "y" || choice == "yes" {
                    println!("Đang chạy lệnh cập nhật và cài đặt fuse3...");
                    // Gọi sudo apt-get update && apt-get install (Bug 4, 52, 98)
                    let status = Command::new("sudo").args(["apt-get", "update"]).status();
                    if status.is_ok() {
                        let status2 = Command::new("sudo")
                            .args(["apt-get", "install", "-y", "fuse3"])
                            .status();
                        if status2.is_ok() && status2.unwrap().success() {
                            println!("Cài đặt fuse3 thành công!");
                        } else {
                            println!("Lỗi: Cài đặt fuse3 thất bại. Chức năng Mount sẽ bị hạn chế.");
                            print!("Nhấn Enter để tiếp tục...");
                            let _ = io::stdout().flush();
                            let _ = io::stdin().read_line(&mut String::new());
                        }
                    } else {
                        println!("Lỗi: Không thể chạy sudo. Chức năng Mount sẽ bị hạn chế.");
                        print!("Nhấn Enter để tiếp tục...");
                        let _ = io::stdout().flush();
                        let _ = io::stdin().read_line(&mut String::new());
                    }
                } else {
                    println!("Bỏ qua cài đặt FUSE. Chức năng Mount sẽ bị hạn chế.");
                    print!("Nhấn Enter để tiếp tục...");
                    let _ = io::stdout().flush();
                    let _ = io::stdin().read_line(&mut String::new());
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            println!("------------------------------------------------------------------");
            println!("CẢNH BÁO: Không phát hiện thư viện FUSE tương thích trên hệ thống.");
            println!("FUSE là bắt buộc để sử dụng chức năng Mount ổ đĩa ảo trên macOS.");
            println!("Để sử dụng chức năng này, vui lòng cài đặt một trong các lựa chọn sau:");
            println!("1. Cài đặt macFUSE từ https://macfuse.io/ (Khuyên dùng)");
            println!("   Hoặc cài đặt thông qua Homebrew: brew install --cask macfuse");
            println!("   LƯU Ý: Với máy chip Apple Silicon (M1/M2/M3+), bạn cần vào");
            println!("   chế độ Recovery Mode để bật nạp Kernel Extension bên thứ ba.");
            println!("2. Sử dụng FUSE-T (Không cần Kernel Extension/Recovery Mode):");
            println!("   Chi tiết xem tại: https://github.com/macos-fuse-t/fuse-t");
            println!("------------------------------------------------------------------");
            print!("Nhấn Enter để tiếp tục...");
            let _ = io::stdout().flush();
            let _ = io::stdin().read_line(&mut String::new());
        }

        #[cfg(windows)]
        {
            println!("------------------------------------------------------------------");
            println!("CẢNH BÁO: Không phát hiện tiện ích WinFsp (Windows File System Proxy) trên hệ thống.");
            println!("WinFsp là bắt buộc đối với rclone để thực hiện chức năng Mount ổ đĩa ảo trên Windows.");
            println!("Vui lòng tải và cài đặt WinFsp từ trang chủ: https://winfsp.dev/");
            print!("Nhấn Enter để tiếp tục...");
            let _ = io::stdout().flush();
            let _ = io::stdin().read_line(&mut String::new());
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Tự động thiết lập RCLONE_LOG_FILE và RCLONE_LOG_LEVEL và tự khởi động lại tiến trình (re-execute)
    // nếu biến môi trường chưa tồn tại, nhằm đảm bảo Go runtime của librclone không in log ra stderr hỏng TUI.
    if std::env::var("RCLONE_LOG_FILE").is_err() {
        let log_path = app_config::AppConfig::config_dir().join("rclone.log");
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let current_exe = std::env::current_exe().expect("Failed to get current exe path");
        let args: Vec<String> = std::env::args().collect();
        let status = std::process::Command::new(current_exe)
            .args(&args[1..])
            .env("RCLONE_LOG_FILE", &log_path)
            .env("RCLONE_LOG_LEVEL", "NOTICE")
            .status();
        match status {
            Ok(s) => std::process::exit(s.code().unwrap_or(0)),
            Err(_) => std::process::exit(1),
        }
    }

    // 2. Kiểm tra Terminal Wrapping (TTY)
    check_terminal_wrapping();

    // 3. Kiểm tra và cài đặt dependencies (FUSE)
    ensure_dependencies();

    // 4. Khởi tạo Rclone Core Engine
    rclone::initialize();

    // Kích hoạt Metadata toàn cục để rclone trả về Metadata.copy-requires-writer-permission cho Google Drive
    let set_param = serde_json::json!({
        "main": {
            "Metadata": true
        }
    }).to_string();
    let _ = rclone::rpc_async("options/set".to_string(), set_param).await;

    // Thiết lập panic hook để dọn dẹp raw mode nếu app crash (Bug 6, 34)
    let default_panic = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen);
        default_panic(info);
    }));

    // 4. Khởi chạy App UI Event Loop
    let mut app = app::App::new();
    let res = app.run().await;

    // 5. Giải phóng Rclone Core Engine
    rclone::finalize();

    if let Err(err) = res {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen);
        eprintln!("Ứng dụng kết thúc với lỗi: {:?}", err);
    }

    Ok(())
}
