use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

#[derive(Debug, Clone, PartialEq)]
pub enum ImportWizardState {
    None,
    InputProfileName {
        input_buffer: String,
    },
    SelectImportType {
        profile_name: String,
        selected_idx: usize, // 0: Link Direct (URL), 1: Copy & Pull (Local Path)
    },
    InputSource {
        profile_name: String,
        import_type: usize, // 0: URL, 1: Local Path
        input_buffer: String,
    },
    ConfirmImportOverwrite {
        profile_name: String,
        source_path_or_url: String,
        import_type: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportPopupState {
    None,
    ConfirmOverwrite { profile_name: String },
    Success { path: String },
}

pub struct ProfileState {
    pub profiles: Vec<(String, String)>, // (Tên Profile, Đường dẫn tệp)
    pub selected_idx: usize,
    pub wizard: ImportWizardState,
    pub export_popup: ExportPopupState,
    pub error_message: Option<String>,
}

impl ProfileState {
    pub fn new() -> Self {
        ProfileState {
            profiles: Vec::new(),
            selected_idx: 0,
            wizard: ImportWizardState::None,
            export_popup: ExportPopupState::None,
            error_message: None,
        }
    }

    pub fn next(&mut self) {
        if !self.profiles.is_empty() {
            self.selected_idx = (self.selected_idx + 1) % self.profiles.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.profiles.is_empty() {
            if self.selected_idx == 0 {
                self.selected_idx = self.profiles.len() - 1;
            } else {
                self.selected_idx -= 1;
            }
        }
    }
}

pub fn draw(state: &ProfileState, frame: &mut Frame, area: Rect, active_profile: &str) {
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

    let block_title = crate::lang::translate("prof_title");
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
        crate::lang::translate("prof_help")
    } else {
        crate::lang::translate("prof_help_wizard")
    };
    let help_paragraph = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
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
            let msg = crate::lang::translate("prof_overwrite_import_msg").replace("{}", profile_name);
            super::draw_popup(frame, &crate::lang::translate("prof_overwrite_import_title"), &msg, 50, 30);
        }
        ImportWizardState::None => {}
    }

    // Vẽ Export Popups
    match &state.export_popup {
        ExportPopupState::ConfirmOverwrite { profile_name } => {
            let msg = crate::lang::translate("prof_overwrite_export_msg").replace("{}", profile_name);
            super::draw_popup(frame, &crate::lang::translate("prof_overwrite_export_title"), &msg, 55, 30);
        }
        ExportPopupState::Success { path } => {
            let msg = crate::lang::translate("prof_export_success").replace("{}", path);
            super::draw_popup(frame, &crate::lang::translate("prof_success_title"), &msg, 60, 30);
        }
        ExportPopupState::None => {}
    }

    if let Some(ref err) = state.error_message {
        super::draw_popup(frame, &crate::lang::translate("prof_error_title"), err, 60, 30);
    }
}

fn draw_input_profile_name(frame: &mut Frame, input_buffer: &str) {
    let size = frame.size();
    let area = centered_rect(50, 25, size);
    frame.render_widget(Clear, area);

    let text = vec![
        Line::from(crate::lang::translate("prof_new_prompt")),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Blue)),
            Span::styled(
                input_buffer,
                Style::default().fg(Color::White).bg(Color::DarkGray),
            ),
        ]),
    ];

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("prof_new_title"),
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_select_import_type(frame: &mut Frame, profile_name: &str, selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(55, 30, size);
    frame.render_widget(Clear, area);

    let types = vec![
        crate::lang::translate("prof_type_url"),
        crate::lang::translate("prof_type_local"),
    ];

    let items: Vec<ListItem> = types
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}", t)).style(style)
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("prof_type_title").replace("{}", profile_name),
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn draw_input_source(
    frame: &mut Frame,
    profile_name: &str,
    import_type: usize,
    input_buffer: &str,
) {
    let size = frame.size();
    let area = centered_rect(60, 30, size);
    frame.render_widget(Clear, area);

    let prompt = if import_type == 0 {
        crate::lang::translate("prof_url_prompt")
    } else {
        crate::lang::translate("prof_file_prompt")
    };

    let text = vec![
        Line::from(crate::lang::translate("prof_importing").replace("{}", profile_name)),
        Line::from(""),
        Line::from(prompt),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Blue)),
            Span::styled(
                input_buffer,
                Style::default().fg(Color::White).bg(Color::DarkGray),
            ),
        ]),
    ];

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("prof_source_title"),
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

use super::centered_rect;
