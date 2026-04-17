//! Status-bar widget showing the active cloud/project context.
//!
//! After a successful context switch the caller invokes
//! [`ContextIndicator::set_context`] with `mark_highlight = true`. The widget
//! then renders the new context with a highlighted style for
//! `highlight_duration` so the change is visible to the operator.
//!
//! The widget itself is a passive timer — `is_highlighting()` is recomputed on
//! every render via `Instant::elapsed()`. The host (StatusBar / App) is
//! responsible for triggering redraws within the highlight window
//! (e.g. via the existing tick loop) so the highlight transitions back to the
//! plain style.

use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::context::ContextSnapshot;
use crate::ui::theme::Theme;

pub struct ContextIndicator {
    snapshot: Option<ContextSnapshot>,
    last_switch_at: Option<Instant>,
    highlight_duration: Duration,
}

impl ContextIndicator {
    pub fn new(highlight_duration: Duration) -> Self {
        Self {
            snapshot: None,
            last_switch_at: None,
            highlight_duration,
        }
    }

    /// Update the displayed context. When `mark_highlight` is true the widget
    /// briefly highlights to draw attention to the change.
    pub fn set_context(&mut self, snapshot: &ContextSnapshot, mark_highlight: bool) {
        self.snapshot = Some(snapshot.clone());
        self.last_switch_at = if mark_highlight {
            Some(Instant::now())
        } else {
            None
        };
    }

    pub fn snapshot(&self) -> Option<&ContextSnapshot> {
        self.snapshot.as_ref()
    }

    pub fn is_highlighting(&self) -> bool {
        self.last_switch_at
            .is_some_and(|t| t.elapsed() < self.highlight_duration)
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let text = match &self.snapshot {
            Some(s) => format!(" {} • {} ", s.target.cloud, s.target.project_name),
            None => " (no context) ".to_string(),
        };
        let style = if self.is_highlighting() {
            Theme::warning()
        } else {
            Theme::disabled()
        };
        let line = Line::from(Span::styled(text, style));
        frame.render_widget(Paragraph::new(line), area);
    }

    /// Test-only: simulate elapsed time without sleeping.
    #[cfg(test)]
    pub(crate) fn set_last_switch_at_for_test(&mut self, instant: Instant) {
        self.last_switch_at = Some(instant);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ContextSnapshot;
    use crate::context::types::ContextTarget;
    use crate::port::types::{ProjectScope, Token, TokenScope};
    use chrono::{TimeZone, Utc};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn make_snapshot(cloud: &str, project: &str) -> ContextSnapshot {
        let target = ContextTarget {
            cloud: cloud.into(),
            project_id: format!("{project}-id"),
            project_name: project.into(),
            domain: "default".into(),
        };
        ContextSnapshot {
            target: target.clone(),
            epoch: 1,
            token: Token {
                id: format!("tok-{project}"),
                expires_at: Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap(),
                project: ProjectScope {
                    id: target.project_id.clone(),
                    name: target.project_name.clone(),
                    domain_id: "default".into(),
                    domain_name: target.domain.clone(),
                },
                roles: Vec::new(),
                catalog: Vec::new(),
            },
            token_scope: TokenScope::from(&target),
            captured_at: Utc.with_ymd_and_hms(2026, 4, 17, 0, 0, 0).unwrap(),
        }
    }

    #[test]
    fn test_initial_state_no_snapshot() {
        let indicator = ContextIndicator::new(Duration::from_secs(2));
        assert!(indicator.snapshot().is_none());
        assert!(!indicator.is_highlighting());
    }

    #[test]
    fn test_set_context_with_highlight_activates() {
        let mut indicator = ContextIndicator::new(Duration::from_secs(2));
        indicator.set_context(&make_snapshot("devstack", "admin"), true);
        assert!(indicator.snapshot().is_some());
        assert!(indicator.is_highlighting());
    }

    #[test]
    fn test_set_context_without_highlight_does_not_activate() {
        let mut indicator = ContextIndicator::new(Duration::from_secs(2));
        indicator.set_context(&make_snapshot("devstack", "admin"), false);
        assert!(indicator.snapshot().is_some());
        assert!(!indicator.is_highlighting());
    }

    #[test]
    fn test_highlight_expires_after_duration() {
        let mut indicator = ContextIndicator::new(Duration::from_millis(50));
        indicator.set_context(&make_snapshot("devstack", "admin"), true);
        // Simulate elapsed time without sleeping: rewind last_switch_at.
        indicator.set_last_switch_at_for_test(Instant::now() - Duration::from_millis(200));
        assert!(!indicator.is_highlighting());
    }

    #[test]
    fn test_set_context_replaces_previous_snapshot() {
        let mut indicator = ContextIndicator::new(Duration::from_secs(2));
        indicator.set_context(&make_snapshot("dev", "admin"), true);
        indicator.set_context(&make_snapshot("prod", "ops"), false);
        let snap = indicator.snapshot().expect("snapshot should be present");
        assert_eq!(snap.target.cloud, "prod");
        assert_eq!(snap.target.project_name, "ops");
        assert!(!indicator.is_highlighting());
    }

    #[test]
    fn test_render_with_snapshot_shows_cloud_and_project() {
        let mut indicator = ContextIndicator::new(Duration::from_secs(2));
        indicator.set_context(&make_snapshot("devstack", "admin"), true);
        let backend = TestBackend::new(40, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| indicator.render(f, f.area())).unwrap();
        let buffer = terminal.backend().buffer();
        let line: String = (0..40)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(line.contains("devstack"), "rendered line: {line:?}");
        assert!(line.contains("admin"), "rendered line: {line:?}");
    }

    #[test]
    fn test_render_without_snapshot_shows_placeholder() {
        let indicator = ContextIndicator::new(Duration::from_secs(2));
        let backend = TestBackend::new(20, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| indicator.render(f, f.area())).unwrap();
        let buffer = terminal.backend().buffer();
        let line: String = (0..20)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(line.contains("no context"), "rendered line: {line:?}");
    }
}
