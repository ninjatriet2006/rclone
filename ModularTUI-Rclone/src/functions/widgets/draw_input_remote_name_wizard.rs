use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use crate::functions::*;

pub fn draw_input_remote_name_wizard(frame: &mut Frame, provider: &str, input_buffer: &str) {
    let size = frame.size();
    let area = centered_rect(50, 25, size);
    frame.render_widget(Clear, area);

    let text = vec![
        Line::from(vec![
            Span::raw(translate("conn_wizard_provider_configuring")),
            Span::styled(
                provider,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(translate("conn_wizard_name_prompt")),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Green)),
            Span::styled(
                input_buffer,
                Style::default().fg(Color::White).bg(Color::DarkGray),
            ),
        ]),
    ];

    let block = Block::default()
        .title(Span::styled(
            translate("conn_wizard_name_title"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}
