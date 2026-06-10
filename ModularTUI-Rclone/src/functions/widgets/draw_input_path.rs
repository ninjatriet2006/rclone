use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use crate::functions::*;

pub fn draw_input_path(
    frame: &mut Frame,
    service_type: &ServiceType,
    remote: &str,
    input_buffer: &str,
    cursor_idx: usize,
) {
    let size = frame.size();
    let area = centered_rect(60, 30, size);
    frame.render_widget(Clear, area);

    let prompt = match service_type {
        ServiceType::Mount | ServiceType::NfsMount => {
            translate("srv_mount_point_prompt")
        }
        ServiceType::Serve => translate("srv_share_path_prompt"),
        _ => "".to_string(),
    };

    let local_system_label = translate("srv_local_system");
    let mut spans = vec![
        Span::styled("> ", Style::default().fg(Color::Magenta)),
    ];
    spans.extend(make_input_spans_with_cursor(input_buffer, cursor_idx, Color::White, Color::DarkGray));

    let text = vec![
        Line::from(vec![
            Span::raw(translate("srv_selected_source")),
            Span::styled(
                if remote.is_empty() {
                    &local_system_label
                } else {
                    remote
                },
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(prompt),
        Line::from(spans),
    ];

    let block = Block::default()
        .title(Span::styled(
            translate("srv_mount_point_title"),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, area);
}
