use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use crate::functions::*;

pub fn draw_input_source(
    frame: &mut Frame,
    profile_name: &str,
    import_type: usize,
    input_buffer: &str,
) {
    let size = frame.size();
    let area = centered_rect(60, 30, size);
    frame.render_widget(Clear, area);

    let prompt = if import_type == 0 {
        translate("prof_url_prompt")
    } else {
        translate("prof_file_prompt")
    };

    let text = vec![
        Line::from(translate("prof_importing").replace("{}", profile_name)),
        Line::from(""),
        Line::from(prompt),
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
            translate("prof_source_title"),
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, area);
}
