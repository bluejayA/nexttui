use std::sync::atomic::{AtomicU8, Ordering};

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::config::ThemeVariant;

// Packed into a single u8: bit 0 = variant (0=Dark, 1=Light), bit 1 = no_color
static THEME_STATE: AtomicU8 = AtomicU8::new(0); // default: Dark, color enabled

fn variant() -> ThemeVariant {
    if THEME_STATE.load(Ordering::Relaxed) & 0x01 == 0 {
        ThemeVariant::Dark
    } else {
        ThemeVariant::Light
    }
}

fn no_color() -> bool {
    THEME_STATE.load(Ordering::Relaxed) & 0x02 != 0
}

/// Build a style, stripping colors when NO_COLOR is active.
fn styled(fg: Option<Color>, bg: Option<Color>, modifiers: Modifier) -> Style {
    if no_color() {
        Style::default().add_modifier(modifiers)
    } else {
        let mut s = Style::default().add_modifier(modifiers);
        if let Some(c) = fg {
            s = s.fg(c);
        }
        if let Some(c) = bg {
            s = s.bg(c);
        }
        s
    }
}

pub struct Theme;

impl Theme {
    /// Initialize the global theme. Call once at app startup.
    /// Detects `NO_COLOR` environment variable automatically.
    pub fn init(v: ThemeVariant) {
        let nc = std::env::var("NO_COLOR").is_ok();
        Self::init_with_no_color(v, nc);
    }

    /// Initialize with explicit no_color flag (for testing).
    pub fn init_with_no_color(v: ThemeVariant, nc: bool) {
        let mut bits: u8 = 0;
        if matches!(v, ThemeVariant::Light) {
            bits |= 0x01;
        }
        if nc {
            bits |= 0x02;
        }
        THEME_STATE.store(bits, Ordering::Relaxed);
    }

    pub fn active() -> Style {
        match variant() {
            ThemeVariant::Dark => styled(Some(Color::Yellow), None, Modifier::BOLD),
            ThemeVariant::Light => styled(Some(Color::Rgb(180, 120, 0)), None, Modifier::BOLD),
        }
    }

    /// ACTIVE servers / success states
    pub fn done() -> Style {
        match variant() {
            ThemeVariant::Dark => styled(Some(Color::LightGreen), None, Modifier::empty()),
            ThemeVariant::Light => styled(Some(Color::Rgb(0, 130, 0)), None, Modifier::empty()),
        }
    }

    /// ERROR states
    pub fn error() -> Style {
        match variant() {
            ThemeVariant::Dark => styled(Some(Color::LightRed), None, Modifier::empty()),
            ThemeVariant::Light => styled(Some(Color::Rgb(180, 0, 0)), None, Modifier::empty()),
        }
    }

    /// SHUTOFF / stopped
    pub fn waiting() -> Style {
        match variant() {
            ThemeVariant::Dark => styled(Some(Color::Gray), None, Modifier::empty()),
            ThemeVariant::Light => styled(Some(Color::DarkGray), None, Modifier::empty()),
        }
    }

    pub fn warning() -> Style {
        match variant() {
            ThemeVariant::Dark => styled(Some(Color::Yellow), None, Modifier::empty()),
            ThemeVariant::Light => styled(Some(Color::Rgb(180, 120, 0)), None, Modifier::empty()),
        }
    }

    pub fn focus_border() -> Style {
        match variant() {
            ThemeVariant::Dark => styled(Some(Color::Cyan), None, Modifier::empty()),
            ThemeVariant::Light => styled(Some(Color::Blue), None, Modifier::empty()),
        }
    }

    pub fn unfocus_border() -> Style {
        match variant() {
            ThemeVariant::Dark => styled(Some(Color::DarkGray), None, Modifier::empty()),
            ThemeVariant::Light => styled(Some(Color::Gray), None, Modifier::empty()),
        }
    }

    /// Selected row highlight
    pub fn highlight() -> Style {
        match variant() {
            ThemeVariant::Dark => styled(None, Some(Color::Rgb(50, 50, 76)), Modifier::BOLD),
            ThemeVariant::Light => styled(None, Some(Color::Rgb(210, 220, 240)), Modifier::BOLD),
        }
    }

    pub fn disabled() -> Style {
        match variant() {
            ThemeVariant::Dark => styled(Some(Color::Gray), None, Modifier::DIM),
            ThemeVariant::Light => styled(Some(Color::Gray), None, Modifier::DIM),
        }
    }

    pub fn link() -> Style {
        match variant() {
            ThemeVariant::Dark => styled(Some(Color::Cyan), None, Modifier::UNDERLINED),
            ThemeVariant::Light => styled(Some(Color::Blue), None, Modifier::UNDERLINED),
        }
    }

    pub fn timestamp() -> Style {
        match variant() {
            ThemeVariant::Dark => styled(Some(Color::Cyan), None, Modifier::empty()),
            ThemeVariant::Light => styled(Some(Color::Blue), None, Modifier::empty()),
        }
    }

    pub fn evacuating() -> Style {
        match variant() {
            ThemeVariant::Dark => styled(Some(Color::Yellow), None, Modifier::empty()),
            ThemeVariant::Light => styled(Some(Color::Rgb(180, 120, 0)), None, Modifier::empty()),
        }
    }

    pub fn evac_success() -> Style {
        match variant() {
            ThemeVariant::Dark => styled(Some(Color::LightGreen), None, Modifier::BOLD),
            ThemeVariant::Light => styled(Some(Color::Rgb(0, 130, 0)), None, Modifier::BOLD),
        }
    }
}

pub struct Icons;

impl Icons {
    pub fn active() -> &'static str {
        "●"
    }

    pub fn shutoff() -> &'static str {
        "○"
    }

    pub fn error() -> &'static str {
        "✗"
    }

    pub fn building() -> &'static str {
        "⟳"
    }

    pub fn verify() -> &'static str {
        "◐"
    }

    pub fn migrating() -> &'static str {
        "↔"
    }

    pub fn host_up() -> &'static str { "●" }
    pub fn host_down() -> &'static str { "✗" }
    pub fn host_disabled() -> &'static str { "⊘" }
    pub fn checkbox_on() -> &'static str { "☑" }
    pub fn checkbox_off() -> &'static str { "☐" }

    pub fn status_icon(status: &str) -> &'static str {
        match status {
            "ACTIVE" => Self::active(),
            "SHUTOFF" | "STOPPED" => Self::shutoff(),
            "ERROR" => Self::error(),
            "BUILD" | "REBUILD" | "RESIZE" | "REBOOT" => Self::building(),
            "VERIFY_RESIZE" => Self::verify(),
            "MIGRATING" => Self::migrating(),
            _ => "?",
        }
    }
}

pub fn panel_title(name: &str, focused: bool) -> String {
    if focused {
        format!("[ {name} ]")
    } else {
        format!("  {name}  ")
    }
}

pub fn panel_title_line(name: &str, focused: bool, all_tenants: bool) -> Line<'static> {
    if !focused {
        return Line::from(format!("  {name}  "));
    }
    if all_tenants {
        Line::from(vec![
            Span::raw(format!("[ {name} | ")),
            Span::styled("ALL", Theme::active()),
            Span::raw(" ]"),
        ])
    } else {
        Line::from(format!("[ {name} ]"))
    }
}

pub fn key_hint<'a>(key: &'a str, desc: &'a str) -> Vec<Span<'a>> {
    vec![
        Span::styled(key, Theme::focus_border().add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(desc, Theme::disabled()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Step 1: Theme 시맨틱 스타일 ===

    /// Reset theme to Dark with colors for test isolation.
    fn reset_dark() {
        Theme::init_with_no_color(crate::config::ThemeVariant::Dark, false);
    }

    #[test]
    fn test_theme_active_is_yellow_bold() {
        reset_dark();
        let style = Theme::active();
        assert_eq!(style.fg, Some(Color::Yellow));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_theme_done_is_green() {
        reset_dark();
        let style = Theme::done();
        assert_eq!(style.fg, Some(Color::LightGreen));
    }

    #[test]
    fn test_theme_error_is_red() {
        reset_dark();
        let style = Theme::error();
        assert_eq!(style.fg, Some(Color::LightRed));
    }

    #[test]
    fn test_theme_highlight_is_bold_only() {
        reset_dark();
        let style = Theme::highlight();
        assert_eq!(style.fg, None);
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_theme_active_vs_warning() {
        reset_dark();
        let active = Theme::active();
        let warning = Theme::warning();
        assert!(active.add_modifier.contains(Modifier::BOLD));
        assert!(!warning.add_modifier.contains(Modifier::BOLD));
    }

    // === Step 2: Icons ===

    #[test]
    fn test_icons_active() {
        assert_eq!(Icons::active(), "●");
    }

    #[test]
    fn test_icons_status_icon_mapping() {
        assert_eq!(Icons::status_icon("ACTIVE"), "●");
        assert_eq!(Icons::status_icon("ERROR"), "✗");
        assert_eq!(Icons::status_icon("UNKNOWN"), "?");
    }

    #[test]
    fn test_icons_status_icon_all_states() {
        assert_eq!(Icons::status_icon("SHUTOFF"), "○");
        assert_eq!(Icons::status_icon("STOPPED"), "○");
        assert_eq!(Icons::status_icon("BUILD"), "⟳");
        assert_eq!(Icons::status_icon("REBUILD"), "⟳");
        assert_eq!(Icons::status_icon("RESIZE"), "⟳");
        assert_eq!(Icons::status_icon("REBOOT"), "⟳");
        assert_eq!(Icons::status_icon("VERIFY_RESIZE"), "◐");
        assert_eq!(Icons::status_icon("MIGRATING"), "↔");
    }

    // === Step 3: panel_title ===

    #[test]
    fn test_panel_title_focused() {
        assert_eq!(panel_title("Servers", true), "[ Servers ]");
    }

    #[test]
    fn test_panel_title_unfocused() {
        assert_eq!(panel_title("Servers", false), "  Servers  ");
    }

    #[test]
    fn test_panel_title_line_focused_no_all() {
        let line = panel_title_line("Servers", true, false);
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].content.as_ref(), "[ Servers ]");
    }

    #[test]
    fn test_panel_title_line_focused_all_tenants() {
        reset_dark();
        let line = panel_title_line("Servers", true, true);
        assert_eq!(line.spans.len(), 3);
        assert_eq!(line.spans[0].content.as_ref(), "[ Servers | ");
        assert_eq!(line.spans[1].content.as_ref(), "ALL");
        assert_eq!(line.spans[1].style.fg, Some(Color::Yellow));
        assert!(line.spans[1].style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(line.spans[2].content.as_ref(), " ]");
    }

    #[test]
    fn test_panel_title_line_unfocused_ignores_all() {
        let line = panel_title_line("Servers", false, true);
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].content.as_ref(), "  Servers  ");
    }

    // === Step 4: key_hint ===

    #[test]
    fn test_key_hint_produces_three_spans() {
        let spans = key_hint("Tab", "패널");
        assert_eq!(spans.len(), 3);
    }

    #[test]
    fn test_key_hint_key_uses_focus_border_bold() {
        reset_dark();
        let spans = key_hint("Tab", "패널");
        let key_span = &spans[0];
        let expected = Theme::focus_border().add_modifier(Modifier::BOLD);
        assert_eq!(key_span.style.fg, expected.fg);
        assert!(key_span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_key_hint_has_separator_space() {
        let spans = key_hint("Tab", "패널");
        assert_eq!(spans[1].content.as_ref(), " ");
    }

    // === Theme variant + NO_COLOR ===

    #[test]
    fn test_theme_init_dark_active_is_yellow() {
        Theme::init(crate::config::ThemeVariant::Dark);
        let style = Theme::active();
        assert_eq!(style.fg, Some(Color::Yellow));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_theme_init_light_done_differs_from_dark() {
        Theme::init(crate::config::ThemeVariant::Light);
        let light_done = Theme::done();
        // Light done should use a darker green for bright backgrounds
        assert_ne!(light_done.fg, Some(Color::LightGreen));
    }

    #[test]
    fn test_no_color_strips_fg_bg() {
        Theme::init_with_no_color(crate::config::ThemeVariant::Dark, true);
        let style = Theme::active();
        assert_eq!(style.fg, None);
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_no_color_overrides_variant() {
        Theme::init_with_no_color(crate::config::ThemeVariant::Light, true);
        let style = Theme::done();
        assert_eq!(style.fg, None);
    }
}
