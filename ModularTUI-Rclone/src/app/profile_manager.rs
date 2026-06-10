use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use crate::functions::*;

pub fn draw_profile_manager(state: &ProfileState, frame: &mut Frame, area: Rect, active_profile: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(3), // Help bar
        ])
        .split(area);

    // Vẽ danh sách profile cấu hình
    let items: Vec<ListItem> = state
        .profiles
        .iter()
        .enumerate()
        .map(|(i, (name, path))| {
            let active_indicator = if name == active_profile {
                " -> [ACTIVE]"
            } else {
                ""
            };
            let style = if i == state.selected_idx
                && state.wizard == ImportWizardState::None
                && state.export_popup == ExportPopupState::None
            {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {} : {}{}", name, path, active_indicator)).style(style)
        })
        .collect();

    let block_title = translate("prof_title");
    let block = Block::default()
        .title(Span::styled(
            &block_title,
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let list = List::new(items).block(block);
    frame.render_widget(list, chunks[0]);

    // Help Bar
    let help_text = if state.wizard == ImportWizardState::None
        && state.export_popup == ExportPopupState::None
    {
        translate("prof_help")
    } else {
        translate("prof_help_wizard")
    };
    let help_paragraph = Paragraph::new(parse_help_line(&help_text))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(help_paragraph, chunks[1]);

    // Vẽ Import Wizard Popup
    match &state.wizard {
        ImportWizardState::InputProfileName { input_buffer } => {
            draw_input_profile_name(frame, input_buffer);
        }
        ImportWizardState::SelectImportType {
            profile_name,
            selected_idx,
        } => {
            draw_select_import_type(frame, profile_name, *selected_idx);
        }
        ImportWizardState::InputSource {
            profile_name,
            import_type,
            input_buffer,
        } => {
            draw_input_source(frame, profile_name, *import_type, input_buffer);
        }
        ImportWizardState::ConfirmImportOverwrite { profile_name, .. } => {
            let msg = translate("prof_overwrite_import_msg").replace("{}", profile_name);
            frame.render_widget(Clear, area);
            draw_popup(frame, &translate("prof_overwrite_import_title"), &msg, 50, 30);
        }
        ImportWizardState::None => {}
    }

    // Vẽ Export Popups
    match &state.export_popup {
        ExportPopupState::ConfirmOverwrite { profile_name } => {
            let msg = translate("prof_overwrite_export_msg").replace("{}", profile_name);
            frame.render_widget(Clear, area);
            draw_popup(frame, &translate("prof_overwrite_export_title"), &msg, 55, 30);
        }
        ExportPopupState::Success { path } => {
            let msg = translate("prof_export_success").replace("{}", path);
            frame.render_widget(Clear, area);
            draw_popup(frame, &translate("prof_success_title"), &msg, 60, 30);
        }
        ExportPopupState::None => {}
    }

    if let Some(ref err) = state.error_message {
        frame.render_widget(Clear, area);
        draw_popup(frame, &translate("prof_error_title"), err, 60, 30);
    }
}
