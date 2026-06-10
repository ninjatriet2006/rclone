use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use crate::functions::*;

pub fn draw_input_profile_name(frame: &mut Frame, input_buffer: &str) {
    let size = frame.size();
    let area = centered_rect(50, 25, size);
    frame.render_widget(Clear, area);

    let text = vec![
        Line::from(translate("prof_new_prompt")),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Blue)),
            Span::styled(
                input_buffer,
                Style::default().fg(Color::White).bg(Color::DarkGray),
            ),
        ]),
    ];

    let block = Block::default()
        .title(Span::styled(
            translate("prof_new_title"),
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}
