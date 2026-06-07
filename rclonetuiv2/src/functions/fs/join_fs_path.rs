pub fn join_fs_path(fs: &str, sub_path: &str) -> String {
    if fs.contains(':') {
        let parts: Vec<&str> = fs.splitn(2, ':').collect();
        let remote = parts[0];
        let path = parts[1];
        let joined_path = if path.is_empty() {
            sub_path.to_string()
        } else if path.ends_with('/') {
            format!("{}{}", path, sub_path)
        } else {
            format!("{}/{}", path, sub_path)
        };
        format!("{}:{}", remote, joined_path)
    } else {
        if fs.ends_with('/') {
            format!("{}{}", fs, sub_path)
        } else {
            format!("{}/{}", fs, sub_path)
        }
    }
}
