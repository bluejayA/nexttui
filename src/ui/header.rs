use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub struct HeaderContext {
    pub resource_type: String,
    pub cloud_name: String,
    pub region: String,
    pub all_tenants: bool,
}

pub struct Header;

impl Header {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, ctx: &HeaderContext) {
        // Badge-style labels: resource type + optional [ALL]
        let mut badges: Vec<Span> = vec![];

        // Resource type badge
        badges.push(Span::styled(
            format!(" {} ", ctx.resource_type),
            Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ));

        // All tenants badge
        if ctx.all_tenants {
            badges.push(Span::raw(" "));
            badges.push(Span::styled(
                " ALL ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        // Right-aligned cloud info badge
        let right_text = format!(" {} | {} ", ctx.cloud_name, ctx.region);
        let right = Span::styled(
            &right_text,
            Style::default()
                .fg(Color::White)
                .bg(Color::Rgb(60, 60, 60)),
        );

        // Calculate padding
        let badges_len: usize = badges.iter().map(|s| s.width()).sum();
        let padding_len = (area.width as usize)
            .saturating_sub(badges_len)
            .saturating_sub(right_text.len());
        let padding = " ".repeat(padding_len);

        badges.push(Span::raw(padding));
        badges.push(right);

        let line = Line::from(badges);
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
            all_tenants: false,
        };
        assert_eq!(ctx.resource_type, "Servers");
        assert_eq!(ctx.cloud_name, "prod");
    }
}
