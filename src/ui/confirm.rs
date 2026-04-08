use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

use super::theme::Theme;
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
    detail_lines: Vec<String>,
}

impl ConfirmDialog {
    pub fn yes_no(message: impl Into<String>) -> Self {
        Self {
            mode: ConfirmMode::YesNo {
                message: message.into(),
            },
            active: true,
            detail_lines: Vec::new(),
        }
    }

    pub fn yes_no_with_details(message: impl Into<String>, details: Vec<String>) -> Self {
        Self {
            mode: ConfirmMode::YesNo {
                message: message.into(),
            },
            active: true,
            detail_lines: details,
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
            detail_lines: Vec::new(),
        }
    }

    pub fn type_to_confirm_with_details(
        message: impl Into<String>,
        expected: impl Into<String>,
        details: Vec<String>,
    ) -> Self {
        Self {
            mode: ConfirmMode::TypeToConfirm {
                message: message.into(),
                expected: expected.into(),
                buffer: String::new(),
            },
            active: true,
            detail_lines: details,
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

    pub fn detail_lines(&self) -> &[String] {
        &self.detail_lines
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

        // Calculate centered modal area (50% width, dynamic height)
        let width = (area.width / 2).max(30).min(area.width);
        let detail_count = self.detail_lines.len() as u16;
        let height = (7u16 + detail_count).min(area.height);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let modal_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, modal_area);

        let detail_style = Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM);

        let lines = match &self.mode {
            ConfirmMode::YesNo { message } => {
                let mut l = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        message.as_str(),
                        Theme::warning().add_modifier(Modifier::BOLD),
                    )),
                ];
                for detail in &self.detail_lines {
                    l.push(Line::from(Span::styled(detail.as_str(), detail_style)));
                }
                l.push(Line::from(""));
                l.push(Line::from(vec![
                    Span::styled("[Y]", Theme::focus_border().add_modifier(Modifier::BOLD)),
                    Span::styled("es  ", Style::default().fg(Color::White)),
                    Span::styled("[N]", Theme::focus_border().add_modifier(Modifier::BOLD)),
                    Span::styled("o", Style::default().fg(Color::White)),
                ]));
                l
            }
            ConfirmMode::TypeToConfirm {
                message,
                expected,
                buffer,
                ..
            } => {
                let mut l = vec![
                    Line::from(Span::styled(
                        message.as_str(),
                        Theme::warning().add_modifier(Modifier::BOLD),
                    )),
                ];
                for detail in &self.detail_lines {
                    l.push(Line::from(Span::styled(detail.as_str(), detail_style)));
                }
                l.push(Line::from(format!("Type '{expected}' to confirm:")));
                l.push(Line::from(""));
                l.push(Line::from(vec![
                    Span::raw("> "),
                    Span::styled(
                        buffer.as_str(),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled("_", Theme::waiting()),
                ]));
                l
            }
        };

        let block = Block::default()
            .title(" Confirm ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Theme::warning().add_modifier(Modifier::BOLD))
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

    // --- detail_lines tests ---

    #[test]
    fn test_yes_no_with_details_creates_dialog() {
        let details = vec!["Volume: vol-01".into(), "Size: 100GB".into()];
        let dialog = ConfirmDialog::yes_no_with_details("Detach volume?", details.clone());
        assert!(dialog.is_active());
        assert_eq!(dialog.message(), "Detach volume?");
        assert_eq!(dialog.detail_lines(), &details);
    }

    #[test]
    fn test_type_to_confirm_with_details_creates_dialog() {
        let details = vec!["Server: web-01".into(), "IP: 10.0.0.1".into()];
        let dialog = ConfirmDialog::type_to_confirm_with_details(
            "Type 'web-01' to delete",
            "web-01",
            details.clone(),
        );
        assert!(dialog.is_active());
        assert_eq!(dialog.message(), "Type 'web-01' to delete");
        assert_eq!(dialog.detail_lines(), &details);
    }

    #[test]
    fn test_yes_no_has_empty_details() {
        let dialog = ConfirmDialog::yes_no("Delete?");
        assert!(dialog.detail_lines().is_empty());
    }

    #[test]
    fn test_type_to_confirm_has_empty_details() {
        let dialog = ConfirmDialog::type_to_confirm("Confirm", "abc");
        assert!(dialog.detail_lines().is_empty());
    }

    #[test]
    fn test_yes_no_with_details_confirm_works() {
        let mut dialog = ConfirmDialog::yes_no_with_details(
            "Detach?",
            vec!["info".into()],
        );
        let result = dialog.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(result, ConfirmResult::Confirmed));
        assert!(!dialog.is_active());
    }

    #[test]
    fn test_type_to_confirm_with_details_confirm_works() {
        let mut dialog = ConfirmDialog::type_to_confirm_with_details(
            "Confirm",
            "abc",
            vec!["detail".into()],
        );
        for c in "abc".chars() {
            dialog.handle_key(key(KeyCode::Char(c)));
        }
        let result = dialog.handle_key(key(KeyCode::Enter));
        assert!(matches!(result, ConfirmResult::Confirmed));
    }
}
