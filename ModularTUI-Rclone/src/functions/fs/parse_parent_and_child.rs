pub fn parse_parent_and_child(fs: &str) -> (String, String) {
    let (remote_part, path_part) = if let Some(idx) = fs.find(':') {
        (format!("{}:", &fs[..idx]), &fs[idx+1..])
    } else {
        (String::new(), fs)
    };
    if let Some(idx) = path_part.rfind('/') {
        let parent = &path_part[..idx];
        let name = &path_part[idx+1..];
        (format!("{}{}", remote_part, parent), name.to_string())
    } else {
        (remote_part, path_part.to_string())
    }
}
