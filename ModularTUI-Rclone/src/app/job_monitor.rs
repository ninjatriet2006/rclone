use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use crate::functions::*;

fn make_colored_progress_bar(
    percentage: u16,
    width: usize,
    is_active: bool,
    is_error: bool,
    cursor_color: Color,
) -> Vec<Span<'static>> {
    let percentage = percentage.min(100) as usize;
    let mut spans = vec![Span::styled("[", Style::default().fg(Color::Gray))];

    let filled_green = (percentage * width) / 100;

    if is_error {
        let filled_red = width - filled_green;
        if filled_green > 0 {
            spans.push(Span::styled("█".repeat(filled_green), Style::default().fg(Color::Green)));
        }
        if filled_red > 0 {
            spans.push(Span::styled("█".repeat(filled_red), Style::default().fg(Color::Red)));
        }
    } else if is_active {
        let filled_yellow = if filled_green < width { 1 } else { 0 };
        let filled_white = width - filled_green - filled_yellow;

        if filled_green > 0 {
            spans.push(Span::styled("█".repeat(filled_green), Style::default().fg(Color::Green)));
        }
        if filled_yellow > 0 {
            spans.push(Span::styled("█", Style::default().fg(cursor_color)));
        }
        if filled_white > 0 {
            spans.push(Span::styled("░".repeat(filled_white), Style::default().fg(Color::White)));
        }
    } else {
        let filled_white = width - filled_green;
        if filled_green > 0 {
            spans.push(Span::styled("█".repeat(filled_green), Style::default().fg(Color::Green)));
        }
        if filled_white > 0 {
            spans.push(Span::styled("░".repeat(filled_white), Style::default().fg(Color::White)));
        }
    }

    spans.push(Span::styled("]", Style::default().fg(Color::Gray)));
    spans
}

pub fn draw_job_monitor(state: &mut MonitorState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),      // Tổng quan tiến trình (Global Stats)
            Constraint::Percentage(55), // Khung ở giữa (Active, Pending & Failed)
            Constraint::Min(5),         // Chi tiết tác vụ đang chọn
            Constraint::Length(3),      // Help bar
        ])
        .split(area);

    // Split the middle section horizontally: Left column (Active + Pending Jobs), Right column (Failed Files)
    let mid_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60), // Active & Pending Jobs
            Constraint::Percentage(40), // Failed & Restricted Files
        ])
        .split(chunks[1]);

    // Split Left column vertically: Active Jobs (60%), Pending Jobs (40%)
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60), // Active Jobs
            Constraint::Percentage(40), // Pending Jobs
        ])
        .split(mid_chunks[0]);

    // 1. Vẽ Tổng quan tiến trình
    let speed_str = format!("{}/s", format_size(state.speed as u64));
    let upload_str = format!("{}/s", format_size(state.upload_speed as u64));
    let download_str = format!("{}/s", format_size(state.download_speed as u64));
    let max_bw_str = format!("{}/s", format_size(state.max_bandwidth));
    let progress_pct = if state.active_jobs.is_empty() {
        100.0
    } else if state.total_bytes > 0 {
        (state.bytes_transferred as f64 / state.total_bytes as f64) * 100.0
    } else {
        0.0
    };

    let first_line_spans = vec![
        Span::raw(translate("mon_speed_label")),
        Span::styled(
            speed_str,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(translate("mon_upload_speed_label")),
        Span::styled(
            upload_str,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(translate("mon_download_speed_label")),
        Span::styled(
            download_str,
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(translate("mon_max_bandwidth_label")),
        Span::styled(
            max_bw_str,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    let mut pct_line_spans = vec![
        Span::raw(translate("mon_total_pct_label")),
    ];
    pct_line_spans.extend(make_colored_progress_bar(
        progress_pct as u16,
        25,
        !state.active_jobs.is_empty(),
        !state.failed_files.is_empty(),
        Color::Yellow,
    ));
    pct_line_spans.push(Span::styled(
        format!(" {:.1}% ", progress_pct),
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    ));
    pct_line_spans.push(Span::raw(translate("mon_transferred_label")));
    pct_line_spans.push(Span::styled(
        format_size(state.bytes_transferred),
        Style::default().fg(Color::Green),
    ));
    pct_line_spans.push(Span::raw(" / "));
    pct_line_spans.push(Span::styled(
        format_size(state.total_bytes),
        Style::default().fg(Color::Cyan),
    ));

    let third_line_spans = vec![
        Span::raw(translate("mon_active_transfers_label")),
        Span::styled(
            state.active_transfers.to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(translate("mon_active_checkers_label")),
        Span::styled(
            state.active_checks.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | PID Engine: "),
        Span::styled(
            std::process::id().to_string(),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    let fourth_line_spans = vec![
        Span::raw(" Phân tích nghẽn (Bottleneck): "),
        Span::styled(
            state.bottleneck_reason.clone(),
            Style::default()
                .fg(if state.bottleneck_reason.contains("Bình thường") || state.bottleneck_reason.contains("Optimal") {
                    Color::Green
                } else if state.bottleneck_reason.contains("băng thông") || state.bottleneck_reason.contains("Limit") {
                    Color::Yellow
                } else {
                    Color::Red
                })
                .add_modifier(Modifier::BOLD),
        ),
    ];

    let stats_text = vec![
        Line::from(first_line_spans),
        Line::from(pct_line_spans),
        Line::from(third_line_spans),
        Line::from(fourth_line_spans),
    ];

    let stats_block = Block::default()
        .title(Span::styled(
            translate("mon_stats_title"),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let stats_paragraph = Paragraph::new(stats_text).block(stats_block);
    frame.render_widget(stats_paragraph, chunks[0]);

    // 2. Vẽ các Job đang chạy (Active Jobs)
    let is_active_focused = state.active_pane == MonitorPane::ActiveJobs;
    let active_border_color = if is_active_focused { Color::Yellow } else { Color::DarkGray };

    let active_height = left_chunks[0].height.saturating_sub(2) as usize;
    if state.visible_nodes.is_empty() {
        state.active_scroll_offset = 0;
    } else {
        if state.selected_node_idx >= state.visible_nodes.len() {
            state.selected_node_idx = 0;
        }
        if state.selected_node_idx < state.active_scroll_offset {
            state.active_scroll_offset = state.selected_node_idx;
        } else if state.selected_node_idx >= state.active_scroll_offset + active_height {
            state.active_scroll_offset = state.selected_node_idx - active_height + 1;
        }
    }

    let active_items: Vec<ListItem> = state
        .visible_nodes
        .iter()
        .enumerate()
        .skip(state.active_scroll_offset)
        .take(active_height)
        .map(|(i, node)| {
            let style = if is_active_focused && i == state.selected_node_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let indent = "  ".repeat(node.depth);

            let line = if node.is_job {
                let expand_marker = if node.expanded { "▼ " } else { "▶ " };
                let bar_spans = make_colored_progress_bar(node.percentage, 12, true, false, Color::Yellow);
                let eta_str = if node.eta >= 0 {
                    format!("ETA: {}s", node.eta)
                } else {
                    "ETA: --".to_string()
                };

                let mut spans = vec![
                    Span::raw(indent),
                    Span::styled(expand_marker, Style::default().fg(Color::Yellow)),
                    Span::styled("⚡ ", Style::default().fg(Color::Yellow)),
                ];
                spans.extend(bar_spans);
                spans.push(Span::styled(
                    format!(" {}% ", node.percentage),
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    format!("{} ", node.name),
                    Style::default().add_modifier(Modifier::BOLD),
                ));

                if node.size > 0 {
                    spans.push(Span::styled(
                        format!(
                            "({} / {}) ",
                            format_size(node.bytes),
                            format_size(node.size)
                        ),
                        Style::default().fg(Color::DarkGray),
                    ));
                }

                spans.push(Span::styled(
                    format!("{} - ", eta_str),
                    Style::default().fg(Color::Magenta),
                ));
                spans.push(Span::styled(
                    format!("{}/s", format_size(node.speed)),
                    Style::default().fg(Color::Yellow),
                ));

                Line::from(spans)
            } else if node.is_dir {
                let expand_marker = if node.expanded { "▼ " } else { "▶ " };
                let line = if node.id == "group/background_tasks" {
                    let mut spans = vec![
                        Span::raw(indent),
                        Span::styled(expand_marker, Style::default().fg(Color::Yellow)),
                        Span::styled("📁 ", Style::default().fg(Color::Cyan)),
                        Span::styled(
                            format!("{} ", node.name),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                    ];
                    
                    if node.direct_total_children > 0 {
                        let bar_spans = make_colored_progress_bar(node.percentage, 12, true, false, Color::Yellow);
                        spans.push(Span::raw(" "));
                        spans.extend(bar_spans);
                        spans.push(Span::styled(
                            format!(" {}% ", node.percentage),
                            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                        ));
                        spans.push(Span::styled(
                            format!(
                                "({} / {} mục) ",
                                node.direct_completed_children,
                                node.direct_total_children
                            ),
                            Style::default().fg(Color::DarkGray),
                        ));
                        spans.push(Span::styled(
                            format!(
                                "({} / {}) ",
                                format_size(node.recursive_completed_bytes),
                                format_size(node.recursive_total_size)
                            ),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                    
                    if node.speed > 0 {
                        spans.push(Span::styled(
                            format!("{}/s", format_size(node.speed)),
                            Style::default().fg(Color::Yellow),
                        ));
                    }
                    
                    Line::from(spans)
                } else if node.id.starts_with("op/") {
                    let is_op_checking = node.status == "checking";
                    let mut spans = vec![
                        Span::raw(indent),
                        Span::styled(expand_marker, Style::default().fg(Color::Yellow)),
                        Span::styled("📁 ", Style::default().fg(Color::Cyan)),
                        Span::styled(
                            format!("{} ", node.name),
                            if is_op_checking {
                                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().add_modifier(Modifier::BOLD)
                            },
                        ),
                    ];
                    
                    if node.direct_total_children > 0 {
                        let bar_color = if is_op_checking { Color::Cyan } else { Color::Yellow };
                        let bar_spans = make_colored_progress_bar(node.percentage, 12, true, false, bar_color);
                        spans.push(Span::raw(" "));
                        spans.extend(bar_spans);
                        spans.push(Span::styled(
                            format!(" {}% ", node.percentage),
                            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                        ));
                        spans.push(Span::styled(
                            format!(
                                "({} / {} mục) ",
                                node.direct_completed_children,
                                node.direct_total_children
                            ),
                            Style::default().fg(Color::DarkGray),
                        ));
                        spans.push(Span::styled(
                            format!(
                                "({} / {}) ",
                                format_size(node.recursive_completed_bytes),
                                format_size(node.recursive_total_size)
                            ),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                    Line::from(spans)
                } else {
                    let mut spans = vec![
                        Span::raw(indent),
                        Span::styled(expand_marker, Style::default().fg(Color::Yellow)),
                        Span::styled("📁 ", Style::default().fg(Color::Cyan)),
                        Span::styled(
                            format!("{} ", node.name),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                    ];
                    
                    if node.direct_total_children > 0 {
                        let bar_spans = make_colored_progress_bar(node.percentage, 12, true, false, Color::Yellow);
                        spans.push(Span::raw(" "));
                        spans.extend(bar_spans);
                        spans.push(Span::styled(
                            format!(" {}% ", node.percentage),
                            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                        ));
                        spans.push(Span::styled(
                            format!(
                                "({} / {} mục) ",
                                node.direct_completed_children,
                                node.direct_total_children
                            ),
                            Style::default().fg(Color::DarkGray),
                        ));
                        spans.push(Span::styled(
                            format!(
                                "({} / {}) ",
                                format_size(node.recursive_completed_bytes),
                                format_size(node.recursive_total_size)
                            ),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                    Line::from(spans)
                };
                line
            } else {
                let expand_marker = "  ";
                let icon = match node.status.as_str() {
                    "completed" => ("🟢 ", Color::Green),
                    "running" => ("⏳ ", Color::Yellow),
                    "checking" => ("🔍 ", Color::Cyan),
                    "failed" => ("🔴 ", Color::Red),
                    "queued" => ("🕒 ", Color::DarkGray),
                    _ => ("📄 ", Color::Gray),
                };

                let mut spans = vec![
                    Span::raw(indent),
                    Span::raw(expand_marker),
                    Span::styled(icon.0, Style::default().fg(icon.1)),
                    Span::styled(
                        node.name.clone(),
                        if node.status == "checking" {
                            Style::default().fg(Color::Cyan)
                        } else {
                            Style::default()
                        }
                    ),
                ];

                if node.status == "failed" {
                    spans.push(Span::styled(
                        format!(" (Lỗi: {})", node.error),
                        Style::default().fg(Color::Red),
                    ));
                } else if node.status == "running" {
                    let bar_spans = make_colored_progress_bar(node.percentage, 10, true, false, Color::Yellow);
                    spans.push(Span::raw(" "));
                    spans.extend(bar_spans);
                    spans.push(Span::styled(
                        format!(" {}% ", node.percentage),
                        Style::default().fg(Color::Green),
                    ));
                    if node.size > 0 {
                        spans.push(Span::styled(
                            format!(
                                "({} / {}) ",
                                format_size(node.bytes),
                                format_size(node.size)
                            ),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                    if node.speed > 0 {
                        spans.push(Span::styled(
                            format!("{}/s ", format_size(node.speed)),
                            Style::default().fg(Color::Yellow),
                        ));
                    }
                    if node.eta >= 0 {
                        spans.push(Span::styled(
                            format!("ETA: {}s", node.eta),
                            Style::default().fg(Color::Magenta),
                        ));
                    }
                } else if node.status == "completed" {
                    if node.size > 0 {
                        spans.push(Span::styled(
                            format!(" ({})", format_size(node.size)),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                }

                Line::from(spans)
            };

            ListItem::new(line).style(style)
        })
        .collect();

    let active_block = Block::default()
        .title(Span::styled(
            format!(" {} ", translate("mon_active_title")),
            Style::default()
                .fg(active_border_color)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(active_border_color));
    let active_list = List::new(active_items).block(active_block);
    frame.render_widget(active_list, left_chunks[0]);

    // 3. Vẽ các tác vụ đang chờ xác nhận (Pending Jobs)
    let is_pending_focused = state.active_pane == MonitorPane::PendingJobs;
    let pending_border_color = if is_pending_focused { Color::Yellow } else { Color::DarkGray };

    let pending_height = left_chunks[1].height.saturating_sub(2) as usize;
    if state.pending_jobs.is_empty() {
        state.pending_scroll_offset = 0;
    } else {
        if state.selected_pending_idx >= state.pending_jobs.len() {
            state.selected_pending_idx = 0;
        }
        if state.selected_pending_idx < state.pending_scroll_offset {
            state.pending_scroll_offset = state.selected_pending_idx;
        } else if state.selected_pending_idx >= state.pending_scroll_offset + pending_height {
            state.pending_scroll_offset = state.selected_pending_idx - pending_height + 1;
        }
    }

    let pending_items: Vec<ListItem> = state
        .pending_jobs
        .iter()
        .enumerate()
        .skip(state.pending_scroll_offset)
        .take(pending_height)
        .map(|(i, job)| {
            let style = if is_pending_focused && i == state.selected_pending_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let status_tag = match job.status.as_str() {
                "Bypassed" => "[ĐÃ BỎ QUA - CẦN QUYẾT ĐỊNH]",
                "Scanned (Has Restrictions)" => "[CÓ FILE BỊ CHẶN TẢI]",
                "Scanned (No Restrictions)" => "[AN TOÀN - KHÔNG BỊ CHẶN]",
                _ => "[CHỜ]",
            };

            let status_color = match job.status.as_str() {
                "Scanned (Has Restrictions)" => Color::Red,
                "Scanned (No Restrictions)" => Color::Green,
                _ => Color::Yellow,
            };

            let has_errors = job.status == "Scanned (Has Restrictions)";
            let bar_spans = make_colored_progress_bar(0, 12, false, has_errors, Color::Yellow);

            let mut spans = vec![
                Span::styled("⚠️ ", Style::default().fg(Color::Yellow)),
            ];
            spans.extend(bar_spans);
            spans.push(Span::styled(format!(" {} ", status_tag), Style::default().fg(status_color).add_modifier(Modifier::BOLD)));
            spans.push(Span::styled(format!("{} -> {} ", job.src, job.dest), Style::default().add_modifier(Modifier::BOLD)));
            spans.push(Span::styled(
                format!("(Bị chặn {}/{} file)", job.restricted_files.len(), job.total_files),
                Style::default().fg(Color::LightRed)
            ));

            let line = Line::from(spans);

            ListItem::new(line).style(style)
        })
        .collect();

    let pending_block = Block::default()
        .title(Span::styled(
            " TÁC VỤ SAO CHÉP CHỜ XÁC NHẬN (PENDING) ",
            Style::default()
                .fg(pending_border_color)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(pending_border_color));
    let pending_list = List::new(pending_items).block(pending_block);
    frame.render_widget(pending_list, left_chunks[1]);

    // 4. Vẽ danh sách các file lỗi hoặc file bị hạn chế (Failed / Restricted Files)
    let is_failed_focused = state.active_pane == MonitorPane::FailedFiles;
    let failed_border_color = if is_failed_focused { Color::Yellow } else { Color::Red };

    let failed_height = mid_chunks[1].height.saturating_sub(2) as usize;

    let (right_panel_title, right_panel_items) = if state.active_pane == MonitorPane::PendingJobs && !state.pending_jobs.is_empty() {
        let job = &state.pending_jobs[state.selected_pending_idx];
        let title = format!(" FILE BỊ KHÓA CỦA JOB ĐANG CHỌN ({}) ", job.restricted_files.len());
        
        let list_len = job.restricted_files.len();
        if state.failed_scroll_offset >= list_len {
            state.failed_scroll_offset = 0;
        }
        
        let items: Vec<ListItem> = job.restricted_files.iter()
            .skip(state.failed_scroll_offset)
            .take(failed_height)
            .map(|f| {
                ListItem::new(Line::from(vec![
                    Span::styled("🔒 ", Style::default().fg(Color::Red)),
                    Span::styled(f.clone(), Style::default().fg(Color::White)),
                ]))
            }).collect();
        (title, items)
    } else {
        let title = " CÁC FILE BỊ LỖI QUYỀN / KHÔNG SAO CHÉP ĐƯỢC ".to_string();
        if state.failed_files.is_empty() {
            state.failed_scroll_offset = 0;
        } else {
            if state.selected_failed_idx >= state.failed_files.len() {
                state.selected_failed_idx = 0;
            }
            if state.selected_failed_idx < state.failed_scroll_offset {
                state.failed_scroll_offset = state.selected_failed_idx;
            } else if state.selected_failed_idx >= state.failed_scroll_offset + failed_height {
                state.failed_scroll_offset = state.selected_failed_idx - failed_height + 1;
            }
        }
        let items: Vec<ListItem> = state
            .failed_files
            .iter()
            .enumerate()
            .skip(state.failed_scroll_offset)
            .take(failed_height)
            .map(|(i, item)| {
                let is_selected = is_failed_focused && i == state.selected_failed_idx;
                let text_style = if is_selected {
                    Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let err_style = if is_selected {
                    Style::default().bg(Color::Yellow).fg(Color::Red).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Red)
                };
                let time_style = if is_selected {
                    Style::default().bg(Color::Yellow).fg(Color::DarkGray).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                let line = Line::from(vec![
                    Span::styled("❌ ", Style::default().fg(Color::Red)),
                    Span::styled(format!("[{}] ", item.time), time_style),
                    Span::styled(format!("{} ", item.src), text_style),
                    Span::styled(format!("(Lỗi: {})", item.error), err_style),
                ]);
                ListItem::new(line)
            })
            .collect();
        (title, items)
    };

    let right_block = Block::default()
        .title(Span::styled(
            right_panel_title,
            Style::default()
                .fg(failed_border_color)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(failed_border_color));
    let right_list = List::new(right_panel_items).block(right_block);
    frame.render_widget(right_list, mid_chunks[1]);

    // 5. Vẽ nhật ký debug/chi tiết tác vụ đang chọn
    let details_block = Block::default()
        .title(Span::styled(
            " NHẬT KÝ DEBUG & CHI TIẾT TÁC VỤ ĐANG CHỌN ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let details_text = if state.active_jobs.is_empty() {
        vec![Line::from(vec![
            Span::styled("[DEBUG] Không có tác vụ rclone nào đang chạy.", Style::default().fg(Color::Black)),
        ])]
    } else {
        let selected_job = if state.selected_node_idx < state.visible_nodes.len() {
            let node = &state.visible_nodes[state.selected_node_idx];
            state.active_jobs.iter().find(|j| j.job_id == node.job_id)
        } else {
            None
        }.or_else(|| state.active_jobs.first());

        if let Some(job) = selected_job {
            let mut lines = Vec::new();

            lines.push(Line::from(vec![
                Span::styled(
                    format!(
                        "[DEBUG] Tác vụ: {} | Job ID: {}",
                        job.name,
                        job.job_id
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| "Không có".to_string())
                    ),
                    Style::default().fg(Color::Black),
                ),
            ]));

            if job.job_id.is_some() {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!(
                            "[DEBUG] Nhóm thống kê: {} | Bắt đầu: {} | Đã chạy: {:.1}s",
                            job.group, job.start_time, job.duration
                        ),
                        Style::default().fg(Color::Black),
                    ),
                ]));
            }

            lines.push(Line::from(vec![
                Span::styled(
                    format!(
                        "[DEBUG] Tiến độ: {}% | Tốc độ: {}/s | Truyền tải: {} / {}",
                        job.percentage,
                        format_size(job.speed),
                        format_size(job.bytes),
                        format_size(job.size)
                    ),
                    Style::default().fg(Color::Black),
                ),
            ]));

            if !job.description.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("[DEBUG] Lệnh/Mô tả đầy đủ: {}", job.description),
                        Style::default().fg(Color::Black),
                    ),
                ]));
            }

            lines
        } else {
            vec![Line::from(vec![
                Span::styled("[DEBUG] Vui lòng chọn một tác vụ phía trên.", Style::default().fg(Color::Black)),
            ])]
        }
    };

    let details_paragraph = Paragraph::new(details_text).block(details_block);
    frame.render_widget(details_paragraph, chunks[2]);

    // Help Bar
    let help_text = if state.active_pane == MonitorPane::PendingJobs {
        " [Tab] Chuyển Khung | [Up/Down] Chọn | [Enter/C] Giải quyết | [Delete/D] Xóa Job | [Esc] Quay lại "
    } else if state.active_pane == MonitorPane::FailedFiles {
        " [Tab] Chuyển Khung | [Up/Down] Chọn | [R] Thử lại tệp lỗi | [Esc] Quay lại "
    } else {
        " [Left/Right/Space] Thu nhỏ/Mở rộng | [Up/Down] Chọn | [Delete/D] Dừng Job | [_] Xóa xong (Clear) | [Tab] Chuyển Khung | [Esc] Quay lại "
    };
    let help_paragraph = Paragraph::new(
        parse_help_line(help_text),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(help_paragraph, chunks[3]);

    // Vẽ popup xác nhận dừng tác vụ
    if let Some(job) = &state.confirm_stop_job {
        let overlay_area = centered_rect(60, 25, area);
        frame.render_widget(Clear, overlay_area);

        let popup_block = Block::default()
            .title(Span::styled(
                " XÁC NHẬN HỦY BỎ TÁC VỤ ",
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red));

        let text = vec![
            Line::from(vec![
                Span::raw("Bạn có chắc chắn muốn dừng và hủy bỏ tác vụ sau:"),
            ]),
            Line::from(vec![
                Span::styled(
                    format!(" {}", job.name),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" [Enter] Đồng ý ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" | "),
                Span::styled(" [Esc] Hủy bỏ ", Style::default().fg(Color::Gray)),
            ]),
        ];

        let paragraph = Paragraph::new(text)
            .block(popup_block)
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(paragraph, overlay_area);
    }
}
