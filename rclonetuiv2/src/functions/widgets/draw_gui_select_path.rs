use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use crate::functions::*;

pub fn draw_gui_select_path(
    frame: &mut Frame,
    service_type: &ServiceType,
    remote: &str,
    current_path: &str,
    items: &[FileItem],
    selected_idx: usize,
    loading: bool,
    error_msg: Option<&str>,
    creating_folder: Option<&str>,
) {
    let size = frame.size();
    let area = centered_rect(75, 75, size);
    frame.render_widget(Clear, area);

    let help_text = translate("srv_browser_help");
    let available_width = area.width.saturating_sub(2) as usize;
    let needed_lines = estimate_wrapped_lines(&help_text, available_width);

    let mut help_height = needed_lines.min(3);
    if help_height > 1 {
        let temp_help_bar_height = help_height + 2;
        let list_height = area.height.saturating_sub(3 + temp_help_bar_height as u16);
        let visible_files_height = list_height.saturating_sub(2);
        if visible_files_height <= 8 {
            help_height = 1;
        }
    }
    if help_height == 0 {
        help_height = 1;
    }
    let help_bar_height = help_height + 2;

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),                      // Info bar
            Constraint::Min(5),                         // List of directories
            Constraint::Length(help_bar_height as u16), // Help bar
        ])
        .split(area);

    let title_prefix = match service_type {
        ServiceType::Mount => "MOUNT",
        ServiceType::NfsMount => "NFS MOUNT",
        ServiceType::Serve => "SHARE",
        ServiceType::WebGui => "WEB GUI",
    };

    // 1. Info block
    let info_text = translate("srv_browser_info")
        .replacen("{}", remote, 1)
        .replacen("{}", current_path, 1);
    let info_block = Block::default()
        .title(Span::styled(
            translate("srv_browser_title").replace("{}", title_prefix),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    let info_p = Paragraph::new(info_text)
        .block(info_block)
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(info_p, layout[0]);

    // 2. Directory List or Loading/Error
    let list_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    if loading {
        let loading_p = Paragraph::new(translate("srv_browser_loading"))
            .block(list_block)
            .style(Style::default().fg(Color::Yellow))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(loading_p, layout[1]);
    } else if let Some(err) = error_msg {
        let err_p = Paragraph::new(format!("LỖI: {}", err))
            .block(list_block)
            .style(Style::default().fg(Color::Red))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(err_p, layout[1]);
    } else if items.is_empty() {
        let empty_p = Paragraph::new(translate("srv_browser_empty"))
            .block(list_block)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(empty_p, layout[1]);
    } else {
        let height = layout[1].height.saturating_sub(2) as usize;
        let range = calculate_scroll_range(selected_idx, items.len(), height);
        let visible_items = &items[range.clone()];

        let list_items: Vec<ListItem> = visible_items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let global_idx = range.start + i;
                let style = if global_idx == selected_idx {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Magenta)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(format!(" 📁 {}", item.name)).style(style)
            })
            .collect();
        let list = List::new(list_items).block(list_block);
        frame.render_widget(list, layout[1]);
    }

    // 3. Help block
    let help_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let help_p = Paragraph::new(parse_help_line(&help_text))
        .block(help_block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(help_p, layout[2]);

    // 4. Create Folder Overlay
    if let Some(buf) = creating_folder {
        let overlay_area = centered_rect(50, 25, size);
        frame.render_widget(Clear, overlay_area);
        let overlay_block = Block::default()
            .title(Span::styled(
                translate("srv_browser_new_title"),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        let overlay_text = vec![
            Line::from(translate("srv_browser_new_prompt")),
            Line::from(Span::styled(
                format!(" > {}", buf),
                Style::default().fg(Color::White).bg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(translate("srv_browser_new_help")),
        ];
        let overlay_p = Paragraph::new(overlay_text).block(overlay_block);
        frame.render_widget(overlay_p, overlay_area);
    }
}
