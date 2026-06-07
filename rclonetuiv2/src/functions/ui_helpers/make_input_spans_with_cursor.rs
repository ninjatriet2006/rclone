use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

pub fn make_input_spans_with_cursor<'a>(
    text: &str,
    cursor_idx: usize,
    fg: Color,
    bg: Color,
) -> Vec<Span<'a>> {
    let chars: Vec<char> = text.chars().collect();
    let mut spans = Vec::new();
    
    if cursor_idx < chars.len() {
        let before: String = chars[0..cursor_idx].iter().collect();
        let cursor_char: String = chars[cursor_idx].to_string();
        let after: String = chars[cursor_idx + 1..].iter().collect();
        
        if !before.is_empty() {
            spans.push(Span::styled(before, Style::default().fg(fg).bg(bg)));
        }
        spans.push(Span::styled(
            cursor_char,
            Style::default().fg(bg).bg(fg).add_modifier(Modifier::BOLD),
        ));
        if !after.is_empty() {
            spans.push(Span::styled(after, Style::default().fg(fg).bg(bg)));
        }
    } else {
        spans.push(Span::styled(text.to_string(), Style::default().fg(fg).bg(bg)));
        spans.push(Span::styled(
            "█",
            Style::default().fg(Color::Red).bg(bg),
        ));
    }
    spans
}
