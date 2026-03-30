use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

const MAX_BUFFER_LEN: usize = 256;

/// Confirm dialog mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmMode {
    /// Simple Y/N confirmation.
    YesNo { message: String },
    /// Type resource name to confirm (destructive actions).
    TypeToConfirm {
        message: String,
        expected: String,
        buffer: String,
    },
}

pub enum ConfirmResult {
    Confirmed,
    Cancelled,
    Pending,
}

pub struct ConfirmDialog {
    mode: ConfirmMode,
    active: bool,
}

impl ConfirmDialog {
    pub fn yes_no(message: impl Into<String>) -> Self {
        Self {
            mode: ConfirmMode::YesNo {
                message: message.into(),
            },
            active: true,
        }
    }

    pub fn type_to_confirm(message: impl Into<String>, expected: impl Into<String>) -> Self {
        Self {
            mode: ConfirmMode::TypeToConfirm {
                message: message.into(),
                expected: expected.into(),
                buffer: String::new(),
            },
            active: true,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn message(&self) -> &str {
        match &self.mode {
            ConfirmMode::YesNo { message } => message,
            ConfirmMode::TypeToConfirm { message, .. } => message,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ConfirmResult {
        if !self.active {
            return ConfirmResult::Pending;
        }

        match &mut self.mode {
            ConfirmMode::YesNo { .. } => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.active = false;
                    ConfirmResult::Confirmed
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.active = false;
                    ConfirmResult::Cancelled
                }
                _ => ConfirmResult::Pending,
            },
            ConfirmMode::TypeToConfirm {
                expected, buffer, ..
            } => match key.code {
                KeyCode::Esc => {
                    self.active = false;
                    ConfirmResult::Cancelled
                }
                KeyCode::Enter => {
                    if buffer == expected {
                        self.active = false;
                        ConfirmResult::Confirmed
                    } else {
                        ConfirmResult::Pending
                    }
                }
                KeyCode::Backspace => {
                    buffer.pop();
                    ConfirmResult::Pending
                }
                KeyCode::Char(c) => {
                    if buffer.len() < MAX_BUFFER_LEN {
                        buffer.push(c);
                    }
                    ConfirmResult::Pending
                }
                _ => ConfirmResult::Pending,
            },
        }
    }

    /// Render the confirm dialog as a centered modal overlay.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.active {
            return;
        }

        // Calculate centered modal area (50% width, 5 lines tall)
        let width = (area.width / 2).max(30).min(area.width);
        let height = 7u16.min(area.height);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let modal_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, modal_area);

        let lines = match &self.mode {
            ConfirmMode::YesNo { message } => {
                vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        message.as_str(),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("[Y]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::styled("es  ", Style::default().fg(Color::White)),
                        Span::styled("[N]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::styled("o", Style::default().fg(Color::White)),
                    ]),
                ]
            }
            ConfirmMode::TypeToConfirm {
                message,
                expected,
                buffer,
                ..
            } => {
                vec![
                    Line::from(Span::styled(
                        message.as_str(),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!("Type '{expected}' to confirm:")),
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("> "),
                        Span::styled(
                            buffer.as_str(),
                            Style::default().fg(Color::White),
                        ),
                        Span::styled("_", Style::default().fg(Color::Gray)),
                    ]),
                ]
            }
        };

        let block = Block::default()
            .title(" Confirm ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(Color::Rgb(30, 30, 40)));
        let widget = Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Center)
            .style(Style::default().bg(Color::Rgb(30, 30, 40)));
        frame.render_widget(widget, modal_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    #[test]
    fn test_yes_no_confirm() {
        let mut dialog = ConfirmDialog::yes_no("Delete server?");
        assert!(dialog.is_active());
        let result = dialog.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(result, ConfirmResult::Confirmed));
        assert!(!dialog.is_active());
    }

    #[test]
    fn test_yes_no_cancel() {
        let mut dialog = ConfirmDialog::yes_no("Delete server?");
        let result = dialog.handle_key(key(KeyCode::Char('n')));
        assert!(matches!(result, ConfirmResult::Cancelled));
    }

    #[test]
    fn test_type_to_confirm_success() {
        let mut dialog = ConfirmDialog::type_to_confirm("Type 'web-01' to delete", "web-01");
        for c in "web-01".chars() {
            let result = dialog.handle_key(key(KeyCode::Char(c)));
            assert!(matches!(result, ConfirmResult::Pending));
        }
        let result = dialog.handle_key(key(KeyCode::Enter));
        assert!(matches!(result, ConfirmResult::Confirmed));
    }

    #[test]
    fn test_type_to_confirm_wrong_name() {
        let mut dialog = ConfirmDialog::type_to_confirm("Type 'web-01' to delete", "web-01");
        for c in "wrong".chars() {
            dialog.handle_key(key(KeyCode::Char(c)));
        }
        let result = dialog.handle_key(key(KeyCode::Enter));
        assert!(matches!(result, ConfirmResult::Pending));
        assert!(dialog.is_active());
    }

    #[test]
    fn test_backspace_in_type_to_confirm() {
        let mut dialog = ConfirmDialog::type_to_confirm("confirm", "abc");
        dialog.handle_key(key(KeyCode::Char('a')));
        dialog.handle_key(key(KeyCode::Char('b')));
        dialog.handle_key(key(KeyCode::Char('x')));
        dialog.handle_key(key(KeyCode::Backspace));
        dialog.handle_key(key(KeyCode::Char('c')));
        let result = dialog.handle_key(key(KeyCode::Enter));
        assert!(matches!(result, ConfirmResult::Confirmed));
    }

    #[test]
    fn test_inactive_ignores_keys() {
        let mut dialog = ConfirmDialog::yes_no("test");
        dialog.handle_key(key(KeyCode::Char('y'))); // confirms, now inactive
        let result = dialog.handle_key(key(KeyCode::Char('y'))); // should be ignored
        assert!(matches!(result, ConfirmResult::Pending));
    }
}
