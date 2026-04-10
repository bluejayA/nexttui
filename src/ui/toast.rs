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

impl From<crate::background::ToastLevel> for ToastSeverity {
    fn from(level: crate::background::ToastLevel) -> Self {
        match level {
            crate::background::ToastLevel::Success => Self::Success,
            crate::background::ToastLevel::Error => Self::Error,
            crate::background::ToastLevel::Info => Self::Info,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToastMessage {
    pub text: String,
    pub severity: ToastSeverity,
    pub resource_id: Option<String>,
}

impl ToastMessage {
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            severity: ToastSeverity::Success,
            resource_id: None,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            severity: ToastSeverity::Error,
            resource_id: None,
        }
    }

    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            severity: ToastSeverity::Info,
            resource_id: None,
        }
    }

    pub fn prefix(&self) -> &'static str {
        match self.severity {
            ToastSeverity::Success => "[OK]",
            ToastSeverity::Error => "[ERR]",
            ToastSeverity::Info => "[i]",
        }
    }

    pub fn style(&self) -> Style {
        use super::theme::Theme;
        match self.severity {
            ToastSeverity::Success => Theme::done(),
            ToastSeverity::Error => Theme::error(),
            ToastSeverity::Info => Theme::warning(),
        }
    }

    /// 80-col 터미널 기준, 좌우 패딩/보더 여유분을 제외한 최대 표시 길이
    const MAX_DISPLAY_LEN: usize = 75;

    pub fn display_text(&self) -> String {
        let raw = match &self.resource_id {
            Some(id) => format!("{} {}: {}", self.prefix(), id, self.text),
            None => format!("{} {}", self.prefix(), self.text),
        };
        if raw.chars().count() > Self::MAX_DISPLAY_LEN {
            let truncated: String = raw.chars().take(Self::MAX_DISPLAY_LEN - 1).collect();
            format!("{truncated}…")
        } else {
            raw
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let text = self.display_text();
        let bg_color = self.style().fg.unwrap_or(Color::White);
        let widget = Paragraph::new(Span::styled(
            text,
            Style::default().fg(Color::White).bg(bg_color),
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

    #[test]
    fn test_toast_resource_id_field() {
        let t = ToastMessage {
            text: "Resize confirmed".to_string(),
            severity: ToastSeverity::Success,
            resource_id: Some("server-01".to_string()),
        };
        assert_eq!(t.resource_id, Some("server-01".to_string()));
    }

    #[test]
    fn test_toast_with_resource_id_format() {
        let t = ToastMessage {
            text: "Resize confirmed".to_string(),
            severity: ToastSeverity::Success,
            resource_id: Some("server-01".to_string()),
        };
        assert_eq!(t.display_text(), "[OK] server-01: Resize confirmed");
    }

    #[test]
    fn test_toast_without_resource_id_format() {
        let t = ToastMessage::success("Loading servers...");
        assert_eq!(t.display_text(), "[OK] Loading servers...");
    }

    #[test]
    fn test_toast_truncation_75_chars() {
        let long_text = "a".repeat(80);
        let t = ToastMessage::info(&long_text);
        let display = t.display_text();
        let char_count = display.chars().count();
        assert!(char_count <= 75, "display_text should be <= 75 chars, got {}", char_count);
        assert!(display.ends_with('…'));
    }

    #[test]
    fn test_toast_truncation_preserves_short() {
        let t = ToastMessage::info("short message");
        let display = t.display_text();
        assert_eq!(display, "[i] short message");
        assert!(!display.contains('…'));
    }

    #[test]
    fn test_toast_color_uses_theme() {
        use crate::ui::theme::Theme;
        Theme::init_with_no_color(crate::config::ThemeVariant::Dark, false);
        let success = ToastMessage::success("ok");
        assert_eq!(success.style(), Theme::done());
        let error = ToastMessage::error("fail");
        assert_eq!(error.style(), Theme::error());
        let info = ToastMessage::info("note");
        assert_eq!(info.style(), Theme::warning());
    }

    #[test]
    fn test_toast_truncation_boundary_75_chars() {
        // prefix "[i] " = 4 chars, so text of 71 chars = exactly 75 total → no truncation
        let exact = "a".repeat(71);
        let t = ToastMessage::info(&exact);
        let display = t.display_text();
        assert_eq!(display.chars().count(), 75);
        assert!(!display.ends_with('…'));

        // 72 chars text = 76 total → truncation
        let over = "a".repeat(72);
        let t = ToastMessage::info(&over);
        let display = t.display_text();
        assert!(display.ends_with('…'));
        assert_eq!(display.chars().count(), 75);
    }
}
