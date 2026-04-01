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
    selected: usize,
    active: bool,
}

impl SelectPopup {
    pub fn new(title: impl Into<String>, items: Vec<SelectItem>) -> Self {
        Self {
            title: title.into(),
            items,
            selected: 0,
            active: true,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn items(&self) -> &[SelectItem] {
        &self.items
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> SelectResult {
        if !self.active {
            return SelectResult::Pending;
        }
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
        for (key, desc) in [("j/k", ":Move  "), ("Enter", ":Select  "), ("Esc", ":Cancel")] {
            hint_spans.extend(theme::key_hint(key, desc));
        }
        lines.push(Line::from(hint_spans));

        let block = Block::default()
            .title(format!(" {} ", self.title))
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
}
