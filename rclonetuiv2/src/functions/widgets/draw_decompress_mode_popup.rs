use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, List, ListItem},
};
use crate::functions::*;

pub fn draw_decompress_mode_popup(frame: &mut Frame, archive_path: &str, selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(55, 30, size);
    frame.render_widget(Clear, area);

    let options = vec![
        translate("exp_archive_here"),
        translate("exp_archive_folder"),
        translate("exp_archive_path"),
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
            ListItem::new(format!("  {}", opt)).style(style)
        })
        .collect();

    let filename = std::path::Path::new(archive_path).file_name().and_then(|f| f.to_str()).unwrap_or(archive_path);
    let block = Block::default()
        .title(Span::styled(
            format!(" {} ({}) ", translate("exp_archive_title"), filename),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}
