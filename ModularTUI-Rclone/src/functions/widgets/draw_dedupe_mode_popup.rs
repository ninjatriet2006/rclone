use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use crate::functions::*;

pub fn draw_dedupe_mode_popup(frame: &mut Frame, selected_idx: usize, by_hash: bool) {
    let size = frame.size();
    let area = centered_rect(65, 55, size);
    frame.render_widget(Clear, area);

    let hash_status = if by_hash {
        Span::styled("BẬT (Tìm trùng theo nội dung/mã Hash)", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("TẮT (Tìm trùng theo Tên file)", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    };

    let title_text = translate("exp_dedupe_title");
    let prompt_text = translate("exp_dedupe_prompt");
    let by_hash_prompt = translate("exp_dedupe_by_hash_prompt").replace("{}", "");
    let help_text = translate("exp_dedupe_help");

    let modes = vec![
        translate("exp_dedupe_mode_rename"),
        translate("exp_dedupe_mode_newest"),
        translate("exp_dedupe_mode_oldest"),
        translate("exp_dedupe_mode_largest"),
        translate("exp_dedupe_mode_smallest"),
        translate("exp_dedupe_mode_first"),
        translate("exp_dedupe_mode_skip"),
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
            Constraint::Length(3), // By Hash toggle status
            Constraint::Min(5),    // List of modes
            Constraint::Length(2), // Help
        ])
        .split(block.inner(area));

    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    frame.render_widget(Paragraph::new(prompt_text), chunks[0]);

    use ratatui::text::Line;
    let hash_prompt_para = Paragraph::new(vec![
        Line::from(by_hash_prompt),
        Line::from(hash_status),
    ])
    .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(hash_prompt_para, chunks[1]);

    let list = List::new(items);
    frame.render_widget(list, chunks[2]);

    let help_para = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC));
    frame.render_widget(help_para, chunks[3]);
}
