use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

pub struct Theme;

impl Theme {
    pub fn active() -> Style {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    }

    pub fn done() -> Style {
        Style::default().fg(Color::Green)
    }

    pub fn error() -> Style {
        Style::default().fg(Color::Red)
    }

    pub fn waiting() -> Style {
        Style::default().fg(Color::DarkGray)
    }

    pub fn warning() -> Style {
        Style::default().fg(Color::Yellow)
    }

    pub fn focus_border() -> Style {
        Style::default().fg(Color::Cyan)
    }

    pub fn unfocus_border() -> Style {
        Style::default().fg(Color::DarkGray)
    }

    pub fn highlight() -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }

    pub fn disabled() -> Style {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM)
    }

    pub fn link() -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::UNDERLINED)
    }

    pub fn timestamp() -> Style {
        Style::default().fg(Color::Cyan)
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
            Span::styled("ALL", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" ]"),
        ])
    } else {
        Line::from(format!("[ {name} ]"))
    }
}

pub fn key_hint<'a>(key: &'a str, desc: &'a str) -> Vec<Span<'a>> {
    vec![
        Span::styled(
            key,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(desc, Style::default().add_modifier(Modifier::DIM)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Step 1: Theme 시맨틱 스타일 ===

    #[test]
    fn test_theme_active_is_yellow_bold() {
        let style = Theme::active();
        assert_eq!(style.fg, Some(Color::Yellow));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_theme_done_is_green() {
        let style = Theme::done();
        assert_eq!(style.fg, Some(Color::Green));
    }

    #[test]
    fn test_theme_error_is_red() {
        let style = Theme::error();
        assert_eq!(style.fg, Some(Color::Red));
    }

    #[test]
    fn test_theme_highlight_is_bold_only() {
        let style = Theme::highlight();
        assert_eq!(style.fg, None);
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_theme_active_vs_warning() {
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
    fn test_key_hint_key_is_cyan_bold() {
        let spans = key_hint("Tab", "패널");
        let key_span = &spans[0];
        assert_eq!(key_span.style.fg, Some(Color::Cyan));
        assert!(key_span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_key_hint_has_separator_space() {
        let spans = key_hint("Tab", "패널");
        assert_eq!(spans[1].content.as_ref(), " ");
    }
}
