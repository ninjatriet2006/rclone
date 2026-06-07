use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn parse_help_line(help_text: &str) -> Line<'static> {
    let mut spans = Vec::new();
    let parts: Vec<&str> = help_text.split('|').collect();
    for (idx, part) in parts.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));
        }

        if let Some(start_idx) = part.find('[') {
            if let Some(end_idx) = part.find(']') {
                if start_idx < end_idx {
                    if start_idx > 0 {
                        spans.push(Span::styled(part[..start_idx].to_string(), Style::default().fg(Color::DarkGray)));
                    }
                    let key_text = &part[start_idx..=end_idx];
                    spans.push(Span::styled(
                        key_text.to_string(),
                        Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                    ));
                    let desc_text = &part[end_idx + 1..];
                    spans.push(Span::styled(desc_text.to_string(), Style::default()));
                    continue;
                }
            }
        }
        spans.push(Span::styled(part.to_string(), Style::default()));
    }
    Line::from(spans)
}
