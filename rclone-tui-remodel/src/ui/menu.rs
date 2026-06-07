use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

pub struct MenuState {
    pub selected_idx: usize,
    pub options: Vec<&'static str>,
}

impl MenuState {
    pub fn new() -> Self {
        MenuState {
            selected_idx: 0,
            options: vec![
                "menu_1",
                "menu_2",
                "menu_3",
                "menu_4",
                "menu_5",
                "menu_6",
                "menu_install_dep",
                "menu_7",
            ],
        }
    }

    pub fn next(&mut self) {
        self.selected_idx = (self.selected_idx + 1) % self.options.len();
    }

    pub fn prev(&mut self) {
        if self.selected_idx == 0 {
            self.selected_idx = self.options.len() - 1;
        } else {
            self.selected_idx -= 1;
        }
    }
}

pub fn draw(state: &MenuState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Banner hướng dẫn
            Constraint::Min(10),   // Menu options
        ])
        .split(area);

    // Banner chào mừng
    let welcome_text = crate::lang::translate("menu_welcome");
    let welcome = Paragraph::new(welcome_text)
        .style(Style::default())
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(welcome, chunks[0]);

    // Các phần tử Menu
    let items: Vec<ListItem> = state
        .options
        .iter()
        .enumerate()
        .map(|(i, key)| {
            let style = if i == state.selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let text = crate::lang::translate(key);
            ListItem::new(text).style(style)
        })
        .collect();

    let title_text = crate::lang::translate("menu_title");
    let list = List::new(items).block(
        Block::default()
            .title(title_text)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(list, chunks[1]);
}
