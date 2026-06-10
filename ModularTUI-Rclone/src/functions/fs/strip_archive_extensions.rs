pub fn strip_archive_extensions(name: &str) -> String {
    let name_lower = name.to_lowercase();
    if name_lower.ends_with(".tar.gz") {
        name[..name.len() - 7].to_string()
    } else if name_lower.ends_with(".tar.xz") {
        name[..name.len() - 7].to_string()
    } else if name_lower.ends_with(".zip")
        || name_lower.ends_with(".tar")
        || name_lower.ends_with(".rar")
        || name_lower.ends_with(".7z")
    {
        if let Some(pos) = name.rfind('.') {
            name[..pos].to_string()
        } else {
            name.to_string()
        }
    } else {
        name.to_string()
    }
}
