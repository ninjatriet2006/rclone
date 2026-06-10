use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use crate::functions::*;

pub fn draw_input_new_folder_popup(frame: &mut Frame, input_buffer: &str, cursor_idx: usize) {
    let size = frame.size();
    let area = centered_rect(50, 25, size);
    frame.render_widget(Clear, area);

    let mut spans = vec![Span::styled("> ", Style::default().fg(Color::Cyan))];
    spans.extend(make_input_spans_with_cursor(input_buffer, cursor_idx, Color::White, Color::DarkGray));

    let text = vec![
        Line::from(translate("exp_new_folder_prompt")),
        Line::from(""),
        Line::from(spans),
    ];

    let block = Block::default()
        .title(Span::styled(
            translate("exp_new_folder_title"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}
