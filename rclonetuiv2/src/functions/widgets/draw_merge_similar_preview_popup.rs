use std::collections::HashSet;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use crate::functions::*;

pub fn draw_merge_similar_preview_popup(
    frame: &mut Frame,
    summary_report: &[String],
    tree_root: &TreeNode,
    expanded_paths: &HashSet<String>,
    selected_rel_path: &str,
    scroll_offset: usize,
) {
    let size = frame.size();
    let area = centered_rect(75, 75, size);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            " XEM TRƯỚC KẾT QUẢ GỘP (MERGE PREVIEW REPORT) ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let inner_area = block.inner(area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),    // Report content
            Constraint::Length(2), // Help line
        ])
        .split(inner_area);

    frame.render_widget(block, area);

    let mut tree_lines = Vec::new();
    flatten_tree(tree_root, "", true, true, expanded_paths, selected_rel_path, &mut tree_lines);

    let mut items = Vec::new();
    for line in summary_report {
        let style = Style::default().fg(Color::White);
        items.push(ListItem::new(Line::from(Span::styled(line.clone(), style))));
    }

    items.push(ListItem::new(Line::from("")));

    for (formatted, _rel_path, is_selected) in tree_lines {
        let mut style = if formatted.contains("TRÙNG LẶP") || formatted.contains("XÓA") || formatted.contains("DELETE") {
            Style::default().fg(Color::Yellow)
        } else if formatted.contains("DI CHUYỂN") || formatted.contains("MOVE") {
            Style::default().fg(Color::Green)
        } else if formatted.contains("GHI ĐÈ") || formatted.contains("OVERWRITE") {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else if formatted.contains("=====") || formatted.contains("-----") {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };

        if is_selected {
            style = style.bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD);
        }

        items.push(ListItem::new(Line::from(Span::styled(formatted, style))));
    }

    let height = chunks[0].height as usize;
    let visible_items: Vec<ListItem> = items
        .into_iter()
        .skip(scroll_offset)
        .take(height)
        .collect();

    let list = List::new(visible_items).block(Block::default().borders(Borders::NONE));
    frame.render_widget(list, chunks[0]);

    let help_line = Paragraph::new(" [Up/Down] Di chuyển | [Right/Left/Space] Đóng/Mở thư mục | [Enter] Bắt đầu gộp | [Esc] Hủy bỏ ")
        .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC));
    frame.render_widget(help_line, chunks[1]);
}

pub fn flatten_tree(
    node: &TreeNode,
    prefix: &str,
    is_last: bool,
    is_root: bool,
    expanded_paths: &HashSet<String>,
    selected_rel_path: &str,
    lines: &mut Vec<(String, String, bool)>,
) {
    let mut node_str = if node.is_dir {
        format!("{}/", node.name)
    } else {
        node.name.clone()
    };

    if node.is_dir && !is_root {
        let indicator = if expanded_paths.contains(&node.rel_path) {
            "▼ "
        } else {
            "▶ "
        };
        node_str = format!("{}{}", indicator, node_str);
    }

    let action_str = match &node.action {
        Some(act) => format!("  {}", act),
        None => "".to_string(),
    };

    let formatted = if is_root {
        format!("{}{}", node_str, action_str)
    } else {
        format!("{}{}{}{}", prefix, if is_last { "└── " } else { "├── " }, node_str, action_str)
    };

    let is_selected = node.rel_path == selected_rel_path;
    lines.push((formatted, node.rel_path.clone(), is_selected));

    if is_root || expanded_paths.contains(&node.rel_path) {
        let new_prefix = if is_root {
            "".to_string()
        } else {
            format!("{}{}", prefix, if is_last { "    " } else { "│   " })
        };
        let child_count = node.children.len();
        for (idx, (_, child)) in node.children.iter().enumerate() {
            flatten_tree(
                child,
                &new_prefix,
                idx == child_count - 1,
                false,
                expanded_paths,
                selected_rel_path,
                lines,
            );
        }
    }
}
