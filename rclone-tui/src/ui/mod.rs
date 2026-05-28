pub mod connection;
pub mod explorer;
pub mod menu;
pub mod monitor;
pub mod profile;
pub mod services;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

/// Hàm vẽ một hình chữ nhật căn giữa màn hình cho popup
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Helper cập nhật scroll offset dựa trên chỉ số đang chọn, chiều cao hiển thị và tổng số phần tử.
pub fn update_scroll_offset(selected_idx: usize, mut scroll_offset: usize, list_h: usize, total_items: usize) -> usize {
    if total_items == 0 || list_h == 0 {
        return 0;
    }
    if selected_idx < scroll_offset {
        scroll_offset = selected_idx;
    } else if selected_idx >= scroll_offset + list_h {
        scroll_offset = selected_idx.saturating_sub(list_h).saturating_add(1);
    }
    let max_offset = total_items.saturating_sub(list_h);
    if scroll_offset > max_offset {
        scroll_offset = max_offset;
    }
    scroll_offset
}

/// Hàm vẽ pop-up căn giữa màn hình
pub fn draw_popup(frame: &mut Frame, title: &str, message: &str, width_pct: u16, height_pct: u16) {
    let size = frame.size();
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_pct) / 2),
            Constraint::Percentage(height_pct),
            Constraint::Percentage((100 - height_pct) / 2),
        ])
        .split(size);

    let area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_pct) / 2),
            Constraint::Percentage(width_pct),
            Constraint::Percentage((100 - width_pct) / 2),
        ])
        .split(popup_layout[1])[1];

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(message)
        .block(block)
        .wrap(Wrap { trim: true })
        .style(Style::default());

    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
}

/// Helper định dạng dung lượng byte sang chuỗi thân thiện (B, KB, MB, GB, TB) (Bug 72, 89)
pub fn format_size(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < units.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} B", bytes)
    } else {
        format!("{:.2} {}", size, units[unit_idx])
    }
}

/// Tính toán vùng hiển thị (slice range) của danh sách dựa trên chỉ số được chọn và chiều cao hiển thị.
/// Giúp danh sách tự động cuộn (scrolling) mượt mà mà không cần lưu trạng thái cuộn.
pub fn calculate_scroll_range(selected_idx: usize, total_items: usize, height: usize) -> std::ops::Range<usize> {
    if total_items == 0 || height == 0 {
        return 0..0;
    }
    let scroll_offset = if selected_idx < height / 2 {
        0
    } else if selected_idx + height / 2 >= total_items {
        total_items.saturating_sub(height)
    } else {
        selected_idx - height / 2
    };
    let end = std::cmp::min(total_items, scroll_offset + height);
    scroll_offset..end
}
