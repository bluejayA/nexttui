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
                // `cursor_pos` is a byte index; step to the previous char
                // boundary so multibyte characters (e.g. Korean) are removed
                // atomically instead of panicking on a mid-codepoint index.
                if self.cursor_pos > 0 {
                    let new_pos = self.buffer[..self.cursor_pos]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.buffer.replace_range(new_pos..self.cursor_pos, "");
                    self.cursor_pos = new_pos;
                }
                if self.mode == InputMode::Search {
                    InputAction::SearchChanged(self.buffer.clone())
                } else {
                    InputAction::None
                }
            }
            KeyCode::Char(c) => {
                // Byte-aware length check: a 3-byte char must not straddle
                // the MAX_BUFFER_LEN boundary by inserting a partial sequence.
                if self.buffer.len() + c.len_utf8() > MAX_BUFFER_LEN {
                    return InputAction::None;
                }
                self.buffer.insert(self.cursor_pos, c);
                self.cursor_pos += c.len_utf8();
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

    // --- Codex adversarial review HIGH #3: UTF-8 cursor safety ---

    #[test]
    fn test_insert_multibyte_korean_no_panic() {
        let mut bar = InputBar::new();
        bar.activate(InputMode::Command);
        bar.handle_key(key(KeyCode::Char('가')));
        bar.handle_key(key(KeyCode::Char('나')));
        bar.handle_key(key(KeyCode::Char('다')));
        assert_eq!(bar.buffer(), "가나다");
    }

    #[test]
    fn test_backspace_multibyte_korean() {
        let mut bar = InputBar::new();
        bar.activate(InputMode::Command);
        bar.handle_key(key(KeyCode::Char('가')));
        bar.handle_key(key(KeyCode::Char('나')));
        bar.handle_key(key(KeyCode::Backspace));
        assert_eq!(bar.buffer(), "가");
        bar.handle_key(key(KeyCode::Backspace));
        assert_eq!(bar.buffer(), "");
    }

    #[test]
    fn test_mixed_ascii_and_multibyte() {
        let mut bar = InputBar::new();
        bar.activate(InputMode::Command);
        for c in "abc가나다xyz".chars() {
            bar.handle_key(key(KeyCode::Char(c)));
        }
        assert_eq!(bar.buffer(), "abc가나다xyz");
        for _ in 0..3 {
            bar.handle_key(key(KeyCode::Backspace));
        }
        assert_eq!(bar.buffer(), "abc가나다");
        // Backspace across the ASCII/CJK boundary should land on char boundary.
        bar.handle_key(key(KeyCode::Backspace));
        assert_eq!(bar.buffer(), "abc가나");
    }

    #[test]
    fn test_emoji_insert_and_backspace() {
        let mut bar = InputBar::new();
        bar.activate(InputMode::Command);
        bar.handle_key(key(KeyCode::Char('🚀')));
        assert_eq!(bar.buffer(), "🚀");
        bar.handle_key(key(KeyCode::Backspace));
        assert_eq!(bar.buffer(), "");
    }

    #[test]
    fn test_set_buffer_then_backspace_on_multibyte_tail() {
        let mut bar = InputBar::new();
        bar.activate(InputMode::Command);
        bar.set_buffer("servers 서울".to_string());
        // Backspace must step by one Korean char, not by one byte.
        bar.handle_key(key(KeyCode::Backspace));
        assert_eq!(bar.buffer(), "servers 서");
    }

    #[test]
    fn test_buffer_len_limit_respects_multibyte_width() {
        // 3-byte Korean char pushing the buffer past MAX_BUFFER_LEN must be
        // rejected atomically, without inserting a partial sequence.
        let mut bar = InputBar::new();
        bar.activate(InputMode::Command);
        bar.set_buffer("x".repeat(MAX_BUFFER_LEN - 1));
        // 3-byte insert would exceed MAX_BUFFER_LEN → rejected.
        bar.handle_key(key(KeyCode::Char('가')));
        assert_eq!(bar.buffer().len(), MAX_BUFFER_LEN - 1);
    }
}
