use std::process::Command;

pub fn kill_process_by_pid(pid: u32, is_mount: bool, mount_path: &str) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let _ = Command::new("kill").arg(pid.to_string()).status();
        if is_mount && !mount_path.is_empty() {
            // Cố gắng unmount point bằng fusermount3 hoặc fusermount
            let _ = Command::new("fusermount3").args(["-uz", mount_path]).status();
            let _ = Command::new("fusermount").args(["-uz", mount_path]).status();
        }
    }
    #[cfg(not(unix))]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .status();
    }
    Ok(())
}
