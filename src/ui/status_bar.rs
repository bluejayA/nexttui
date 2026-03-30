use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::toast::ToastMessage;

pub struct StatusInfo {
    pub message: String,
    pub help_hint: String,
    pub item_count: Option<usize>,
    pub selected_index: Option<usize>,
}

pub struct StatusBar;

impl StatusBar {
    pub fn new() -> Self {
        Self
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        info: &StatusInfo,
        active_toasts: &[ToastMessage],
    ) {
        // Toast takes priority
        if let Some(toast) = active_toasts.first() {
            toast.render(frame, area);
            return;
        }

        let left = if let (Some(count), Some(idx)) = (info.item_count, info.selected_index) {
            format!("{} | {}/{}", info.message, idx + 1, count)
        } else {
            info.message.clone()
        };

        // Build styled hint: keys in Cyan+Bold, descriptions in Gray
        let hint_spans = Self::style_hint(&info.help_hint);
        let hint_plain_len: usize = hint_spans.iter().map(|s| s.content.len()).sum();

        let padding_len = area
            .width
            .saturating_sub(left.len() as u16)
            .saturating_sub(hint_plain_len as u16) as usize;
        let padding = " ".repeat(padding_len);

        let mut spans = vec![
            Span::styled(&left, Style::default().fg(Color::White)),
            Span::raw(padding),
        ];
        spans.extend(hint_spans);

        let line = Line::from(spans);
        let widget = Paragraph::new(line);
        frame.render_widget(widget, area);
    }

    /// Parse "Key:Label Key:Label" into styled spans.
    /// Keys → Cyan+Bold, labels → Gray, separators → dark.
    fn style_hint(hint: &str) -> Vec<Span<'_>> {
        let mut spans = Vec::new();
        for (i, part) in hint.split(' ').enumerate() {
            if i > 0 {
                spans.push(Span::styled(" ", Style::default().fg(Color::DarkGray)));
            }
            if let Some((key, label)) = part.split_once(':') {
                spans.push(Span::styled(
                    key,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    format!(":{label}"),
                    Style::default().fg(Color::Gray),
                ));
            } else {
                spans.push(Span::styled(part, Style::default().fg(Color::Gray)));
            }
        }
        spans
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

    #[test]
    fn test_status_info_creation() {
        let info = StatusInfo {
            message: "5 servers loaded".to_string(),
            help_hint: ":help | j/k | Enter".to_string(),
            item_count: Some(5),
            selected_index: Some(2),
        };
        assert_eq!(info.item_count, Some(5));
        assert_eq!(info.selected_index, Some(2));
    }
}
