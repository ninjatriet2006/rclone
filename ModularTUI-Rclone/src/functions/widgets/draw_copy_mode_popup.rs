use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use crate::functions::*;

pub fn draw_copy_mode_popup(frame: &mut Frame, selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(65, 35, size);
    frame.render_widget(Clear, area);

    let title_text = translate("exp_copy_mode_title");
    let prompt_text = translate("exp_copy_mode_prompt");
    let help_text = translate("exp_copy_mode_help");

    let modes = vec![
        translate("exp_copy_mode_normal"),
        translate("exp_copy_mode_checksum"),
    ];

    let items: Vec<ListItem> = modes
        .iter()
        .enumerate()
        .map(|(i, mode)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}", mode)).style(style)
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            title_text,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Prompt
            Constraint::Min(2),    // List of options
            Constraint::Length(2), // Help
        ])
        .split(block.inner(area));

    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    frame.render_widget(Paragraph::new(prompt_text), chunks[0]);

    let list = List::new(items);
    frame.render_widget(list, chunks[1]);

    let help_para = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC));
    frame.render_widget(help_para, chunks[2]);
}
