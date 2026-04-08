use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use super::theme::{self, Theme};

/// Hint for visual styling of a select item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemHint {
    Normal,
    Current,
    Warning(String),
}

/// A selectable item in the popup.
#[derive(Debug, Clone)]
pub struct SelectItem {
    pub id: String,
    pub label: String,
    pub hint: ItemHint,
}

/// Result of handling a key event.
pub enum SelectResult {
    Selected(String),
    Cancelled,
    Pending,
}

/// Modal popup for selecting one item from a list.
pub struct SelectPopup {
    title: String,
    items: Vec<SelectItem>,
    all_items: Vec<SelectItem>,
    selected: usize,
    active: bool,
    search_query: Option<String>,
}

impl SelectPopup {
    pub fn new(title: impl Into<String>, items: Vec<SelectItem>) -> Self {
        let all_items = items.clone();
        Self {
            title: title.into(),
            items,
            all_items,
            selected: 0,
            active: true,
            search_query: None,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn items(&self) -> &[SelectItem] {
        &self.items
    }

    pub fn is_search_mode(&self) -> bool {
        self.search_query.is_some()
    }

    pub fn search_query(&self) -> Option<&str> {
        self.search_query.as_deref()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> SelectResult {
        if !self.active {
            return SelectResult::Pending;
        }

        if let Some(ref mut query) = self.search_query {
            // Search mode
            match key.code {
                KeyCode::Esc => {
                    self.search_query = None;
                    self.items = self.all_items.clone();
                    self.selected = 0;
                    SelectResult::Pending
                }
                KeyCode::Enter => {
                    if let Some(item) = self.items.get(self.selected) {
                        self.active = false;
                        SelectResult::Selected(item.id.clone())
                    } else {
                        SelectResult::Cancelled
                    }
                }
                KeyCode::Down => {
                    if self.selected + 1 < self.items.len() {
                        self.selected += 1;
                    }
                    SelectResult::Pending
                }
                KeyCode::Up => {
                    self.selected = self.selected.saturating_sub(1);
                    SelectResult::Pending
                }
                KeyCode::Backspace => {
                    query.pop();
                    self.apply_search_filter();
                    SelectResult::Pending
                }
                KeyCode::Char(c) => {
                    query.push(c);
                    self.apply_search_filter();
                    SelectResult::Pending
                }
                _ => SelectResult::Pending,
            }
        } else {
            // Normal mode
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.active = false;
                    SelectResult::Cancelled
                }
                KeyCode::Enter => {
                    if let Some(item) = self.items.get(self.selected) {
                        self.active = false;
                        SelectResult::Selected(item.id.clone())
                    } else {
                        SelectResult::Cancelled
                    }
                }
                KeyCode::Char('/') => {
                    self.search_query = Some(String::new());
                    SelectResult::Pending
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    if self.selected + 1 < self.items.len() {
                        self.selected += 1;
                    }
                    SelectResult::Pending
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.selected = self.selected.saturating_sub(1);
                    SelectResult::Pending
                }
                KeyCode::Char('g') => {
                    self.selected = 0;
                    SelectResult::Pending
                }
                KeyCode::Char('G') => {
                    if !self.items.is_empty() {
                        self.selected = self.items.len() - 1;
                    }
                    SelectResult::Pending
                }
                _ => SelectResult::Pending,
            }
        }
    }

    fn apply_search_filter(&mut self) {
        let query = self.search_query.as_deref().unwrap_or("");
        if query.is_empty() {
            self.items = self.all_items.clone();
        } else {
            let query_lower = query.to_lowercase();
            self.items = self
                .all_items
                .iter()
                .filter(|item| {
                    item.label.to_lowercase().contains(&query_lower)
                        || item.id.to_lowercase().contains(&query_lower)
                })
                .cloned()
                .collect();
        }
        self.selected = 0;
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.active || self.items.is_empty() {
            return;
        }

        // Modal dimensions
        let width = ((area.width as u32) * 60 / 100).min(area.width as u32) as u16;
        let width = width.max(40).min(area.width);
        let max_visible = ((area.height as u32 * 70 / 100) as usize).saturating_sub(4); // border + title + hint
        let visible_count = self.items.len().min(max_visible).max(1);
        let height = (visible_count as u16 + 4).min(area.height);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let modal_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, modal_area);

        // Scroll window
        let scroll_offset = if self.selected >= max_visible {
            self.selected - max_visible + 1
        } else {
            0
        };

        let mut lines = Vec::new();
        for (i, item) in self.items.iter().enumerate().skip(scroll_offset).take(visible_count) {
            let is_selected = i == self.selected;
            let prefix = if is_selected { ">> " } else { "   " };

            let mut spans = vec![];

            // Prefix
            if is_selected {
                spans.push(Span::styled(prefix, Theme::focus_border().add_modifier(Modifier::BOLD)));
            } else {
                spans.push(Span::raw(prefix));
            }

            // Label
            match &item.hint {
                ItemHint::Current => {
                    spans.push(Span::styled(&item.label, Theme::disabled()));
                    spans.push(Span::styled(" (current)", Theme::disabled()));
                }
                ItemHint::Warning(reason) => {
                    let style = if is_selected {
                        Theme::warning().add_modifier(Modifier::BOLD)
                    } else {
                        Theme::warning()
                    };
                    spans.push(Span::styled(&item.label, style));
                    spans.push(Span::styled(format!(" ⚠ {reason}"), Theme::warning()));
                }
                ItemHint::Normal => {
                    let style = if is_selected {
                        Theme::focus_border().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    spans.push(Span::styled(&item.label, style));
                }
            }

            lines.push(Line::from(spans));
        }

        // Bottom hint using theme::key_hint()
        lines.push(Line::from(""));
        let mut hint_spans = Vec::new();
        if self.search_query.is_some() {
            for (key, desc) in [("↑/↓", ":Move  "), ("Enter", ":Select  "), ("Esc", ":Back")] {
                hint_spans.extend(theme::key_hint(key, desc));
            }
        } else {
            for (key, desc) in [("j/k", ":Move  "), ("/", ":Search  "), ("Enter", ":Select  "), ("Esc", ":Cancel")] {
                hint_spans.extend(theme::key_hint(key, desc));
            }
        }
        lines.push(Line::from(hint_spans));

        let display_title = if let Some(ref query) = self.search_query {
            format!(" {} [/{}] ", self.title, query)
        } else {
            format!(" {} ", self.title)
        };

        let block = Block::default()
            .title(display_title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Theme::warning().add_modifier(Modifier::BOLD))
            .style(Style::default().bg(Color::Rgb(30, 30, 40)));
        let widget = Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(Color::Rgb(30, 30, 40)));
        frame.render_widget(widget, modal_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_items() -> Vec<SelectItem> {
        vec![
            SelectItem { id: "f1".into(), label: "m1.tiny (1vCPU / 512MB / 1GB)".into(), hint: ItemHint::Current },
            SelectItem { id: "f2".into(), label: "m1.small (1vCPU / 2048MB / 20GB)".into(), hint: ItemHint::Normal },
            SelectItem { id: "f3".into(), label: "m1.medium (2vCPU / 4096MB / 40GB)".into(), hint: ItemHint::Normal },
            SelectItem { id: "f4".into(), label: "m1.nano (1vCPU / 128MB / 0GB)".into(), hint: ItemHint::Warning("disk shrink".into()) },
        ]
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    #[test]
    fn test_new_popup_starts_at_first_item() {
        let popup = SelectPopup::new("Test", make_items());
        assert!(popup.is_active());
        assert_eq!(popup.selected_index(), 0);
        assert_eq!(popup.items().len(), 4);
    }

    #[test]
    fn test_navigate_down_up() {
        let mut popup = SelectPopup::new("Test", make_items());
        popup.handle_key(key(KeyCode::Char('j')));
        assert_eq!(popup.selected_index(), 1);
        popup.handle_key(key(KeyCode::Char('j')));
        assert_eq!(popup.selected_index(), 2);
        popup.handle_key(key(KeyCode::Char('k')));
        assert_eq!(popup.selected_index(), 1);
    }

    #[test]
    fn test_navigate_bounds() {
        let mut popup = SelectPopup::new("Test", make_items());
        popup.handle_key(key(KeyCode::Char('k'))); // already at 0
        assert_eq!(popup.selected_index(), 0);
        popup.handle_key(key(KeyCode::Char('G'))); // go to end
        assert_eq!(popup.selected_index(), 3);
        popup.handle_key(key(KeyCode::Char('j'))); // can't go past end
        assert_eq!(popup.selected_index(), 3);
        popup.handle_key(key(KeyCode::Char('g'))); // go to start
        assert_eq!(popup.selected_index(), 0);
    }

    #[test]
    fn test_enter_selects_item() {
        let mut popup = SelectPopup::new("Test", make_items());
        popup.handle_key(key(KeyCode::Char('j'))); // select index 1
        let result = popup.handle_key(key(KeyCode::Enter));
        assert!(matches!(result, SelectResult::Selected(id) if id == "f2"));
        assert!(!popup.is_active());
    }

    #[test]
    fn test_esc_cancels() {
        let mut popup = SelectPopup::new("Test", make_items());
        let result = popup.handle_key(key(KeyCode::Esc));
        assert!(matches!(result, SelectResult::Cancelled));
        assert!(!popup.is_active());
    }

    #[test]
    fn test_q_cancels() {
        let mut popup = SelectPopup::new("Test", make_items());
        let result = popup.handle_key(key(KeyCode::Char('q')));
        assert!(matches!(result, SelectResult::Cancelled));
        assert!(!popup.is_active());
    }

    #[test]
    fn test_enter_on_empty_cancels() {
        let mut popup = SelectPopup::new("Test", vec![]);
        let result = popup.handle_key(key(KeyCode::Enter));
        assert!(matches!(result, SelectResult::Cancelled));
    }

    #[test]
    fn test_inactive_popup_returns_pending() {
        let mut popup = SelectPopup::new("Test", make_items());
        popup.handle_key(key(KeyCode::Esc)); // deactivate
        let result = popup.handle_key(key(KeyCode::Enter));
        assert!(matches!(result, SelectResult::Pending));
    }

    #[test]
    fn test_arrow_keys_navigate() {
        let mut popup = SelectPopup::new("Test", make_items());
        popup.handle_key(key(KeyCode::Down));
        assert_eq!(popup.selected_index(), 1);
        popup.handle_key(key(KeyCode::Up));
        assert_eq!(popup.selected_index(), 0);
    }

    // --- Search mode tests ---

    #[test]
    fn test_slash_enters_search_mode() {
        let mut popup = SelectPopup::new("Test", make_items());
        let result = popup.handle_key(key(KeyCode::Char('/')));
        assert!(matches!(result, SelectResult::Pending));
        assert!(popup.is_search_mode());
    }

    #[test]
    fn test_search_filters_items_by_label() {
        let mut popup = SelectPopup::new("Test", make_items());
        popup.handle_key(key(KeyCode::Char('/'))); // enter search mode
        popup.handle_key(key(KeyCode::Char('t'))); // type 't' -> matches "m1.tiny"
        popup.handle_key(key(KeyCode::Char('i')));
        popup.handle_key(key(KeyCode::Char('n')));
        popup.handle_key(key(KeyCode::Char('y')));
        assert_eq!(popup.items().len(), 1);
        assert_eq!(popup.items()[0].id, "f1");
        assert_eq!(popup.selected_index(), 0);
    }

    #[test]
    fn test_search_filters_items_by_id() {
        let mut popup = SelectPopup::new("Test", make_items());
        popup.handle_key(key(KeyCode::Char('/'))); // enter search
        popup.handle_key(key(KeyCode::Char('f')));
        popup.handle_key(key(KeyCode::Char('3')));
        assert_eq!(popup.items().len(), 1);
        assert_eq!(popup.items()[0].id, "f3");
    }

    #[test]
    fn test_search_case_insensitive() {
        let mut popup = SelectPopup::new("Test", make_items());
        popup.handle_key(key(KeyCode::Char('/'))); // enter search
        popup.handle_key(key(KeyCode::Char('S'))); // uppercase S -> should match "m1.small"
        popup.handle_key(key(KeyCode::Char('M')));
        popup.handle_key(key(KeyCode::Char('A')));
        popup.handle_key(key(KeyCode::Char('L')));
        popup.handle_key(key(KeyCode::Char('L')));
        assert_eq!(popup.items().len(), 1);
        assert_eq!(popup.items()[0].id, "f2");
    }

    #[test]
    fn test_search_backspace_refilters() {
        let mut popup = SelectPopup::new("Test", make_items());
        popup.handle_key(key(KeyCode::Char('/'))); // enter search
        popup.handle_key(key(KeyCode::Char('t')));
        popup.handle_key(key(KeyCode::Char('i')));
        popup.handle_key(key(KeyCode::Char('n')));
        popup.handle_key(key(KeyCode::Char('y')));
        assert_eq!(popup.items().len(), 1);
        // backspace 4 times -> back to empty query, all items shown
        popup.handle_key(key(KeyCode::Backspace));
        popup.handle_key(key(KeyCode::Backspace));
        popup.handle_key(key(KeyCode::Backspace));
        popup.handle_key(key(KeyCode::Backspace));
        assert_eq!(popup.items().len(), 4);
    }

    #[test]
    fn test_search_esc_restores_all_items() {
        let mut popup = SelectPopup::new("Test", make_items());
        popup.handle_key(key(KeyCode::Char('/'))); // enter search
        popup.handle_key(key(KeyCode::Char('t')));
        popup.handle_key(key(KeyCode::Char('i')));
        assert!(popup.items().len() < 4); // filtered
        popup.handle_key(key(KeyCode::Esc)); // exit search
        assert!(!popup.is_search_mode());
        assert_eq!(popup.items().len(), 4); // restored
        assert!(popup.is_active()); // still active (not cancelled)
    }

    #[test]
    fn test_search_enter_selects_from_filtered() {
        let mut popup = SelectPopup::new("Test", make_items());
        popup.handle_key(key(KeyCode::Char('/'))); // enter search
        popup.handle_key(key(KeyCode::Char('m')));
        popup.handle_key(key(KeyCode::Char('e')));
        popup.handle_key(key(KeyCode::Char('d')));
        // should have filtered to "m1.medium"
        assert_eq!(popup.items().len(), 1);
        let result = popup.handle_key(key(KeyCode::Enter));
        assert!(matches!(result, SelectResult::Selected(id) if id == "f3"));
    }

    #[test]
    fn test_search_arrow_keys_navigate_filtered() {
        let mut popup = SelectPopup::new("Test", make_items());
        popup.handle_key(key(KeyCode::Char('/'))); // enter search
        popup.handle_key(key(KeyCode::Char('m'))); // matches all items starting with m1
        // all 4 items have "m1" in label
        assert!(popup.items().len() > 1);
        popup.handle_key(key(KeyCode::Down));
        assert_eq!(popup.selected_index(), 1);
        popup.handle_key(key(KeyCode::Up));
        assert_eq!(popup.selected_index(), 0);
    }

    #[test]
    fn test_search_j_k_are_text_input_not_navigation() {
        let mut popup = SelectPopup::new("Test", make_items());
        popup.handle_key(key(KeyCode::Char('/'))); // enter search
        popup.handle_key(key(KeyCode::Char('j'))); // should be text, not nav
        // 'j' is not in any item label/id, so filter should reduce
        // But it's treated as text input to search query
        assert!(popup.is_search_mode());
        // The query should be "j", not navigation
    }

    #[test]
    fn test_search_no_match_shows_empty() {
        let mut popup = SelectPopup::new("Test", make_items());
        popup.handle_key(key(KeyCode::Char('/'))); // enter search
        popup.handle_key(key(KeyCode::Char('z')));
        popup.handle_key(key(KeyCode::Char('z')));
        popup.handle_key(key(KeyCode::Char('z')));
        assert_eq!(popup.items().len(), 0);
    }

    #[test]
    fn test_search_enter_on_empty_filtered_cancels() {
        let mut popup = SelectPopup::new("Test", make_items());
        popup.handle_key(key(KeyCode::Char('/'))); // enter search
        popup.handle_key(key(KeyCode::Char('z')));
        popup.handle_key(key(KeyCode::Char('z')));
        let result = popup.handle_key(key(KeyCode::Enter));
        assert!(matches!(result, SelectResult::Cancelled));
    }

    #[test]
    fn test_normal_mode_slash_does_not_cancel() {
        let mut popup = SelectPopup::new("Test", make_items());
        popup.handle_key(key(KeyCode::Char('/')));
        assert!(popup.is_active());
        assert!(popup.is_search_mode());
    }

    #[test]
    fn test_search_query_display() {
        let popup_for_title = SelectPopup::new("Select Flavor", make_items());
        // Verify search_query accessor
        assert!(popup_for_title.search_query().is_none());

        let mut popup = SelectPopup::new("Select Flavor", make_items());
        popup.handle_key(key(KeyCode::Char('/')));
        assert_eq!(popup.search_query(), Some(""));
        popup.handle_key(key(KeyCode::Char('a')));
        assert_eq!(popup.search_query(), Some("a"));
    }
}
