pub fn estimate_wrapped_lines(help_text: &str, width: usize) -> usize {
    if width == 0 {
        return 1;
    }
    let parsed_text = help_text.replace('|', " | ");
    let mut lines = 1;
    let mut current_line_len = 0;
    
    for word in parsed_text.split_whitespace() {
        let word_len = word.chars().count();
        if current_line_len == 0 {
            current_line_len = word_len;
        } else if current_line_len + 1 + word_len <= width {
            current_line_len += 1 + word_len;
        } else {
            lines += 1;
            current_line_len = word_len;
        }
    }
    lines
}
