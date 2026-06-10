use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use crate::functions::*;

pub fn draw_tui_selector_popup(
    frame: &mut Frame,
    _archive_path: &str,
    remote: &str,
    path: &str,
    items: &[FileItem],
    selected_idx: usize,
    scroll_offset: usize,
    loading: bool,
) {
    let size = frame.size();
    let area = centered_rect(70, 70, size);
    frame.render_widget(Clear, area);

    let fs_label = if remote.is_empty() {
        translate("srv_local_system")
    } else {
        remote.to_string()
    };
    let title = format!(" [Duyệt Đích] {} : {} ", fs_label, path);

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    frame.render_widget(&block, area);
    let inner_area = block.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(inner_area);

    let height = chunks[0].height as usize;
    let list_items: Vec<ListItem> = if items.is_empty() {
        if loading {
            vec![ListItem::new(translate("exp_loading"))]
        } else {
            vec![ListItem::new(translate("exp_empty"))]
        }
    } else {
        items
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(height)
            .map(|(i, item)| {
                let prefix = if item.is_dir { "📁 " } else { "📄 " };
                let line = Line::from(vec![
                    Span::styled(
                        format!("{}{}", prefix, format_display_name(&item.name)),
                        if item.is_dir {
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        },
                    ),
                ]);
                let style = if i == selected_idx {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default()
                };
                ListItem::new(line).style(style)
            })
            .collect()
    };

    let list = List::new(list_items);
    frame.render_widget(list, chunks[0]);

    let footer_text = translate("exp_archive_tui_prompt");
    frame.render_widget(Paragraph::new(footer_text).style(Style::default().fg(Color::Yellow)), chunks[1]);
}
