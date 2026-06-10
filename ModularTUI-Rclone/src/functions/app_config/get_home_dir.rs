pub fn get_home_dir() -> String {
    if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| "C:\\Users\\Default".to_string())
    } else {
        std::env::var("HOME").unwrap_or_else(|_| "/home/bimatkeo".to_string())
    }
}
