use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

use super::theme::Theme;
use crate::context::types::ContextTarget;
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
    /// Rendered context line (e.g. "devstack • admin") shown above the message
    /// so destructive actions surface their target cloud/project explicitly.
    /// (Unit 5 Step 4.)
    context_fingerprint: Option<String>,
    /// Project name kept separately so [`require_recontext_confirm`] can
    /// escalate a YesNo into a TypeToConfirm that echoes the project.
    context_project: Option<String>,
}

impl ConfirmDialog {
    pub fn yes_no(message: impl Into<String>) -> Self {
        Self {
            mode: ConfirmMode::YesNo {
                message: message.into(),
            },
            active: true,
            detail_lines: Vec::new(),
            context_fingerprint: None,
            context_project: None,
        }
    }

    pub fn yes_no_with_details(message: impl Into<String>, details: Vec<String>) -> Self {
        Self {
            mode: ConfirmMode::YesNo {
                message: message.into(),
            },
            active: true,
            detail_lines: details,
            context_fingerprint: None,
            context_project: None,
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
            context_fingerprint: None,
            context_project: None,
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
            context_fingerprint: None,
            context_project: None,
        }
    }

    /// Attach a context fingerprint line (" {cloud} • {project} ") to the
    /// dialog so destructive actions surface the active target next to the
    /// prompt. The project name is also retained so
    /// [`Self::require_recontext_confirm`] can echo it as an expected typing
    /// token. (Unit 5 Step 4.)
    pub fn with_context_fingerprint(mut self, target: &ContextTarget) -> Self {
        self.context_fingerprint = Some(format!(" {} • {} ", target.cloud, target.project_name));
        self.context_project = Some(target.project_name.clone());
        self
    }

    /// Read-only accessor for tests / introspection.
    pub fn context_fingerprint(&self) -> Option<&str> {
        self.context_fingerprint.as_deref()
    }

    /// When `recently_switched` is true and a fingerprint has been attached,
    /// escalate a simple YesNo into a TypeToConfirm that echoes the project
    /// name. Rationale: right after a context switch, muscle-memory can
    /// approve a destructive action in the wrong project with one keystroke;
    /// demanding the project name forces a visual check against the
    /// fingerprint.
    ///
    /// No-op when `recently_switched` is false, the dialog is already
    /// TypeToConfirm (typing already required), or no fingerprint has been
    /// set (nothing to echo — caller should have set one).
    pub fn require_recontext_confirm(mut self, recently_switched: bool) -> Self {
        if !recently_switched {
            return self;
        }
        let Some(project) = self.context_project.clone() else {
            return self;
        };
        if let ConfirmMode::YesNo { message } = &self.mode {
            let new_message = format!("{message} (recently switched — confirm project)");
            self.mode = ConfirmMode::TypeToConfirm {
                message: new_message,
                expected: project,
                buffer: String::new(),
            };
        }
        self
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
        let fingerprint_rows = if self.context_fingerprint.is_some() {
            1
        } else {
            0
        };
        let height = (7u16 + detail_count + fingerprint_rows).min(area.height);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let modal_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, modal_area);

        let detail_style = Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM);

        let fingerprint_style = Theme::focus_border().add_modifier(Modifier::DIM);
        let lines = match &self.mode {
            ConfirmMode::YesNo { message } => {
                let mut l = vec![Line::from("")];
                if let Some(ref fp) = self.context_fingerprint {
                    l.push(Line::from(Span::styled(fp.as_str(), fingerprint_style)));
                }
                l.push(Line::from(Span::styled(
                    message.as_str(),
                    Theme::warning().add_modifier(Modifier::BOLD),
                )));
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
                let mut l: Vec<Line> = Vec::new();
                if let Some(ref fp) = self.context_fingerprint {
                    l.push(Line::from(Span::styled(fp.as_str(), fingerprint_style)));
                }
                l.push(Line::from(Span::styled(
                    message.as_str(),
                    Theme::warning().add_modifier(Modifier::BOLD),
                )));
                for detail in &self.detail_lines {
                    l.push(Line::from(Span::styled(detail.as_str(), detail_style)));
                }
                l.push(Line::from(format!("Type '{expected}' to confirm:")));
                l.push(Line::from(""));
                l.push(Line::from(vec![
                    Span::raw("> "),
                    Span::styled(buffer.as_str(), Style::default().fg(Color::White)),
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
        let mut dialog = ConfirmDialog::yes_no_with_details("Detach?", vec!["info".into()]);
        let result = dialog.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(result, ConfirmResult::Confirmed));
        assert!(!dialog.is_active());
    }

    #[test]
    fn test_type_to_confirm_with_details_confirm_works() {
        let mut dialog =
            ConfirmDialog::type_to_confirm_with_details("Confirm", "abc", vec!["detail".into()]);
        for c in "abc".chars() {
            dialog.handle_key(key(KeyCode::Char(c)));
        }
        let result = dialog.handle_key(key(KeyCode::Enter));
        assert!(matches!(result, ConfirmResult::Confirmed));
    }

    // --- Unit 5 Step 4: context fingerprint + recontext confirm ---

    use crate::context::types::ContextTarget;

    fn sample_target() -> ContextTarget {
        ContextTarget {
            cloud: "devstack".into(),
            project_id: "admin-id".into(),
            project_name: "admin".into(),
            domain: "default".into(),
        }
    }

    #[test]
    fn test_with_context_fingerprint_stores_line() {
        let dialog =
            ConfirmDialog::yes_no("Delete server?").with_context_fingerprint(&sample_target());
        let fp = dialog
            .context_fingerprint()
            .expect("fingerprint should be present");
        assert!(fp.contains("devstack"), "got: {fp}");
        assert!(fp.contains("admin"), "got: {fp}");
    }

    #[test]
    fn test_context_fingerprint_rendered_in_dialog() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let dialog =
            ConfirmDialog::yes_no("Delete server?").with_context_fingerprint(&sample_target());
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| dialog.render(f, f.area())).unwrap();
        let buf = terminal.backend().buffer();
        // Row-major concat so multi-char tokens on the same line survive.
        let mut rows: Vec<String> = Vec::new();
        for y in 0..buf.area.height {
            let row: String = (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect();
            rows.push(row);
        }
        let has_cloud = rows.iter().any(|r| r.contains("devstack"));
        let has_project = rows.iter().any(|r| r.contains("admin"));
        assert!(has_cloud, "fingerprint cloud missing: {rows:#?}");
        assert!(has_project, "fingerprint project missing: {rows:#?}");
    }

    #[test]
    fn test_require_recontext_confirm_converts_yes_no_to_type() {
        // recently_switched=true must escalate a YesNo into a TypeToConfirm
        // that demands the project name, so a post-switch destructive action
        // can't be accidentally confirmed with a single keystroke.
        let mut dialog = ConfirmDialog::yes_no("Delete server?")
            .with_context_fingerprint(&sample_target())
            .require_recontext_confirm(true);
        // `y` alone must not confirm — it's now just a buffered character.
        let r = dialog.handle_key(key(KeyCode::Char('y')));
        assert!(
            matches!(r, ConfirmResult::Pending),
            "YesNo should have been escalated to TypeToConfirm"
        );
        assert!(dialog.is_active());
        // Clear the stray `y` and type the project name to confirm.
        dialog.handle_key(key(KeyCode::Backspace));
        for c in "admin".chars() {
            dialog.handle_key(key(KeyCode::Char(c)));
        }
        let r = dialog.handle_key(key(KeyCode::Enter));
        assert!(matches!(r, ConfirmResult::Confirmed));
    }

    #[test]
    fn test_require_recontext_confirm_false_keeps_yes_no() {
        let mut dialog = ConfirmDialog::yes_no("Delete server?")
            .with_context_fingerprint(&sample_target())
            .require_recontext_confirm(false);
        let r = dialog.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(r, ConfirmResult::Confirmed));
    }

    #[test]
    fn test_require_recontext_confirm_without_fingerprint_is_noop() {
        // Without a fingerprint the indicator has no project name to echo,
        // so escalation is skipped (best-effort fail-open — the caller should
        // always pair `with_context_fingerprint` with `require_recontext_confirm`).
        let mut dialog = ConfirmDialog::yes_no("Delete server?").require_recontext_confirm(true);
        let r = dialog.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(r, ConfirmResult::Confirmed));
    }

    #[test]
    fn test_require_recontext_confirm_preserves_existing_type_to_confirm() {
        // TypeToConfirm already requires typing — recontext is a no-op.
        let mut dialog = ConfirmDialog::type_to_confirm("Type 'web-01' to delete", "web-01")
            .with_context_fingerprint(&sample_target())
            .require_recontext_confirm(true);
        for c in "web-01".chars() {
            dialog.handle_key(key(KeyCode::Char(c)));
        }
        let r = dialog.handle_key(key(KeyCode::Enter));
        assert!(matches!(r, ConfirmResult::Confirmed));
    }
}
