use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, List, ListItem},
};
use crate::functions::*;

pub fn draw_select_auth_mode_wizard(
    frame: &mut Frame,
    _provider: &str,
    remote_name: &str,
    selected_idx: usize,
) {
    let size = frame.size();
    let area = centered_rect(55, 35, size);
    frame.render_widget(Clear, area);

    let modes = vec![
        translate("conn_wizard_auth_simple"),
        translate("conn_wizard_auth_headless"),
        translate("conn_wizard_auth_advanced"),
    ];

    let items: Vec<ListItem> = modes
        .iter()
        .enumerate()
        .map(|(i, mode)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}", mode)).style(style)
        })
        .collect();

    let title_raw = translate("conn_wizard_auth_mode_title");
    let title_fmt = format!(" {} ", title_raw.replace("{}", remote_name));
    let block = Block::default()
        .title(Span::styled(
            title_fmt,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}
