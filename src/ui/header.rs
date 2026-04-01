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
        let (app, fill, badge, right) = Self::build_header_parts(area.width, ctx);
        let mut spans = vec![
            Span::styled(app, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(fill, Style::default().add_modifier(Modifier::DIM)),
        ];
        if let Some(badge) = badge {
            spans.push(Span::styled(badge, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        }
        spans.push(Span::styled(right, Style::default().add_modifier(Modifier::DIM)));
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    /// Build header line parts: (app_name, fill, optional_badge, right_context).
    pub fn build_header_parts(width: u16, ctx: &HeaderContext) -> (String, String, Option<String>, String) {
        let app_name = "nexttui".to_string();
        let badge = if ctx.all_tenants { Some("[ALL] ".to_string()) } else { None };
        let right_text = format!("{}@{} | {}", ctx.user_name, ctx.cloud_name, ctx.region);
        let total_right = badge.as_ref().map_or(0, |b| b.len()) + right_text.len();
        let fill_len = (width as usize)
            .saturating_sub(app_name.len())
            .saturating_sub(total_right)
            .saturating_sub(2);
        let fill = format!(" {} ", "─".repeat(fill_len));
        (app_name, fill, badge, right_text)
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
        let (app_name, _, _, _) = Header::build_header_parts(80, &ctx);
        assert_eq!(app_name, "nexttui");
    }

    #[test]
    fn test_header_fill_char_is_dash() {
        let ctx = sample_ctx();
        let (_, fill, _, _) = Header::build_header_parts(80, &ctx);
        assert!(fill.contains('─'), "fill should contain ─ character");
    }

    #[test]
    fn test_header_context_format() {
        let ctx = sample_ctx();
        let (_, _, badge, right) = Header::build_header_parts(80, &ctx);
        assert!(badge.is_none());
        assert_eq!(right, "admin@prod | RegionOne");
    }

    #[test]
    fn test_header_all_tenants_badge_separate() {
        let ctx = HeaderContext {
            all_tenants: true,
            ..sample_ctx()
        };
        let (_, _, badge, right) = Header::build_header_parts(80, &ctx);
        assert_eq!(badge, Some("[ALL] ".to_string()));
        assert_eq!(right, "admin@prod | RegionOne");
    }

    #[test]
    fn test_header_total_width_fits_80() {
        let ctx = sample_ctx();
        let (app, fill, badge, right) = Header::build_header_parts(80, &ctx);
        let badge_len = badge.map_or(0, |b| b.chars().count());
        let total = app.chars().count() + fill.chars().count() + badge_len + right.chars().count();
        assert_eq!(total, 80);
    }

    #[test]
    fn test_header_total_width_fits_80_with_all_tenants() {
        let ctx = HeaderContext {
            all_tenants: true,
            ..sample_ctx()
        };
        let (app, fill, badge, right) = Header::build_header_parts(80, &ctx);
        let badge_len = badge.map_or(0, |b| b.chars().count());
        let total = app.chars().count() + fill.chars().count() + badge_len + right.chars().count();
        assert_eq!(total, 80);
    }
}
