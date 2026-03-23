use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub struct HeaderContext {
    pub resource_type: String,
    pub cloud_name: String,
    pub region: String,
}

pub struct Header;

impl Header {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, ctx: &HeaderContext) {
        let left = Span::styled(
            &ctx.resource_type,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
        let right = Span::styled(
            format!("{} | {}", ctx.cloud_name, ctx.region),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM),
        );
        // Pad middle with spaces
        let padding_len = area
            .width
            .saturating_sub(ctx.resource_type.len() as u16)
            .saturating_sub(ctx.cloud_name.len() as u16 + ctx.region.len() as u16 + 3)
            as usize;
        let padding = " ".repeat(padding_len);

        let line = Line::from(vec![left, Span::raw(padding), right]);
        let widget = Paragraph::new(line)
            .style(Style::default().bg(Color::DarkGray));
        frame.render_widget(widget, area);
    }
}

impl Default for Header {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_context_creation() {
        let ctx = HeaderContext {
            resource_type: "Servers".to_string(),
            cloud_name: "prod".to_string(),
            region: "RegionOne".to_string(),
        };
        assert_eq!(ctx.resource_type, "Servers");
        assert_eq!(ctx.cloud_name, "prod");
    }
}
