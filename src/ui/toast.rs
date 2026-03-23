use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastSeverity {
    Success,
    Error,
    Info,
}

#[derive(Debug, Clone)]
pub struct ToastMessage {
    pub text: String,
    pub severity: ToastSeverity,
}

impl ToastMessage {
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            severity: ToastSeverity::Success,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            severity: ToastSeverity::Error,
        }
    }

    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            severity: ToastSeverity::Info,
        }
    }

    pub fn prefix(&self) -> &'static str {
        match self.severity {
            ToastSeverity::Success => "[OK]",
            ToastSeverity::Error => "[ERR]",
            ToastSeverity::Info => "[i]",
        }
    }

    pub fn color(&self) -> Color {
        match self.severity {
            ToastSeverity::Success => Color::Green,
            ToastSeverity::Error => Color::Red,
            ToastSeverity::Info => Color::Yellow,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let text = format!("{} {}", self.prefix(), self.text);
        let widget = Paragraph::new(Span::styled(
            text,
            Style::default().fg(Color::White).bg(self.color()),
        ));
        frame.render_widget(widget, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toast_constructors() {
        let t = ToastMessage::success("deleted");
        assert_eq!(t.severity, ToastSeverity::Success);
        assert_eq!(t.prefix(), "[OK]");
        assert_eq!(t.color(), Color::Green);

        let t = ToastMessage::error("failed");
        assert_eq!(t.severity, ToastSeverity::Error);
        assert_eq!(t.prefix(), "[ERR]");

        let t = ToastMessage::info("loading");
        assert_eq!(t.severity, ToastSeverity::Info);
        assert_eq!(t.prefix(), "[i]");
    }

    #[test]
    fn test_toast_text() {
        let t = ToastMessage::success("Server deleted");
        assert_eq!(t.text, "Server deleted");
    }
}
