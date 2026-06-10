use crate::app::{App, Screen};
use crossterm::event::{KeyEvent, KeyCode};
use std::io::Write;

pub async fn handle_dependency_keys(
    app: &mut App,
    key: KeyEvent,
) {
    match key.code {
        KeyCode::Esc => {
            app.screen = Screen::MainMenu;
        }
        KeyCode::Up => {
            if app.selected_dependency_idx > 0 {
                app.selected_dependency_idx -= 1;
            } else {
                app.selected_dependency_idx = 1;
            }
        }
        KeyCode::Down => {
            if app.selected_dependency_idx < 1 {
                app.selected_dependency_idx += 1;
            } else {
                app.selected_dependency_idx = 0;
            }
        }
        KeyCode::Enter => {
            let idx = app.selected_dependency_idx;
            if idx == 0 {
                // Cài đặt FUSE
                #[cfg(all(unix, not(target_os = "macos")))]
                {
                    let _ = crossterm::terminal::disable_raw_mode();
                    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
                    
                    println!("Đang chạy lệnh cập nhật và cài đặt fuse3...");
                    let status = std::process::Command::new("sudo").args(["apt-get", "update"]).status();
                    if status.is_ok() {
                        let status2 = std::process::Command::new("sudo")
                            .args(["apt-get", "install", "-y", "fuse3"])
                            .status();
                        if status2.is_ok() && status2.unwrap().success() {
                            println!("Cài đặt fuse3 thành công!");
                            app.fuse_installed = true;
                        } else {
                            println!("Lỗi: Cài đặt fuse3 thất bại.");
                        }
                    } else {
                        println!("Lỗi: Không thể chạy sudo.");
                    }
                    
                    println!("\nNhấn Enter để quay lại...");
                    let _ = std::io::stdout().flush();
                    let _ = std::io::stdin().read_line(&mut String::new());
                    
                    let _ = crossterm::terminal::enable_raw_mode();
                    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen);
                    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::All));
                }
                #[cfg(target_os = "macos")]
                {
                    let _ = crossterm::terminal::disable_raw_mode();
                    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
                    println!("------------------------------------------------------------------");
                    println!("Không phát hiện thư viện FUSE tương thích trên hệ thống.");
                    println!("FUSE là bắt buộc để sử dụng chức năng Mount ổ đĩa ảo trên macOS.");
                    println!("Để sử dụng chức năng này, vui lòng cài đặt một trong các lựa chọn sau:");
                    println!("1. Cài đặt macFUSE từ https://macfuse.io/ (Khuyên dùng)");
                    println!("   Hoặc cài đặt thông qua Homebrew: brew install --cask macfuse");
                    println!("   LƯU Ý: Với máy chip Apple Silicon (M1/M2/M3+), bạn cần vào");
                    println!("   chế độ Recovery Mode để bật nạp Kernel Extension bên thứ ba.");
                    println!("2. Sử dụng FUSE-T (Không cần Kernel Extension/Recovery Mode):");
                    println!("   Chi tiết xem tại: https://github.com/macos-fuse-t/fuse-t");
                    println!("------------------------------------------------------------------");
                    println!("\nNhấn Enter để quay lại...");
                    let _ = std::io::stdout().flush();
                    let _ = std::io::stdin().read_line(&mut String::new());
                    let _ = crossterm::terminal::enable_raw_mode();
                    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen);
                    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::All));
                }
                #[cfg(windows)]
                {
                    let _ = crossterm::terminal::disable_raw_mode();
                    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
                    println!("------------------------------------------------------------------");
                    println!("Không phát hiện tiện ích WinFsp (Windows File System Proxy) trên hệ thống.");
                    println!("WinFsp là bắt buộc đối với rclone để thực hiện chức năng Mount ổ đĩa ảo trên Windows.");
                    println!("Vui lòng tải và cài đặt WinFsp từ trang chủ: https://winfsp.dev/");
                    println!("------------------------------------------------------------------");
                    println!("\nNhấn Enter để quay lại...");
                    let _ = std::io::stdout().flush();
                    let _ = std::io::stdin().read_line(&mut String::new());
                    let _ = crossterm::terminal::enable_raw_mode();
                    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen);
                    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::All));
                }
            } else if idx == 1 {
                // Cài đặt Filen CLI
                let _ = crossterm::terminal::disable_raw_mode();
                let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
                
                println!("Đang chạy lệnh cài đặt Filen CLI...");
                let status = std::process::Command::new("sh")
                    .arg("-c")
                    .arg("curl -sL https://filen.io/cli.sh | bash")
                    .status();
                    
                if status.is_ok() && status.unwrap().success() {
                    println!("Cài đặt Filen CLI thành công!");
                    app.filen_cli_installed = true;
                } else {
                    println!("Lỗi: Cài đặt Filen CLI thất bại.");
                }
                
                println!("\nNhấn Enter để quay lại...");
                let _ = std::io::stdout().flush();
                let _ = std::io::stdin().read_line(&mut String::new());
                
                let _ = crossterm::terminal::enable_raw_mode();
                let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen);
                let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::All));
            }
        }
        _ => {}
    }
}
