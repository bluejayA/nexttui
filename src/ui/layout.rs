use ratatui::layout::{Constraint, Direction, Layout, Rect};

const MIN_WIDTH: u16 = 80;
const MIN_HEIGHT: u16 = 24;
const DEFAULT_SIDEBAR_PERCENT: u16 = 15;

pub struct LayoutAreas {
    pub header: Rect,
    pub sidebar: Option<Rect>,
    pub content: Rect,
    pub input_bar: Rect,
    pub toast_bar: Rect,
    pub status_bar: Rect,
}

pub struct LayoutManager {
    sidebar_visible: bool,
    terminal_width: u16,
    terminal_height: u16,
    sidebar_width_percent: u16,
}

impl LayoutManager {
    pub fn new() -> Self {
        Self {
            sidebar_visible: true,
            terminal_width: MIN_WIDTH,
            terminal_height: MIN_HEIGHT,
            sidebar_width_percent: DEFAULT_SIDEBAR_PERCENT,
        }
    }

    /// Calculate layout areas from frame size.
    pub fn calculate(&self, frame_size: Rect) -> LayoutAreas {
        // Vertical: Header(1) | Body(rest) | InputBar(1) | ToastBar(1) | StatusBar(1)
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(frame_size);

        let header = vertical[0];
        let body = vertical[1];
        let input_bar = vertical[2];
        let toast_bar = vertical[3];
        let status_bar = vertical[4];

        let (sidebar, content) = if self.sidebar_visible {
            let horizontal = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(self.sidebar_width_percent),
                    Constraint::Min(0),
                ])
                .split(body);
            (Some(horizontal[0]), horizontal[1])
        } else {
            (None, body)
        };

        LayoutAreas {
            header,
            sidebar,
            content,
            input_bar,
            toast_bar,
            status_bar,
        }
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
    }

    pub fn set_sidebar_visible(&mut self, visible: bool) {
        self.sidebar_visible = visible;
    }

    pub fn is_sidebar_visible(&self) -> bool {
        self.sidebar_visible
    }

    pub fn on_resize(&mut self, width: u16, height: u16) {
        self.terminal_width = width;
        self.terminal_height = height;
    }

    pub fn is_minimum_size(&self) -> bool {
        self.terminal_width >= MIN_WIDTH && self.terminal_height >= MIN_HEIGHT
    }
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: u16, h: u16) -> Rect {
        Rect::new(0, 0, w, h)
    }

    #[test]
    fn test_calculate_with_sidebar() {
        let lm = LayoutManager::new();
        let areas = lm.calculate(rect(100, 30));
        assert!(areas.sidebar.is_some());
        assert_eq!(areas.header.height, 1);
        assert_eq!(areas.input_bar.height, 1);
        assert_eq!(areas.toast_bar.height, 1);
        assert_eq!(areas.status_bar.height, 1);
        // Body = 30 - 4 = 26 (header + input + toast + status)
        assert_eq!(areas.content.height, 26);
    }

    #[test]
    fn test_layout_has_toast_bar() {
        let lm = LayoutManager::new();
        let areas = lm.calculate(rect(80, 24));
        assert_eq!(areas.toast_bar.height, 1);
        assert_eq!(areas.toast_bar.width, 80);
    }

    #[test]
    fn test_layout_no_overlap_80x24() {
        let lm = LayoutManager::new();
        let areas = lm.calculate(rect(80, 24));
        // All areas should have distinct y positions (no overlap)
        let mut ys: Vec<(u16, u16)> = vec![
            (areas.header.y, areas.header.height),
            (areas.content.y, areas.content.height),
            (areas.input_bar.y, areas.input_bar.height),
            (areas.toast_bar.y, areas.toast_bar.height),
            (areas.status_bar.y, areas.status_bar.height),
        ];
        ys.sort_by_key(|&(y, _)| y);
        for w in ys.windows(2) {
            assert!(
                w[0].0 + w[0].1 <= w[1].0,
                "areas overlap: {:?} vs {:?}",
                w[0],
                w[1]
            );
        }
        // Total should fill 24 rows
        let total: u16 = areas.header.height
            + areas.content.height
            + areas.input_bar.height
            + areas.toast_bar.height
            + areas.status_bar.height;
        assert_eq!(total, 24);
    }

    #[test]
    fn test_layout_body_height_is_frame_minus_4() {
        let lm = LayoutManager::new();
        let areas = lm.calculate(rect(80, 24));
        // body = 24 - 4 (header + input + toast + status) = 20
        assert_eq!(areas.content.height, 20);
    }

    #[test]
    fn test_calculate_without_sidebar() {
        let mut lm = LayoutManager::new();
        lm.toggle_sidebar();
        let areas = lm.calculate(rect(100, 30));
        assert!(areas.sidebar.is_none());
        assert_eq!(areas.content.width, 100);
    }

    #[test]
    fn test_toggle_sidebar() {
        let mut lm = LayoutManager::new();
        assert!(lm.is_sidebar_visible());
        lm.toggle_sidebar();
        assert!(!lm.is_sidebar_visible());
        lm.toggle_sidebar();
        assert!(lm.is_sidebar_visible());
    }

    #[test]
    fn test_on_resize() {
        let mut lm = LayoutManager::new();
        lm.on_resize(120, 40);
        assert!(lm.is_minimum_size());
    }

    #[test]
    fn test_minimum_size_fail() {
        let mut lm = LayoutManager::new();
        lm.on_resize(60, 20);
        assert!(!lm.is_minimum_size());
    }

    #[test]
    fn test_sidebar_width_is_percentage() {
        let lm = LayoutManager::new();
        let areas = lm.calculate(rect(100, 30));
        let sidebar = areas.sidebar.unwrap();
        // 15% of 100 = 15
        assert_eq!(sidebar.width, 15);
    }
}
