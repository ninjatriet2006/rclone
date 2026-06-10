use std::path::Path;
use crate::functions::app_config::get_home_dir::get_home_dir;

pub fn get_rclone_tui_conf() -> String {
    if cfg!(target_os = "windows") {
        if let Ok(appdata) = std::env::var("APPDATA") {
            Path::new(&appdata).join("rclone-tui").join("rclone_tui.config").to_string_lossy().to_string()
        } else {
            Path::new(&get_home_dir()).join(".config").join("rclone-tui").join("rclone_tui.config").to_string_lossy().to_string()
        }
    } else if cfg!(target_os = "macos") {
        let home = get_home_dir();
        Path::new(&home)
            .join("Library")
            .join("Application Support")
            .join("rclone-tui")
            .join("rclone_tui.config")
            .to_string_lossy()
            .to_string()
    } else {
        let home = get_home_dir();
        format!("{}/.config/rclone-tui/rclone_tui.config", home)
    }
}
