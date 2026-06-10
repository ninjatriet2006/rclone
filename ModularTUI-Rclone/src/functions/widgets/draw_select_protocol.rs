use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, List, ListItem},
};
use crate::functions::*;

pub fn draw_select_protocol(frame: &mut Frame, remote: &str, path: &str, selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(55, 35, size);
    frame.render_widget(Clear, area);

    let protocols = vec![
        translate("srv_proto_http"),
        translate("srv_proto_ftp"),
        translate("srv_proto_webdav"),
        translate("srv_proto_sftp"),
    ];

    let items: Vec<ListItem> = protocols
        .iter()
        .enumerate()
        .map(|(i, proto)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Magenta)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(proto.clone()).style(style)
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            translate("srv_select_proto_title").replace(
                "{}{}",
                &format!(
                    "{}{}",
                    if remote.is_empty() { "Local:" } else { remote },
                    path
                ),
            ),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}
