//! Holds the currently-active cloud/project identity plus a "just switched"
//! highlight timer. Rendering lives in the host widget (StatusBar) so this
//! type stays small and StatusBar can position/style it alongside the rest of
//! the bar instead of claiming its own Rect.
//!
//! Usage: the host calls [`set_target`] with `mark_highlight = true` after a
//! successful switch. Subsequent renders query [`target`] and
//! [`is_highlighting`] to pick the display style. The highlight transitions
//! back to the plain style on the first redraw after `highlight_duration`,
//! so the host must already be redrawing within that window (tick loop).
//!
//! Takes `&ContextTarget` rather than `&ContextSnapshot`: the indicator only
//! needs `cloud` + `project_name` for display, so accepting the lighter type
//! avoids a Token clone on every switch and matches `AppEvent::ContextChanged
//! { target }` directly.

use std::time::{Duration, Instant};

use crate::context::types::ContextTarget;

/// Active cloud/project identity plus a transient "just switched" highlight
/// timer. Rendering is performed by the host widget (StatusBar) so this type
/// stays a pure state holder.
pub struct ContextIndicator {
    target: Option<ContextTarget>,
    last_switch_at: Option<Instant>,
    highlight_duration: Duration,
}

impl ContextIndicator {
    /// Build a new indicator with no target set yet and the given highlight
    /// window applied to future `set_target(.., true)` calls.
    pub fn new(highlight_duration: Duration) -> Self {
        Self {
            target: None,
            last_switch_at: None,
            highlight_duration,
        }
    }

    /// Replace the displayed target. When `mark_highlight` is true the
    /// indicator briefly highlights so the transition is visible.
    pub fn set_target(&mut self, target: &ContextTarget, mark_highlight: bool) {
        self.target = Some(target.clone());
        self.last_switch_at = if mark_highlight {
            Some(Instant::now())
        } else {
            None
        };
    }

    /// Current target, or `None` if no context has been set yet.
    pub fn target(&self) -> Option<&ContextTarget> {
        self.target.as_ref()
    }

    /// True while the highlight timer has not yet elapsed. Evaluated lazily
    /// against `Instant::now()`, so the host must trigger a redraw within the
    /// window for the style transition to be observed.
    pub fn is_highlighting(&self) -> bool {
        self.last_switch_at
            .is_some_and(|t| t.elapsed() < self.highlight_duration)
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

    fn make_target(cloud: &str, project: &str) -> ContextTarget {
        ContextTarget {
            cloud: cloud.into(),
            project_id: format!("{project}-id"),
            project_name: project.into(),
            domain: "default".into(),
        }
    }

    #[test]
    fn test_initial_state_no_target() {
        let indicator = ContextIndicator::new(Duration::from_secs(2));
        assert!(indicator.target().is_none());
        assert!(!indicator.is_highlighting());
    }

    #[test]
    fn test_set_target_with_highlight_activates() {
        let mut indicator = ContextIndicator::new(Duration::from_secs(2));
        indicator.set_target(&make_target("devstack", "admin"), true);
        assert!(indicator.target().is_some());
        assert!(indicator.is_highlighting());
    }

    #[test]
    fn test_set_target_without_highlight_does_not_activate() {
        let mut indicator = ContextIndicator::new(Duration::from_secs(2));
        indicator.set_target(&make_target("devstack", "admin"), false);
        assert!(indicator.target().is_some());
        assert!(!indicator.is_highlighting());
    }

    #[test]
    fn test_highlight_expires_after_duration() {
        let mut indicator = ContextIndicator::new(Duration::from_millis(50));
        indicator.set_target(&make_target("devstack", "admin"), true);
        // Simulate elapsed time without sleeping: rewind last_switch_at.
        indicator.set_last_switch_at_for_test(Instant::now() - Duration::from_millis(200));
        assert!(!indicator.is_highlighting());
    }

    #[test]
    fn test_set_target_replaces_previous() {
        let mut indicator = ContextIndicator::new(Duration::from_secs(2));
        indicator.set_target(&make_target("dev", "admin"), true);
        indicator.set_target(&make_target("prod", "ops"), false);
        let t = indicator.target().expect("target should be present");
        assert_eq!(t.cloud, "prod");
        assert_eq!(t.project_name, "ops");
        assert!(!indicator.is_highlighting());
    }
}
