use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use crate::functions::*;

pub fn draw_ask_flags(
    frame: &mut Frame,
    _service_type: &ServiceType,
    remote: &str,
    path: &str,
    protocol: &Option<String>,
    flags: &[(String, String, String, String)],
    current_flag_idx: usize,
    input_buffer: &str,
    is_editing: bool,
    cursor_idx: usize,
) {
    let size = frame.size();
    let area = centered_rect(65, 45, size);
    frame.render_widget(Clear, area);

    let (flag_name, question, default_val, _) = &flags[current_flag_idx];

    let local_system_label = translate("srv_local_system");
    let mut info_spans = vec![
        Span::raw(translate("srv_config_for")),
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
    ];
    if !path.is_empty() {
        info_spans.push(Span::raw(format!(" (Path: {})", path)));
    }
    if let Some(proto) = protocol {
        info_spans.push(Span::raw(format!(" | Protocol: {}", proto)));
    }

    let progress_text = translate("srv_wizard_progress")
        .replacen("{}", &format!("{}", current_flag_idx + 1), 1)
        .replacen("{}", &format!("{}", flags.len()), 1);

    let mut input_spans = vec![
        Span::raw(translate("srv_wizard_input_prompt")),
        Span::styled(
            default_val,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("]): "),
    ];
    if is_editing {
        input_spans.extend(make_input_spans_with_cursor(input_buffer, cursor_idx, Color::White, Color::DarkGray));
    } else {
        input_spans.push(Span::styled(
            input_buffer,
            Style::default().fg(Color::White).bg(Color::DarkGray),
        ));
    }

    let text = vec![
        Line::from(info_spans),
        Line::from(progress_text),
        Line::from("------------------------------------------------------------------"),
        Line::from(""),
        Line::from(Span::styled(
            translate("srv_wizard_flag").replace("{}", flag_name),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(question.as_str()),
        Line::from(""),
        Line::from(input_spans),
    ];

    let block = Block::default()
        .title(Span::styled(
            translate("srv_wizard_title"),
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
