use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use crate::functions::*;

pub fn draw_confirm_fallback_popup(
    frame: &mut Frame,
    title: &str,
    options: &[String],
    selected_idx: usize,
    restricted_files: &Option<Vec<String>>,
    restricted_scroll: usize,
    focus_files: bool,
) {
    let size = frame.size();
    let area = if restricted_files.is_some() {
        centered_rect(75, 70, size)
    } else {
        centered_rect(65, 50, size)
    };
    frame.render_widget(Clear, area);

    let chunks = if restricted_files.is_some() {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Warning header
                Constraint::Min(6),    // Scrollable restricted files list
                Constraint::Length(6), // Options list
                Constraint::Length(2), // Help / Instructions footer
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Warning header
                Constraint::Min(4),    // Options list
            ])
            .split(area)
    };

    // Render warning title / header
    let header_text = if restricted_files.is_some() {
        Line::from(vec![
            Span::styled("⚠️ PHÁT HIỆN QUYỀN TRUY CẬP BỊ HẠN CHẾ (ACCESS RESTRICTED): ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw("Một số tệp tin không có quyền tải xuống!"),
        ])
    } else {
        Line::from(vec![
            Span::styled("⚠️ CẢNH BÁO: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("Tính năng này không hỗ trợ trực tiếp bởi Remote!"),
        ])
    };
    
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::BOTTOM))
        .style(Style::default().fg(Color::White));
    frame.render_widget(header, chunks[0]);

    if let Some(files) = restricted_files {
        let files_border_color = if focus_files { Color::Cyan } else { Color::DarkGray };
        let files_title = format!(" DANH SÁCH FILE BỊ KHÓA / CHẶN TẢI ({}) ", files.len());
        let files_block = Block::default()
            .title(Span::styled(files_title, Style::default().fg(files_border_color).add_modifier(Modifier::BOLD)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(files_border_color));
        
        let files_height = chunks[1].height.saturating_sub(2) as usize;
        let range = calculate_scroll_range(restricted_scroll, files.len(), files_height);
        
        let file_items: Vec<ListItem> = files[range.clone()]
            .iter()
            .enumerate()
            .map(|(idx, f)| {
                let actual_idx = range.start + idx;
                let is_selected_file = actual_idx == restricted_scroll;
                
                let prefix = if is_selected_file && focus_files {
                    "👉 "
                } else {
                    "   "
                };
                
                let style = if is_selected_file && focus_files {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else if is_selected_file {
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                
                ListItem::new(Line::from(vec![
                    Span::styled(format!("  {:02}. ", actual_idx + 1), Style::default().fg(Color::DarkGray)),
                    Span::styled("🔒 ", Style::default().fg(Color::Red)),
                    Span::styled(prefix, Style::default().fg(Color::Yellow)),
                    Span::styled(f.clone(), style),
                ]))
            })
            .collect();
            
        let list = List::new(file_items).block(files_block);
        frame.render_widget(list, chunks[1]);

        let options_border_color = if !focus_files { Color::Cyan } else { Color::DarkGray };
        let options_title = " LỰA CHỌN PHƯƠNG ÁN XỬ LÝ ";
        let options_block = Block::default()
            .title(Span::styled(options_title, Style::default().fg(options_border_color).add_modifier(Modifier::BOLD)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(options_border_color));

        let items: Vec<ListItem> = options
            .iter()
            .enumerate()
            .map(|(i, opt)| {
                let style = if !focus_files && i == selected_idx {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else if i == selected_idx {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::UNDERLINED)
                } else {
                    Style::default()
                };
                ListItem::new(format!("  • {}", opt)).style(style)
            })
            .collect();
        
        let list = List::new(items).block(options_block);
        frame.render_widget(list, chunks[2]);

        let help_text = if focus_files {
            " [Tab] Chuyển Focus | [Up/Down] Cuộn danh sách file | [Esc] Hủy "
        } else {
            " [Tab] Chuyển Focus | [Up/Down] Chọn phương án | [Enter] Thực thi | [Esc] Hủy "
        };
        let help_paragraph = Paragraph::new(parse_help_line(help_text));
        frame.render_widget(help_paragraph, chunks[3]);
    } else {
        // Render options list
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
                ListItem::new(format!("  • {}", opt)).style(style)
            })
            .collect();

        let block = Block::default()
            .title(Span::styled(
                title,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let height = chunks[1].height.saturating_sub(2) as usize;
        let range = calculate_scroll_range(selected_idx, items.len(), height);
        let visible_items: Vec<ListItem> = items.into_iter().skip(range.start).take(range.end - range.start).collect();

        let list = List::new(visible_items).block(block);
        frame.render_widget(list, chunks[1]);
    }
}
