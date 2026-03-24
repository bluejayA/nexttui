use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::action::Action;
use crate::models::common::Route;

#[derive(Debug, Clone)]
pub struct SidebarItem {
    pub label: String,
    pub route: Route,
    pub shortcut: String,
    pub admin_only: bool,
}

pub struct Sidebar {
    items: Vec<SidebarItem>,
    selected_index: usize,
}

impl Sidebar {
    pub fn new(items: Vec<SidebarItem>) -> Self {
        Self {
            items,
            selected_index: 0,
        }
    }

    /// Filter items by admin visibility.
    pub fn visible_items(&self, is_admin: bool) -> Vec<&SidebarItem> {
        self.items
            .iter()
            .filter(|item| !item.admin_only || is_admin)
            .collect()
    }

    /// Handle j/k/Enter keys. Returns Action::Navigate on Enter.
    pub fn handle_key(&mut self, key: KeyEvent, is_admin: bool) -> Option<Action> {
        let visible_count = self.visible_items(is_admin).len();
        if visible_count == 0 {
            return None;
        }
        let max = visible_count.saturating_sub(1);
        // Clamp selected_index if visible list shrank (e.g., admin→non-admin switch)
        if self.selected_index > max {
            self.selected_index = max;
        }

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.selected_index < max {
                    self.selected_index += 1;
                }
                None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected_index = self.selected_index.saturating_sub(1);
                None
            }
            KeyCode::Enter | KeyCode::Right => {
                let visible = self.visible_items(is_admin);
                let route = visible.get(self.selected_index).map(|item| item.route);
                route.map(Action::Navigate)
            }
            _ => None,
        }
    }

    /// Sync selected index to match current route.
    pub fn sync_active(&mut self, current_route: &Route, is_admin: bool) {
        let visible = self.visible_items(is_admin);
        if let Some(idx) = visible.iter().position(|item| &item.route == current_route) {
            self.selected_index = idx;
        }
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        is_admin: bool,
        current_route: &Route,
        focused: bool,
    ) {
        let visible = self.visible_items(is_admin);
        let items: Vec<ListItem> = visible
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let marker = if &item.route == current_route {
                    ">"
                } else {
                    " "
                };
                let style = if i == self.selected_index && focused {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else if i == self.selected_index {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else if item.admin_only {
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::DIM)
                } else {
                    Style::default().fg(Color::White)
                };
                let line = Line::from(Span::styled(
                    format!("{marker} {}", item.label),
                    style,
                ));
                ListItem::new(line)
            })
            .collect();

        let block = Block::default()
            .title(" Modules ")
            .borders(Borders::RIGHT);
        let list = List::new(items).block(block);
        let mut state = ListState::default();
        state.select(Some(self.selected_index));
        frame.render_stateful_widget(list, area, &mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_items() -> Vec<SidebarItem> {
        vec![
            SidebarItem {
                label: "Servers".into(),
                route: Route::Servers,
                shortcut: ":srv".into(),
                admin_only: false,
            },
            SidebarItem {
                label: "Networks".into(),
                route: Route::Networks,
                shortcut: ":net".into(),
                admin_only: false,
            },
            SidebarItem {
                label: "Projects".into(),
                route: Route::Projects,
                shortcut: ":proj".into(),
                admin_only: true,
            },
        ]
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    #[test]
    fn test_visible_items_non_admin() {
        let sidebar = Sidebar::new(sample_items());
        let visible = sidebar.visible_items(false);
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].label, "Servers");
        assert_eq!(visible[1].label, "Networks");
    }

    #[test]
    fn test_visible_items_admin() {
        let sidebar = Sidebar::new(sample_items());
        let visible = sidebar.visible_items(true);
        assert_eq!(visible.len(), 3);
    }

    #[test]
    fn test_handle_key_j_k() {
        let mut sidebar = Sidebar::new(sample_items());
        assert_eq!(sidebar.selected_index(), 0);

        sidebar.handle_key(key(KeyCode::Char('j')), true);
        assert_eq!(sidebar.selected_index(), 1);

        sidebar.handle_key(key(KeyCode::Char('j')), true);
        assert_eq!(sidebar.selected_index(), 2);

        // At max, should not go further
        sidebar.handle_key(key(KeyCode::Char('j')), true);
        assert_eq!(sidebar.selected_index(), 2);

        sidebar.handle_key(key(KeyCode::Char('k')), true);
        assert_eq!(sidebar.selected_index(), 1);
    }

    #[test]
    fn test_handle_key_enter() {
        let mut sidebar = Sidebar::new(sample_items());
        sidebar.handle_key(key(KeyCode::Char('j')), true);
        let action = sidebar.handle_key(key(KeyCode::Enter), true);
        assert!(matches!(action, Some(Action::Navigate(Route::Networks))));
    }

    #[test]
    fn test_sync_active() {
        let mut sidebar = Sidebar::new(sample_items());
        sidebar.sync_active(&Route::Networks, true);
        assert_eq!(sidebar.selected_index(), 1);
    }

    #[test]
    fn test_non_admin_bounds() {
        let mut sidebar = Sidebar::new(sample_items());
        // Non-admin: only 2 items
        sidebar.handle_key(key(KeyCode::Char('j')), false);
        sidebar.handle_key(key(KeyCode::Char('j')), false);
        // Should stay at index 1 (max for 2 items)
        assert_eq!(sidebar.selected_index(), 1);
    }
}
