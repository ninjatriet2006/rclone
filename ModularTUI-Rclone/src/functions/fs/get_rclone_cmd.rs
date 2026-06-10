pub fn get_rclone_cmd() -> String {
    if let Ok(mut exe_path) = std::env::current_exe() {
        exe_path.pop(); // Thư mục chứa file exe hiện tại
        let local_rclone = if cfg!(target_os = "windows") {
            exe_path.join("rclone.exe")
        } else {
            exe_path.join("rclone")
        };
        if local_rclone.exists() {
            return local_rclone.to_string_lossy().to_string();
        }
    }
    "rclone".to_string()
}
