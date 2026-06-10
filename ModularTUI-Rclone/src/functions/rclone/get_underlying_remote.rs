pub fn get_underlying_remote(config_path: &str, remote: &str) -> Option<String> {
    let target_section = match remote.find(':') {
        Some(pos) => &remote[..pos],
        None => remote,
    }.trim();
    
    if let Ok(content) = std::fs::read_to_string(config_path) {
        let mut in_section = false;
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                let section_name = &line[1..line.len()-1];
                in_section = section_name == target_section;
            } else if in_section {
                if line.starts_with("remote") {
                    let parts: Vec<&str> = line.split('=').collect();
                    if parts.len() >= 2 {
                        return Some(parts[1].trim().to_string());
                    }
                }
            }
        }
    }
    None
}
