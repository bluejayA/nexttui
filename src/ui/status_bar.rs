use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::theme;

pub struct StatusInfo {
    pub panel_name: String,
    pub item_count: Option<usize>,
    pub selected_index: Option<usize>,
    pub context_hints: Vec<(String, String)>,
    pub error_badge_count: usize,
}

impl StatusInfo {
    /// Build left-side text: `[PanelName] idx/count` or `[PanelName]`.
    pub fn left_text(&self) -> String {
        if let (Some(count), Some(idx)) = (self.item_count, self.selected_index) {
            format!("[{}] {}/{}", self.panel_name, idx + 1, count)
        } else {
            format!("[{}]", self.panel_name)
        }
    }
}

pub struct StatusBar;

impl StatusBar {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, info: &StatusInfo) {
        // Paragraph bg applies to spans without explicit bg (ratatui style merge)
        let bg = Style::default().bg(Color::DarkGray).fg(Color::White);
        let left = info.left_text();

        // Error badge: " ⚠N" in red after left text
        let badge = if info.error_badge_count > 0 {
            format!(" \u{26A0}{}", info.error_badge_count)
        } else {
            String::new()
        };
        // Use char count for display width (⚠ is 1 column in most terminals)
        let badge_len = badge.chars().count();

        // Right: key hints using theme::key_hint()
        let mut hint_spans: Vec<Span> = Vec::new();
        for (i, (key, desc)) in info.context_hints.iter().enumerate() {
            if i > 0 {
                hint_spans.push(Span::raw("  "));
            }
            hint_spans.extend(theme::key_hint(key, desc));
        }
        let hint_plain_len: usize = hint_spans.iter().map(|s| s.content.len()).sum();

        let padding_len = (area.width as usize)
            .saturating_sub(left.len())
            .saturating_sub(badge_len)
            .saturating_sub(hint_plain_len);
        let padding = " ".repeat(padding_len);

        let mut spans = vec![Span::styled(&left, bg)];
        if info.error_badge_count > 0 {
            spans.push(Span::styled(
                badge,
                Style::default().bg(Color::DarkGray).fg(Color::Red),
            ));
        }
        spans.push(Span::styled(padding, bg));
        spans.extend(hint_spans);

        let line = Line::from(spans);
        let widget = Paragraph::new(line).style(bg);
        frame.render_widget(widget, area);
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_info() -> StatusInfo {
        StatusInfo {
            panel_name: "Servers".to_string(),
            item_count: Some(5),
            selected_index: Some(2),
            context_hints: vec![
                ("j/k".into(), "이동".into()),
                ("Enter".into(), "상세".into()),
            ],
            error_badge_count: 0,
        }
    }

    #[test]
    fn test_status_info_new_fields() {
        let info = sample_info();
        assert_eq!(info.panel_name, "Servers");
        assert_eq!(info.item_count, Some(5));
        assert_eq!(info.selected_index, Some(2));
        assert_eq!(info.context_hints.len(), 2);
        assert_eq!(info.context_hints[0], ("j/k".into(), "이동".into()));
    }

    #[test]
    fn test_status_info_left_text_with_count() {
        let info = sample_info();
        assert_eq!(info.left_text(), "[Servers] 3/5");
    }

    #[test]
    fn test_status_info_left_text_without_count() {
        let info = StatusInfo {
            panel_name: "Flavors".to_string(),
            item_count: None,
            selected_index: None,
            context_hints: vec![],
            error_badge_count: 0,
        };
        assert_eq!(info.left_text(), "[Flavors]");
    }

    #[test]
    fn test_error_badge_count_zero_no_badge_text() {
        let info = sample_info();
        let left = info.left_text();
        // With error_badge_count=0, no badge should appear in left text
        assert!(!left.contains('⚠'));
    }

    #[test]
    fn test_error_badge_count_positive_shows_badge() {
        let mut info = sample_info();
        info.error_badge_count = 3;
        // The badge is rendered in StatusBar::render, not in left_text.
        // We test the render output by checking the spans.
        // Use a buffer to capture render output.
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).ok();
        if let Some(ref mut term) = terminal {
            let _ = term.draw(|frame| {
                let area = frame.area();
                let bar = StatusBar::new();
                StatusBar::render(&bar, frame, area, &info);
            });
            let buf = term.backend().buffer().clone();
            let content: String = (0..buf.area.width)
                .map(|x| buf[(x, 0)].symbol().to_string())
                .collect();
            assert!(
                content.contains('⚠'),
                "badge should appear in rendered output: {content}"
            );
            assert!(
                content.contains('3'),
                "badge count should appear: {content}"
            );
        }
    }

    #[test]
    fn test_status_bar_key_hint_integration() {
        let spans = theme::key_hint("Tab", "패널");
        assert_eq!(spans.len(), 3); // key + separator + desc
        assert_eq!(spans[0].style.fg, Some(ratatui::style::Color::Cyan));
    }
}
