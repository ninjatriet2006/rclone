pub fn format_display_name(name: &str) -> String {
    if name.starts_with(' ') || name.ends_with(' ') {
        let trimmed_start = name.trim_start_matches(' ');
        let leading_count = name.len() - trimmed_start.len();
        let trimmed_end = trimmed_start.trim_end_matches(' ');
        let trailing_count = trimmed_start.len() - trimmed_end.len();
        format!(
            "{}{}{}",
            "·".repeat(leading_count),
            trimmed_end,
            "·".repeat(trailing_count)
        )
    } else {
        name.to_string()
    }
}
