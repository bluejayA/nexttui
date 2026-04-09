use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

pub struct GaugeBar {
    label: String,
    used: u64,
    total: u64,
    unit: String,
    bar_width: u16,
}

impl GaugeBar {
    pub fn new(label: &str, used: u64, total: u64) -> Self {
        Self {
            label: label.to_string(),
            used,
            total,
            unit: String::new(),
            bar_width: 20,
        }
    }

    pub fn with_unit(mut self, unit: &str) -> Self {
        self.unit = unit.to_string();
        self
    }

    pub fn with_bar_width(mut self, width: u16) -> Self {
        self.bar_width = width;
        self
    }

    pub fn ratio(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        let r = self.used as f64 / self.total as f64;
        r.clamp(0.0, 1.0)
    }

    pub fn color(&self) -> Color {
        let pct = (self.ratio() * 100.0) as u16;
        match pct {
            0..=70 => Color::Green,
            71..=90 => Color::Yellow,
            _ => Color::Red,
        }
    }

    pub fn render_line(&self) -> Line<'static> {
        let pct = (self.ratio() * 100.0) as u16;
        let filled = ((self.bar_width as f64) * self.ratio()) as u16;
        let empty = self.bar_width.saturating_sub(filled);

        let bar_color = self.color();
        let filled_str: String = "█".repeat(filled as usize);
        let empty_str: String = "░".repeat(empty as usize);

        let unit_sep = if self.unit.is_empty() { "" } else { " " };
        let info = format!(
            " {}/{}{unit_sep}{} ({pct}%)",
            self.used, self.total, self.unit,
        );

        let label_width = 12;
        let padded_label = format!("{:<width$}", self.label, width = label_width);

        Line::from(vec![
            Span::styled(padded_label, Style::default().fg(Color::White)),
            Span::raw("["),
            Span::styled(filled_str, Style::default().fg(bar_color)),
            Span::styled(empty_str, Style::default().fg(Color::DarkGray)),
            Span::raw("]"),
            Span::styled(info, Style::default().fg(Color::White)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === RED → GREEN: GaugeBar::new basic construction ===

    #[test]
    fn test_gauge_bar_new_defaults() {
        let g = GaugeBar::new("CPU", 10, 100);
        assert_eq!(g.label, "CPU");
        assert_eq!(g.used, 10);
        assert_eq!(g.total, 100);
        assert_eq!(g.unit, "");
        assert_eq!(g.bar_width, 20);
    }

    #[test]
    fn test_gauge_bar_with_unit() {
        let g = GaugeBar::new("RAM", 4096, 8192).with_unit("MB");
        assert_eq!(g.unit, "MB");
    }

    #[test]
    fn test_gauge_bar_with_bar_width() {
        let g = GaugeBar::new("Disk", 50, 100).with_bar_width(30);
        assert_eq!(g.bar_width, 30);
    }

    // === RED → GREEN: ratio calculation ===

    #[test]
    fn test_ratio_normal() {
        let g = GaugeBar::new("CPU", 50, 100);
        assert!((g.ratio() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ratio_zero_total() {
        let g = GaugeBar::new("CPU", 10, 0);
        assert!((g.ratio() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ratio_over_capacity() {
        let g = GaugeBar::new("CPU", 150, 100);
        assert!((g.ratio() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ratio_full() {
        let g = GaugeBar::new("CPU", 100, 100);
        assert!((g.ratio() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ratio_empty() {
        let g = GaugeBar::new("CPU", 0, 100);
        assert!((g.ratio() - 0.0).abs() < f64::EPSILON);
    }

    // === RED → GREEN: color thresholds ===

    #[test]
    fn test_color_green_at_0_pct() {
        let g = GaugeBar::new("CPU", 0, 100);
        assert_eq!(g.color(), Color::Green);
    }

    #[test]
    fn test_color_green_at_70_pct() {
        let g = GaugeBar::new("CPU", 70, 100);
        assert_eq!(g.color(), Color::Green);
    }

    #[test]
    fn test_color_yellow_at_71_pct() {
        let g = GaugeBar::new("CPU", 71, 100);
        assert_eq!(g.color(), Color::Yellow);
    }

    #[test]
    fn test_color_yellow_at_90_pct() {
        let g = GaugeBar::new("CPU", 90, 100);
        assert_eq!(g.color(), Color::Yellow);
    }

    #[test]
    fn test_color_red_at_91_pct() {
        let g = GaugeBar::new("CPU", 91, 100);
        assert_eq!(g.color(), Color::Red);
    }

    #[test]
    fn test_color_red_at_100_pct() {
        let g = GaugeBar::new("CPU", 100, 100);
        assert_eq!(g.color(), Color::Red);
    }

    #[test]
    fn test_color_green_zero_total() {
        let g = GaugeBar::new("CPU", 0, 0);
        assert_eq!(g.color(), Color::Green);
    }

    // === RED → GREEN: render_line structure ===

    #[test]
    fn test_render_line_has_six_spans() {
        let g = GaugeBar::new("CPU", 50, 100);
        let line = g.render_line();
        assert_eq!(line.spans.len(), 6);
    }

    #[test]
    fn test_render_line_label_padded() {
        let g = GaugeBar::new("vCPU", 10, 64);
        let line = g.render_line();
        // First span is padded label (12 chars wide)
        assert_eq!(line.spans[0].content.len(), 12);
        assert!(line.spans[0].content.starts_with("vCPU"));
    }

    #[test]
    fn test_render_line_brackets() {
        let g = GaugeBar::new("CPU", 10, 100);
        let line = g.render_line();
        assert_eq!(line.spans[1].content.as_ref(), "[");
        assert_eq!(line.spans[4].content.as_ref(), "]");
    }

    #[test]
    fn test_render_line_bar_width_20() {
        let g = GaugeBar::new("CPU", 50, 100);
        let line = g.render_line();
        let filled_len = line.spans[2].content.chars().count();
        let empty_len = line.spans[3].content.chars().count();
        assert_eq!(filled_len + empty_len, 20);
    }

    #[test]
    fn test_render_line_info_with_unit() {
        let g = GaugeBar::new("RAM", 4096, 8192).with_unit("MB");
        let line = g.render_line();
        let info = line.spans[5].content.to_string();
        assert!(info.contains("4096/8192"), "info: {info}");
        assert!(info.contains("MB"), "info: {info}");
        assert!(info.contains("50%"), "info: {info}");
    }

    #[test]
    fn test_render_line_info_without_unit() {
        let g = GaugeBar::new("VMs", 5, 20);
        let line = g.render_line();
        let info = line.spans[5].content.to_string();
        assert!(info.contains("5/20"), "info: {info}");
        assert!(info.contains("25%"), "info: {info}");
    }

    #[test]
    fn test_render_line_filled_color_matches_color_method() {
        let g = GaugeBar::new("CPU", 85, 100);
        let line = g.render_line();
        let expected_color = g.color();
        assert_eq!(line.spans[2].style.fg, Some(expected_color));
    }

    #[test]
    fn test_render_line_custom_bar_width() {
        let g = GaugeBar::new("Disk", 30, 100).with_bar_width(10);
        let line = g.render_line();
        let filled_len = line.spans[2].content.chars().count();
        let empty_len = line.spans[3].content.chars().count();
        assert_eq!(filled_len + empty_len, 10);
    }

    // === Builder chaining ===

    #[test]
    fn test_builder_chaining() {
        let g = GaugeBar::new("Disk", 50, 200)
            .with_unit("GB")
            .with_bar_width(15);
        assert_eq!(g.unit, "GB");
        assert_eq!(g.bar_width, 15);
        assert!((g.ratio() - 0.25).abs() < f64::EPSILON);
    }
}
