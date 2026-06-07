#![allow(dead_code, unused_imports, unused_variables)]

mod app;
mod functions;

use std::env;
use std::io::{self, Write};
use std::panic;
use std::process::Command;

fn ensure_dependencies() {
    if !crate::functions::check_fuse_dependency() {
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
        let log_path = crate::functions::AppConfig::config_dir().join("rclone.log");
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
    crate::functions::check_terminal_wrapping();

    // 3. Kiểm tra và cài đặt dependencies (FUSE)
    ensure_dependencies();

    // 4. Khởi tạo Rclone Core Engine
    crate::functions::initialize();

    // Kích hoạt Metadata toàn cục để rclone trả về Metadata.copy-requires-writer-permission cho Google Drive
    let set_param = serde_json::json!({
        "main": {
            "Metadata": true
        }
    }).to_string();
    let _ = crate::functions::rpc_async("options/set".to_string(), set_param).await;

    // Thiết lập panic hook để dọn dẹp raw mode nếu app crash (Bug 6, 34)
    let default_panic = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen);
        default_panic(info);
    }));

    // 5. Khởi chạy App UI Event Loop
    let mut app = app::App::new();
    let res = app.run().await;

    // 6. Giải phóng Rclone Core Engine
    crate::functions::finalize();

    if let Err(err) = res {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen);
        eprintln!("Ứng dụng kết thúc với lỗi: {:?}", err);
    }

    Ok(())
}
