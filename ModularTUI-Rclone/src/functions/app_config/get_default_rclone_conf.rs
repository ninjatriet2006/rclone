use std::path::Path;
use crate::functions::app_config::get_home_dir::get_home_dir;

pub fn get_default_rclone_conf() -> String {
    if cfg!(target_os = "windows") {
        if let Ok(appdata) = std::env::var("APPDATA") {
            Path::new(&appdata).join("rclone").join("rclone.conf").to_string_lossy().to_string()
        } else {
            Path::new(&get_home_dir()).join(".config").join("rclone").join("rclone.conf").to_string_lossy().to_string()
        }
    } else {
        let home = get_home_dir();
        format!("{}/.config/rclone/rclone.conf", home)
    }
}
