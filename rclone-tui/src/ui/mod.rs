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
    text::{Line, Span},
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

/// Phân tích cú pháp dòng hướng dẫn (dạng [Key]Action|[Key]Action)
/// thành một đối tượng Line chứa nhiều Span được tô màu sắc tương phản để dễ đọc.
pub fn parse_help_line(help_text: &str) -> Line<'static> {
    let mut spans = Vec::new();
    let parts: Vec<&str> = help_text.split('|').collect();
    for (idx, part) in parts.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));
        }

        if let Some(start_idx) = part.find('[') {
            if let Some(end_idx) = part.find(']') {
                if start_idx < end_idx {
                    // Phần tiền tố trước dấu '[' (nếu có)
                    if start_idx > 0 {
                        spans.push(Span::styled(part[..start_idx].to_string(), Style::default().fg(Color::DarkGray)));
                    }
                    // Phần phím tắt nằm trong ngoặc vuông (ví dụ: [Alt+R])
                    let key_text = &part[start_idx..=end_idx];
                    spans.push(Span::styled(
                        key_text.to_string(),
                        Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                    ));
                    // Phần nhãn mô tả sau dấu ']' (ví dụ: Remote)
                    let desc_text = &part[end_idx + 1..];
                    spans.push(Span::styled(desc_text.to_string(), Style::default()));
                    continue;
                }
            }
        }
        // Trường hợp fallback nếu không đúng cấu trúc
        spans.push(Span::styled(part.to_string(), Style::default()));
    }
    Line::from(spans)
}

/// Ước lượng số dòng cần thiết để hiển thị chuỗi trợ giúp đã được wrap theo chiều rộng có sẵn.
pub fn estimate_wrapped_lines(help_text: &str, width: usize) -> usize {
    if width == 0 {
        return 1;
    }
    // Chuyển đổi chuỗi trợ giúp (dạng ngăn cách bởi |) sang chuỗi hiển thị thực tế
    let parsed_text = help_text.replace('|', " | ");
    let mut lines = 1;
    let mut current_line_len = 0;
    
    for word in parsed_text.split_whitespace() {
        let word_len = word.chars().count();
        if current_line_len == 0 {
            current_line_len = word_len;
        } else if current_line_len + 1 + word_len <= width {
            current_line_len += 1 + word_len;
        } else {
            lines += 1;
            current_line_len = word_len;
        }
    }
    lines
}


/// Tạo danh sách các Span cho chuỗi nhập liệu kèm theo con trỏ hiển thị ở vị trí chỉ định.
pub fn make_input_spans_with_cursor<'a>(
    text: &str,
    cursor_idx: usize,
    fg: Color,
    bg: Color,
) -> Vec<Span<'a>> {
    let chars: Vec<char> = text.chars().collect();
    let mut spans = Vec::new();
    
    if cursor_idx < chars.len() {
        let before: String = chars[0..cursor_idx].iter().collect();
        let cursor_char: String = chars[cursor_idx].to_string();
        let after: String = chars[cursor_idx + 1..].iter().collect();
        
        if !before.is_empty() {
            spans.push(Span::styled(before, Style::default().fg(fg).bg(bg)));
        }
        // Block cursor: đảo ngược màu chữ và nền
        spans.push(Span::styled(
            cursor_char,
            Style::default().fg(bg).bg(fg).add_modifier(Modifier::BOLD),
        ));
        if !after.is_empty() {
            spans.push(Span::styled(after, Style::default().fg(fg).bg(bg)));
        }
    } else {
        // Con trỏ nằm ở cuối chuỗi
        spans.push(Span::styled(text.to_string(), Style::default().fg(fg).bg(bg)));
        spans.push(Span::styled(
            "█",
            Style::default().fg(Color::Red).bg(bg),
        ));
    }
    spans
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn test_estimate_wrapped_lines() {
        let help_text = "[Tab]Chuyển đổi khung|[Enter/BS]Vào/Lùi|[Alt+R]Remote|[Alt+Y]Đổi tên|[Ctrl+C/V]Sao chép/Dán|[Ctrl+X]Di chuyển|[Delete]Xóa|[Alt+N]Thư mục mới|[Alt+T]Đồng bộ|[Alt+V]Chọn đơn|[Shift+V]Chọn vùng|[Alt+O]Chức năng khác|[ESC]Quay lại";
        
        // Rất rộng thì chỉ cần 1 dòng
        let lines_large = estimate_wrapped_lines(help_text, 1000);
        assert_eq!(lines_large, 1);

        // Chiều rộng bình thường thì sẽ wrap thành nhiều dòng
        let lines_normal = estimate_wrapped_lines(help_text, 100);
        assert!(lines_normal > 1);
    }
}
