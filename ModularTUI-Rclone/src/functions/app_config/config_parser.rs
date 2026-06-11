use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub struct ConfigSection {
    pub name: Option<String>,
    pub lines: Vec<String>,
}

pub fn parse_config(content: &str) -> Vec<ConfigSection> {
    let mut sections = Vec::new();
    let mut current_section = ConfigSection {
        name: None,
        lines: Vec::new(),
    };

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            sections.push(current_section);
            let name = trimmed[1..trimmed.len() - 1].to_string();
            current_section = ConfigSection {
                name: Some(name),
                lines: vec![line.to_string()],
            };
        } else {
            current_section.lines.push(line.to_string());
        }
    }
    sections.push(current_section);
    sections
}

pub fn write_config(sections: &[ConfigSection]) -> String {
    let mut output = String::new();
    for section in sections {
        for line in &section.lines {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

pub fn natural_cmp_nocase(a: &str, b: &str) -> Ordering {
    let mut a_chars = a.chars().peekable();
    let mut b_chars = b.chars().peekable();
    loop {
        match (a_chars.peek(), b_chars.peek()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(&a_char), Some(&b_char)) => {
                if a_char.is_ascii_digit() && b_char.is_ascii_digit() {
                    let mut a_num = String::new();
                    while let Some(&c) = a_chars.peek() {
                        if c.is_ascii_digit() {
                            a_num.push(a_chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    let mut b_num = String::new();
                    while let Some(&c) = b_chars.peek() {
                        if c.is_ascii_digit() {
                            b_num.push(b_chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    let a_val = a_num.parse::<u64>().unwrap_or(u64::MAX);
                    let b_val = b_num.parse::<u64>().unwrap_or(u64::MAX);
                    match a_val.cmp(&b_val) {
                        Ordering::Equal => {
                            if a_num.len() != b_num.len() {
                                return a_num.len().cmp(&b_num.len());
                            }
                        }
                        ord => return ord,
                    }
                } else {
                    let a_c = a_chars.next().unwrap();
                    let b_c = b_chars.next().unwrap();
                    let a_lower = a_c.to_lowercase().to_string();
                    let b_lower = b_c.to_lowercase().to_string();
                    match a_lower.cmp(&b_lower) {
                        Ordering::Equal => {}
                        ord => return ord,
                    }
                }
            }
        }
    }
}

pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let ord = natural_cmp_nocase(a, b);
    if ord == Ordering::Equal {
        a.cmp(b)
    } else {
        ord
    }
}

pub fn reorder_ini_sections(file_path: &str, remote1: &str, remote2: &str) -> std::io::Result<()> {
    let content = std::fs::read_to_string(file_path)?;
    let mut sections = parse_config(&content);
    
    let idx1 = sections.iter().position(|s| s.name.as_deref() == Some(remote1));
    let idx2 = sections.iter().position(|s| s.name.as_deref() == Some(remote2));
    
    if let (Some(i1), Some(i2)) = (idx1, idx2) {
        sections.swap(i1, i2);
        let new_content = write_config(&sections);
        std::fs::write(file_path, new_content)?;
    }
    Ok(())
}

pub fn save_sorted_remotes_to_ini(file_path: &str, remotes: &[String]) -> std::io::Result<()> {
    let content = std::fs::read_to_string(file_path)?;
    let mut sections = parse_config(&content);
    
    let mut ordered = Vec::new();
    
    if let Some(pos) = sections.iter().position(|s| s.name.is_none()) {
        ordered.push(sections.remove(pos));
    }
    
    for remote in remotes {
        if let Some(pos) = sections.iter().position(|s| s.name.as_deref() == Some(remote)) {
            ordered.push(sections.remove(pos));
        }
    }
    
    ordered.extend(sections);
    
    let new_content = write_config(&ordered);
    std::fs::write(file_path, new_content)?;
    Ok(())
}
