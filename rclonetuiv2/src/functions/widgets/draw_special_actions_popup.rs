use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem},
};
use crate::functions::*;

pub fn draw_special_actions_popup(frame: &mut Frame, selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(50, 50, size);
    frame.render_widget(Clear, area);

    let options = vec![
        translate("exp_special_link"),
        translate("exp_special_hash"),
        translate("exp_special_cleanup"),
        translate("exp_special_rmdir"),
        translate("exp_special_rmdirs"),
        translate("exp_special_cryptdecode"),
        translate("exp_special_archive"),
        translate("exp_special_dedupe"),
        translate("exp_special_merge_similar"),
        translate("exp_special_close"),
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

    let block = Block::default()
        .title(Span::styled(
            translate("exp_special_title"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}
