use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use crate::functions::*;

pub fn draw_simple_oauth_wizard(frame: &mut Frame, provider: &str, remote_name: &str, auth_url: &str) {
    let size = frame.size();
    let area = centered_rect(60, 40, size);
    frame.render_widget(Clear, area);

    let text = vec![
        Line::from(vec![
            Span::raw(translate("conn_wizard_oauth_started")),
            Span::styled(
                remote_name,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ("),
            Span::raw(provider),
            Span::raw(")"),
        ]),
        Line::from(""),
        Line::from(translate("conn_wizard_oauth_open_browser")),
        Line::from(translate("conn_wizard_oauth_copy_url")),
        Line::from(""),
        Line::from(Span::styled(
            auth_url,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::UNDERLINED),
        )),
        Line::from(""),
        Line::from(Span::styled(
            translate("conn_wizard_oauth_waiting"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::SLOW_BLINK),
        )),
    ];

    let block = Block::default()
        .title(Span::styled(
            translate("conn_wizard_oauth_title"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, area);
}
