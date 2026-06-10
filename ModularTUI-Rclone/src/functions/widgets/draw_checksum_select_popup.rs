use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, List, ListItem},
};
use crate::functions::*;

pub fn draw_checksum_select_popup(frame: &mut Frame, selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(40, 35, size);
    frame.render_widget(Clear, area);

    let options = vec![
        "md5".to_string(),
        "sha1".to_string(),
        "sha256".to_string(),
        "crc32".to_string(),
        "xxhash".to_string(),
    ];

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}", opt.to_uppercase())).style(style)
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            translate("exp_hash_title"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}
