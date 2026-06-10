use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    text::{Line, Span},
};
use crate::functions::*;

pub fn draw_file_explorer(state: &mut ExplorerState, frame: &mut Frame, area: Rect) {
    let help_text = translate("exp_help");
    let available_width = area.width.saturating_sub(2) as usize;
    let needed_lines = estimate_wrapped_lines(&help_text, available_width);

    let mut help_height = needed_lines.min(3);
    if help_height > 1 {
        let temp_help_bar_height = help_height + 2;
        let list_height = area.height.saturating_sub(3 + temp_help_bar_height as u16);
        let visible_files_height = list_height.saturating_sub(2);
        if visible_files_height <= 5 {
            help_height = 1;
        }
    }
    if help_height == 0 {
        help_height = 1;
    }
    let help_bar_height = help_height + 2;

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(3), // Console panel
            Constraint::Length(help_bar_height as u16), // Help bar
        ])
        .split(area);

    let panes_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_chunks[0]);

    // Vẽ Pane Trái
    draw_pane(
        frame,
        &mut state.left_pane,
        panes_chunks[0],
        state.active_pane == ActivePane::Left,
    );

    // Vẽ Pane Phải
    draw_pane(
        frame,
        &mut state.right_pane,
        panes_chunks[1],
        state.active_pane == ActivePane::Right,
    );

    // Console / Status Panel
    let console_text = if let Some(ref items) = state.clipboard_items {
        let count = items.len();
        let first_remote = items.first().map(|i| i.remote.clone()).unwrap_or_default();
        let label = if first_remote.is_empty() {
            translate("srv_local_system")
        } else {
            first_remote.trim_end_matches(':').to_string()
        };
        let msg = format!("📋 {} mục đã sao chép từ {}", count, label);
        Line::from(vec![
            Span::styled("⚡ ", Style::default().fg(Color::Yellow)),
            Span::styled(msg, Style::default().fg(Color::Black)),
        ])
    } else if let Some(ref item) = state.clipboard {
        let src_display = if item.remote.is_empty() {
            let local_prefix = translate("srv_local_system");
            if item.path == "/" || item.path.is_empty() {
                format!("{}:/{}", local_prefix, item.name)
            } else {
                format!("{}:{}/{}", local_prefix, item.path.trim_end_matches('/'), item.name)
            }
        } else {
            let clean_remote = item.remote.trim_end_matches(':');
            let clean_path = item.path.trim_start_matches('/');
            if clean_path.is_empty() {
                format!("{}:/{}", clean_remote, item.name)
            } else {
                format!("{}:{}/{}", clean_remote, clean_path.trim_end_matches('/'), item.name)
            }
        };

        let template = translate("exp_console_pending");
        let msg = template.replace("{}", &src_display);

        Line::from(vec![
            Span::styled("⚡ ", Style::default().fg(Color::Yellow)),
            Span::styled(msg, Style::default().fg(Color::Black)),
        ])
    } else {
        let empty_msg = translate("exp_console_empty");
        Line::from(vec![
            Span::styled("⚙️ ", Style::default().fg(Color::DarkGray)),
            Span::styled(empty_msg, Style::default().fg(Color::DarkGray)),
        ])
    };

    let console_paragraph = Paragraph::new(console_text)
        .block(
            Block::default()
                .title(Span::styled(
                    translate("exp_console_title"),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(console_paragraph, main_chunks[1]);

    // Help Bar
    let help_paragraph = Paragraph::new(parse_help_line(&help_text))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(help_paragraph, main_chunks[2]);

    // Vẽ Popups từ widgets
    match &state.popup {
        ExplorerPopup::InputNewFolder { input_buffer } => {
            draw_input_new_folder_popup(frame, input_buffer, state.edit_cursor_idx);
        }
        ExplorerPopup::CopyProgress { src, dest, pct, .. } => {
            let msg = translate("exp_copy_msg")
                .replacen("{}", src, 1)
                .replacen("{}", dest, 1)
                .replace("{:.1}", &format!("{:.1}", pct));
            draw_popup(frame, &translate("exp_copy_title"), &msg, 60, 35);
        }
        ExplorerPopup::MoveProgress { src, dest, pct, .. } => {
            let msg = translate("exp_move_msg")
                .replacen("{}", src, 1)
                .replacen("{}", dest, 1)
                .replace("{:.1}", &format!("{:.1}", pct));
            draw_popup(frame, &translate("exp_move_title"), &msg, 60, 35);
        }
        ExplorerPopup::SelectRemote {
            remotes,
            selected_idx,
        } => {
            draw_select_remote_popup(frame, remotes, *selected_idx);
        }
        ExplorerPopup::ConfirmFallback {
            title,
            options,
            selected_idx,
            restricted_files,
            restricted_scroll,
            focus_files,
            ..
        } => {
            draw_confirm_fallback_popup(
                frame,
                title,
                options,
                *selected_idx,
                restricted_files,
                *restricted_scroll,
                *focus_files,
            );
        }
        ExplorerPopup::InputRename { old_name, input_buffer, .. } => {
            draw_input_rename_popup(frame, old_name, input_buffer, state.edit_cursor_idx);
        }
        ExplorerPopup::SpecialActionsMenu { selected_idx } => {
            draw_special_actions_popup(frame, *selected_idx);
        }
        ExplorerPopup::ViewFile { file_name, content, scroll_offset } => {
            draw_file_view_popup(frame, file_name, content, *scroll_offset);
        }
        ExplorerPopup::ChecksumTypeSelect { selected_idx } => {
            draw_checksum_select_popup(frame, *selected_idx);
        }
        ExplorerPopup::CryptdecodeForm { remote_input, encrypted_input, is_remote_focused, output_result } => {
            draw_cryptdecode_form_popup(frame, remote_input, encrypted_input, *is_remote_focused, output_result.as_deref(), state.edit_cursor_idx);
        }
        ExplorerPopup::DecompressModeSelect { archive_path, selected_idx } => {
            draw_decompress_mode_popup(frame, archive_path, *selected_idx);
        }
        ExplorerPopup::DecompressPathInput { archive_path, selected_idx } => {
            draw_decompress_path_popup(frame, archive_path, *selected_idx);
        }
        ExplorerPopup::DecompressPathManualInput { archive_path, input_buffer } => {
            draw_decompress_path_manual_input(frame, archive_path, input_buffer, state.edit_cursor_idx);
        }
        ExplorerPopup::TuiExplorerSelector { archive_path, remote, path, items, selected_idx, scroll_offset, loading } => {
            draw_tui_selector_popup(frame, archive_path, remote, path, items, *selected_idx, *scroll_offset, *loading);
        }
        ExplorerPopup::SpecialActionMessage { title, message } => {
            draw_popup(frame, title, message, 65, 40);
        }
        ExplorerPopup::InputPasteRename { input_buffer } => {
            draw_input_paste_rename_popup(frame, input_buffer, state.edit_cursor_idx);
        }
        ExplorerPopup::InputSharedLink { input_buffer } => {
            draw_input_shared_link_popup(frame, input_buffer, state.edit_cursor_idx);
        }
        ExplorerPopup::SelectBaseRemote { remotes, selected_idx, .. } => {
            draw_select_base_remote_popup(frame, remotes, *selected_idx);
        }
        ExplorerPopup::PermissionScanning { src, dest, is_dir: _, scanned_count, total_files, restricted_count } => {
            draw_permission_scanning_popup(frame, src, dest, *scanned_count, *total_files, *restricted_count);
        }
        ExplorerPopup::DedupeModeSelect { by_hash, selected_idx } => {
            draw_dedupe_mode_popup(frame, *selected_idx, *by_hash);
        }
        ExplorerPopup::CopyModeSelect { selected_idx, .. } => {
            draw_copy_mode_popup(frame, *selected_idx);
        }
        ExplorerPopup::MergeSimilarDestinationSelect { folders, selected_idx } => {
            draw_merge_similar_destination_select_popup(frame, folders, *selected_idx);
        }
        ExplorerPopup::MergeSimilarScanning { folders_count, scanned_count } => {
            draw_merge_similar_scanning_popup(frame, *folders_count, *scanned_count);
        }
        ExplorerPopup::MergeSimilarPreview {
            summary_report,
            tree_root,
            expanded_paths,
            selected_rel_path,
            scroll_offset,
            ..
        } => {
            draw_merge_similar_preview_popup(
                frame,
                summary_report,
                tree_root,
                expanded_paths,
                selected_rel_path,
                *scroll_offset,
            );
        }
        ExplorerPopup::None => {}
    }

    if let Some((ref title, ref msg)) = state.notification {
        frame.render_widget(Clear, area);
        draw_popup(frame, title, msg, 60, 30);
    }
}

fn draw_pane(frame: &mut Frame, pane: &mut ExplorerPane, area: Rect, is_active: bool) {
    let title_color = if is_active { Color::Cyan } else { Color::DarkGray };
    let border_color = if is_active { Color::Cyan } else { Color::DarkGray };

    let fs_label = if pane.remote.is_empty() {
        translate("srv_local_system")
    } else {
        pane.remote.clone()
    };

    let title = if pane.selected_names.is_empty() {
        format!(" {} : {} ", fs_label, pane.path)
    } else {
        format!(" {} : {} (Đã chọn: {}) ", fs_label, pane.path, pane.selected_names.len())
    };

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default().fg(title_color).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let height = area.height.saturating_sub(2) as usize;
    pane.adjust_scroll(height);

    let pane_width = area.width as usize;

    let (show_size, size_col_width) = if pane_width >= 40 {
        (true, 10)
    } else {
        (false, 0)
    };

    let total_cols_width = if show_size { size_col_width + 2 } else { 0 };
    let name_width = pane_width.saturating_sub(2 + total_cols_width);

    let items: Vec<ListItem> = if pane.items.is_empty() {
        if pane.loading {
            vec![ListItem::new(Line::from(vec![Span::styled(
                translate("exp_loading"),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
            )]))]
        } else {
            vec![ListItem::new(Line::from(vec![Span::styled(
                translate("exp_empty"),
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            )]))]
        }
    } else {
        pane.items
            .iter()
            .enumerate()
            .skip(pane.scroll_offset)
            .take(height)
            .map(|(i, item)| {
                let is_selected_item = pane.selected_names.contains(&item.name);
                let is_anchor = pane.shift_anchor == Some(i) || pane.alt_anchor == Some(i);

                let select_prefix = if item.name == ".." {
                    "  "
                } else if is_selected_item {
                    "✔ "
                } else {
                    "  "
                };

                let prefix = if item.name == ".." {
                    "📁 "
                } else if item.is_dir {
                    "📁 "
                } else {
                    "📄 "
                };

                let display_name = format_display_name(&item.name);
                let raw_name = format!("{}{}{}", select_prefix, prefix, display_name);
                let display_name = if raw_name.chars().count() > name_width {
                    if name_width > 3 {
                        raw_name.chars().take(name_width - 3).collect::<String>() + "..."
                    } else {
                        raw_name.chars().take(name_width).collect::<String>()
                    }
                } else {
                    let pad = name_width.saturating_sub(raw_name.chars().count());
                    raw_name + &" ".repeat(pad)
                };

                let mut style = if i == pane.selected_idx && is_active {
                    if is_selected_item {
                        Style::default().fg(Color::Black).bg(Color::LightGreen).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
                    }
                } else if is_selected_item {
                    Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)
                } else if item.is_dir {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                if is_anchor {
                    style = style.add_modifier(Modifier::UNDERLINED);
                    if i != pane.selected_idx || !is_active {
                        style = style.fg(Color::LightMagenta);
                    }
                }

                let name_span = Span::styled(display_name, style);
                let mut spans = vec![name_span];

                if show_size {
                    let size_str = if item.is_dir {
                        "---".to_string()
                    } else if item.name == ".." {
                        "---".to_string()
                    } else {
                        format_size(item.size)
                    };
                    let size_formatted = if size_str.len() < size_col_width {
                        let pad = size_col_width - size_str.len();
                        " ".repeat(pad) + &size_str
                    } else {
                        size_str.chars().take(size_col_width).collect::<String>()
                    };
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled(
                        size_formatted,
                        if i == pane.selected_idx && is_active {
                            style
                        } else {
                            Style::default().fg(Color::Magenta)
                        },
                    ));
                }

                let item_style = if i == pane.selected_idx && is_active {
                    if is_selected_item {
                        Style::default().bg(Color::LightGreen)
                    } else {
                        Style::default().bg(Color::Cyan)
                    }
                } else {
                    Style::default()
                };

                let line = Line::from(spans);
                ListItem::new(line).style(item_style)
            })
            .collect()
    };

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}
