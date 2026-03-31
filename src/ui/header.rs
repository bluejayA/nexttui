use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub struct HeaderContext {
    pub user_name: String,
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
        let (app, fill, right) = Self::build_header_line(area.width, ctx);
        let line = Line::from(vec![
            Span::styled(app, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(fill, Style::default().add_modifier(Modifier::DIM)),
            Span::styled(right, Style::default().add_modifier(Modifier::DIM)),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }

    /// Build header line parts: (app_name, fill, right_context).
    pub fn build_header_line(width: u16, ctx: &HeaderContext) -> (String, String, String) {
        let app_name = "nexttui".to_string();
        let right_text = if ctx.all_tenants {
            format!("[ALL] {}@{} | {}", ctx.user_name, ctx.cloud_name, ctx.region)
        } else {
            format!("{}@{} | {}", ctx.user_name, ctx.cloud_name, ctx.region)
        };
        let fill_len = (width as usize)
            .saturating_sub(app_name.len())
            .saturating_sub(right_text.len())
            .saturating_sub(2);
        let fill = format!(" {} ", "─".repeat(fill_len));
        (app_name, fill, right_text)
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

    fn sample_ctx() -> HeaderContext {
        HeaderContext {
            user_name: "admin".to_string(),
            cloud_name: "prod".to_string(),
            region: "RegionOne".to_string(),
            all_tenants: false,
        }
    }

    #[test]
    fn test_header_context_creation() {
        let ctx = sample_ctx();
        assert_eq!(ctx.user_name, "admin");
        assert_eq!(ctx.cloud_name, "prod");
    }

    #[test]
    fn test_header_app_name_is_nexttui() {
        let ctx = sample_ctx();
        let (app_name, _, _) = Header::build_header_line(80, &ctx);
        assert_eq!(app_name, "nexttui");
    }

    #[test]
    fn test_header_fill_char_is_dash() {
        let ctx = sample_ctx();
        let (_, fill, _) = Header::build_header_line(80, &ctx);
        assert!(fill.contains('─'), "fill should contain ─ character");
    }

    #[test]
    fn test_header_context_format() {
        let ctx = sample_ctx();
        let (_, _, right) = Header::build_header_line(80, &ctx);
        assert_eq!(right, "admin@prod | RegionOne");
    }

    #[test]
    fn test_header_all_tenants_prefix() {
        let ctx = HeaderContext {
            all_tenants: true,
            ..sample_ctx()
        };
        let (_, _, right) = Header::build_header_line(80, &ctx);
        assert!(right.starts_with("[ALL] "));
    }

    #[test]
    fn test_header_total_width_fits_80() {
        let ctx = sample_ctx();
        let (app, fill, right) = Header::build_header_line(80, &ctx);
        let total = app.chars().count() + fill.chars().count() + right.chars().count();
        assert_eq!(total, 80);
    }
}
