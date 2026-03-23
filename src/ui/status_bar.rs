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

        let right = &info.help_hint;
        let padding_len = area
            .width
            .saturating_sub(left.len() as u16)
            .saturating_sub(right.len() as u16) as usize;
        let padding = " ".repeat(padding_len);

        let line = Line::from(vec![
            Span::styled(&left, Style::default().fg(Color::White)),
            Span::raw(padding),
            Span::styled(
                right.as_str(),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM),
            ),
        ]);
        let widget = Paragraph::new(line);
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
