use crate::app::{App, AppEvent, Screen};
use crate::functions::*;
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
use serde_json::json;

pub async fn handle_profile_keys(
    app: &mut App,
    key: KeyEvent,
    _tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
) {
    if app.profile_state.export_popup != ExportPopupState::None {
        if key.code == KeyCode::Enter || key.code == KeyCode::Esc {
            if let ExportPopupState::ConfirmOverwrite { profile_name } = &app.profile_state.export_popup {
                if key.code == KeyCode::Enter {
                    let res = app.config.export_profile(profile_name, true);
                    if let ExportResult::Success(path) = res {
                        app.profile_state.export_popup = ExportPopupState::Success {
                            path: path.to_string_lossy().to_string(),
                        };
                    }
                } else {
                    app.profile_state.export_popup = ExportPopupState::None;
                }
            } else {
                app.profile_state.export_popup = ExportPopupState::None;
            }
        }
        return;
    }

    let wizard = app.profile_state.wizard.clone();
    match wizard {
        ImportWizardState::None => {
            match key.code {
                KeyCode::Esc => {
                    app.screen = Screen::MainMenu;
                }
                KeyCode::Up => app.profile_state.prev(),
                KeyCode::Down => app.profile_state.next(),
                KeyCode::Enter => {
                    if !app.profile_state.profiles.is_empty() {
                        let name = app.profile_state.profiles[app.profile_state.selected_idx].0.clone();
                        app.config.active_profile = name;
                        let _ = app.config.save();

                        let path = app.config.get_active_profile_path();
                        unsafe {
                            std::env::set_var("RCLONE_CONFIG", &path);
                        }
                        let _ = rpc("config/setpath", &json!({"path": path}).to_string());
                    }
                }
                KeyCode::Char('x') | KeyCode::Char('X') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if !app.profile_state.profiles.is_empty() {
                        let name = app.profile_state.profiles[app.profile_state.selected_idx].0.clone();
                        let res = app.config.export_profile(&name, false);
                        match res {
                            ExportResult::Success(path) => {
                                app.profile_state.export_popup = ExportPopupState::Success {
                                    path: path.to_string_lossy().to_string(),
                                };
                            }
                            ExportResult::AlreadyExists(_) => {
                                app.profile_state.export_popup = ExportPopupState::ConfirmOverwrite {
                                    profile_name: name,
                                };
                            }
                            ExportResult::SourceNotFound => {
                                app.profile_state.error_message = Some("Không tìm thấy tệp cấu hình nguồn.".to_string());
                            }
                            ExportResult::Error(e) => {
                                app.profile_state.error_message = Some(e);
                            }
                        }
                    }
                }
                KeyCode::Insert => {
                    app.profile_state.wizard = ImportWizardState::InputProfileName {
                        input_buffer: String::new(),
                    };
                }
                _ => {}
            }
        }
        ImportWizardState::InputProfileName { mut input_buffer } => {
            match key.code {
                KeyCode::Esc => {
                    app.profile_state.wizard = ImportWizardState::None;
                }
                KeyCode::Char(c) => {
                    input_buffer.push(c);
                    app.profile_state.wizard = ImportWizardState::InputProfileName { input_buffer };
                }
                KeyCode::Backspace => {
                    input_buffer.pop();
                    app.profile_state.wizard = ImportWizardState::InputProfileName { input_buffer };
                }
                KeyCode::Enter => {
                    let name = input_buffer.trim().to_string();
                    if !name.is_empty() {
                        app.profile_state.wizard = ImportWizardState::SelectImportType {
                            profile_name: name,
                            selected_idx: 0,
                        };
                    }
                }
                _ => {}
            }
        }
        ImportWizardState::SelectImportType { profile_name, mut selected_idx } => match key.code {
            KeyCode::Esc => {
                app.profile_state.wizard = ImportWizardState::None;
            }
            KeyCode::Up | KeyCode::Down | KeyCode::Tab => {
                selected_idx = if selected_idx == 0 { 1 } else { 0 };
                app.profile_state.wizard = ImportWizardState::SelectImportType {
                    profile_name,
                    selected_idx,
                };
            }
            KeyCode::Enter => {
                app.profile_state.wizard = ImportWizardState::InputSource {
                    profile_name: profile_name.clone(),
                    import_type: selected_idx,
                    input_buffer: String::new(),
                };
            }
            _ => {}
        },
        ImportWizardState::InputSource { profile_name, import_type, mut input_buffer } => match key.code {
            KeyCode::Esc => {
                app.profile_state.wizard = ImportWizardState::None;
            }
            KeyCode::Char(c) => {
                input_buffer.push(c);
                app.profile_state.wizard = ImportWizardState::InputSource {
                    profile_name,
                    import_type,
                    input_buffer,
                };
            }
            KeyCode::Backspace => {
                input_buffer.pop();
                app.profile_state.wizard = ImportWizardState::InputSource {
                    profile_name,
                    import_type,
                    input_buffer,
                };
            }
            KeyCode::Enter => {
                let src = input_buffer.trim().to_string();
                if !src.is_empty() {
                    let already_exists = app.config.profiles.contains_key(&profile_name);
                    if already_exists {
                        app.profile_state.wizard = ImportWizardState::ConfirmImportOverwrite {
                            profile_name: profile_name.clone(),
                            source_path_or_url: src,
                            import_type,
                        };
                    } else {
                        app.execute_import_profile(profile_name.clone(), src, import_type);
                    }
                }
            }
            _ => {}
        },
        ImportWizardState::ConfirmImportOverwrite { profile_name, source_path_or_url, import_type } => match key.code {
            KeyCode::Esc => {
                app.profile_state.wizard = ImportWizardState::None;
            }
            KeyCode::Enter => {
                app.execute_import_profile(profile_name.clone(), source_path_or_url.clone(), import_type);
            }
            _ => {}
        },
    }
}
