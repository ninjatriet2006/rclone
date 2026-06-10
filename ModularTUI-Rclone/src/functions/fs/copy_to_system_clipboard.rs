use std::process::Command;

pub fn copy_to_system_clipboard(text: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if Command::new("clip").stdin(std::process::Stdio::piped()).spawn().is_ok() {
            let mut child = Command::new("clip")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            }
            let _ = child.wait();
            return Ok(());
        }
    }

    #[cfg(target_os = "macos")]
    {
        if Command::new("pbcopy").stdin(std::process::Stdio::piped()).spawn().is_ok() {
            let mut child = Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            }
            let _ = child.wait();
            return Ok(());
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if Command::new("xclip").arg("-selection").arg("clipboard").stdin(std::process::Stdio::piped()).spawn().is_ok() {
            let mut child = Command::new("xclip")
                .arg("-selection")
                .arg("clipboard")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            }
            let _ = child.wait();
            return Ok(());
        }
        if Command::new("xsel").arg("--clipboard").arg("--input").stdin(std::process::Stdio::piped()).spawn().is_ok() {
            let mut child = Command::new("xsel")
                .arg("--clipboard")
                .arg("--input")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            }
            let _ = child.wait();
            return Ok(());
        }
        if Command::new("wl-copy").stdin(std::process::Stdio::piped()).spawn().is_ok() {
            let mut child = Command::new("wl-copy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            }
            let _ = child.wait();
            return Ok(());
        }
    }
    Err("Không tìm thấy tiện ích clipboard nào trên hệ thống".to_string())
}
