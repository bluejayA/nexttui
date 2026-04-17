use crossterm::event::{KeyCode, KeyEvent};

const MAX_BUFFER_LEN: usize = 256;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::theme::Theme;
// Single source of truth for input modes (BL-P2-073). The widget only reacts
// to `Command` and `Search`; other variants (Normal / Form / Confirm) leave
// the bar inert and render the default hint.
use crate::component::InputMode;

#[derive(Debug)]
pub enum InputAction {
    Commit(String),
    Cancel,
    AutoComplete,
    HistoryUp,
    HistoryDown,
    SearchChanged(String),
    None,
}

pub struct InputBar {
    mode: InputMode,
    buffer: String,
    cursor_pos: usize,
}

impl InputBar {
    pub fn new() -> Self {
        Self {
            mode: InputMode::Normal,
            buffer: String::new(),
            cursor_pos: 0,
        }
    }

    pub fn mode(&self) -> &InputMode {
        &self.mode
    }

    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    pub fn activate(&mut self, mode: InputMode) {
        self.mode = mode;
        self.buffer.clear();
        self.cursor_pos = 0;
    }

    pub fn deactivate(&mut self) {
        self.mode = InputMode::Normal;
        self.buffer.clear();
        self.cursor_pos = 0;
    }

    pub fn set_buffer(&mut self, value: String) {
        self.cursor_pos = value.len();
        self.buffer = value;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> InputAction {
        // Only Command / Search modes capture input here; everything else is
        // inert (Form is handled by its own widget, Confirm by ConfirmDialog).
        if !matches!(self.mode, InputMode::Command | InputMode::Search) {
            return InputAction::None;
        }

        match key.code {
            KeyCode::Enter => {
                let value = self.buffer.clone();
                self.deactivate();
                InputAction::Commit(value)
            }
            KeyCode::Esc => {
                self.deactivate();
                InputAction::Cancel
            }
            KeyCode::Tab => InputAction::AutoComplete,
            KeyCode::Up => InputAction::HistoryUp,
            KeyCode::Down => InputAction::HistoryDown,
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.buffer.remove(self.cursor_pos);
                }
                if self.mode == InputMode::Search {
                    InputAction::SearchChanged(self.buffer.clone())
                } else {
                    InputAction::None
                }
            }
            KeyCode::Char(c) => {
                if self.buffer.len() >= MAX_BUFFER_LEN {
                    return InputAction::None;
                }
                self.buffer.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
                if self.mode == InputMode::Search {
                    InputAction::SearchChanged(self.buffer.clone())
                } else {
                    InputAction::None
                }
            }
            _ => InputAction::None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let (prefix, style) = match self.mode {
            InputMode::Command => (":", Theme::warning()),
            InputMode::Search => ("/", Theme::focus_border()),
            _ => {
                let hint = "Press : for command, / for search";
                let widget = Paragraph::new(Span::styled(hint, Theme::disabled()));
                frame.render_widget(widget, area);
                return;
            }
        };

        let line = Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(&self.buffer, Style::default().fg(Color::White)),
            Span::styled("_", Theme::waiting()),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }
}

impl Default for InputBar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    #[test]
    fn test_initial_state() {
        let bar = InputBar::new();
        assert_eq!(*bar.mode(), InputMode::Normal);
        assert_eq!(bar.buffer(), "");
    }

    #[test]
    fn test_activate_deactivate() {
        let mut bar = InputBar::new();
        bar.activate(InputMode::Command);
        assert_eq!(*bar.mode(), InputMode::Command);
        bar.deactivate();
        assert_eq!(*bar.mode(), InputMode::Normal);
    }

    #[test]
    fn test_char_input_command() {
        let mut bar = InputBar::new();
        bar.activate(InputMode::Command);
        let action = bar.handle_key(key(KeyCode::Char('s')));
        assert!(matches!(action, InputAction::None));
        bar.handle_key(key(KeyCode::Char('r')));
        bar.handle_key(key(KeyCode::Char('v')));
        assert_eq!(bar.buffer(), "srv");
    }

    #[test]
    fn test_char_input_search_emits_changed() {
        let mut bar = InputBar::new();
        bar.activate(InputMode::Search);
        let action = bar.handle_key(key(KeyCode::Char('w')));
        match action {
            InputAction::SearchChanged(s) => assert_eq!(s, "w"),
            _ => panic!("expected SearchChanged"),
        }
    }

    #[test]
    fn test_enter_commits() {
        let mut bar = InputBar::new();
        bar.activate(InputMode::Command);
        bar.handle_key(key(KeyCode::Char('h')));
        bar.handle_key(key(KeyCode::Char('i')));
        let action = bar.handle_key(key(KeyCode::Enter));
        match action {
            InputAction::Commit(s) => assert_eq!(s, "hi"),
            _ => panic!("expected Commit"),
        }
        assert_eq!(*bar.mode(), InputMode::Normal);
    }

    #[test]
    fn test_esc_cancels() {
        let mut bar = InputBar::new();
        bar.activate(InputMode::Command);
        bar.handle_key(key(KeyCode::Char('x')));
        let action = bar.handle_key(key(KeyCode::Esc));
        assert!(matches!(action, InputAction::Cancel));
        assert_eq!(*bar.mode(), InputMode::Normal);
        assert_eq!(bar.buffer(), "");
    }

    #[test]
    fn test_backspace() {
        let mut bar = InputBar::new();
        bar.activate(InputMode::Command);
        bar.handle_key(key(KeyCode::Char('a')));
        bar.handle_key(key(KeyCode::Char('b')));
        bar.handle_key(key(KeyCode::Backspace));
        assert_eq!(bar.buffer(), "a");
    }
}
