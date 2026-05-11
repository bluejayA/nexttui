//! BL-P2-085 Step 17 (Phase 10) — User-facing toast builders for the
//! three cross-project block scenarios.
//!
//! Each builder returns `(message, ToastLevel::Error)` so the caller can
//! hand it straight to `BackgroundTracker::add_toast`. Error level is the
//! deliberate choice — the user just attempted a privileged action and
//! got blocked, so the toast must outlive the longer Error TTL (Phase 7
//! Step 11c parity with `PermissionDenied`).
//!
//! Project / image / field names are truncated to a 60-character display
//! window so a malicious or pathological name can't overrun the
//! one-line toast region. This is a char-count truncate; a future cycle
//! tied to BL-P2-050 can swap in width-aware shortening via
//! `unicode-width` without touching call sites.
//!
//! Naming conventions mirror the audit `CrossProjectReason` shape:
//! - `origin_mismatch_toast` — FR2 worker block (dispatch origin ≠
//!   current active scope).
//! - `glance_owner_mismatch_toast` — FR4 pre-mutation block specifically
//!   for Glance images (owner != active).
//! - `form_mismatch_toast` — FR4 generic form-selection block
//!   (multi-field form, named field mismatch).

use crate::background::ToastLevel;

/// Char-count truncate used by all three builders. Returns the input
/// unchanged when it fits within `max_chars`; otherwise truncates and
/// suffixes with `…` (single char so the visible budget stays
/// predictable). Char-based for now — width-aware variant via
/// `unicode-width` will land with BL-P2-050.
fn safe_display_60(value: &str) -> String {
    const MAX_CHARS: usize = 60;
    let count = value.chars().count();
    if count <= MAX_CHARS {
        return value.to_string();
    }
    // Reserve 1 char for the ellipsis so the total visible width stays
    // at MAX_CHARS exactly.
    let kept: String = value.chars().take(MAX_CHARS - 1).collect();
    format!("{kept}…")
}

/// FR2 worker-layer block toast (dispatch-time origin ≠ active scope).
///
/// `origin` / `target` are project ids (or display names) carried in
/// `CrossProjectReason::OriginScopeMismatch`. Both are truncated.
pub fn origin_mismatch_toast(origin: &str, target: &str) -> (String, ToastLevel) {
    let msg = format!(
        "Cross-project block: action originated in '{}' but active scope is '{}'",
        safe_display_60(origin),
        safe_display_60(target),
    );
    (msg, ToastLevel::Error)
}

/// FR4 pre-mutation block toast for Glance images. The `owner` argument
/// is the image's project id from `Image.owner`; `active` is the user's
/// current scope. Both are truncated.
pub fn glance_owner_mismatch_toast(owner: &str, active: &str) -> (String, ToastLevel) {
    let msg = format!(
        "Image belongs to project '{}' but active scope is '{}' — refusing cross-project mutation",
        safe_display_60(owner),
        safe_display_60(active),
    );
    (msg, ToastLevel::Error)
}

/// FR4 generic form-selection block toast. `field` is the form field
/// label the user selected (e.g. `"network"`, `"flavor"`). All three
/// strings are truncated.
pub fn form_mismatch_toast(
    field: &str,
    selected: &str,
    active: &str,
) -> (String, ToastLevel) {
    let msg = format!(
        "Form field '{}' references project '{}' but active scope is '{}'",
        safe_display_60(field),
        safe_display_60(selected),
        safe_display_60(active),
    );
    (msg, ToastLevel::Error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_origin_mismatch_toast_contains_both_project_names() {
        let (msg, _) = origin_mismatch_toast("proj-A", "proj-B");
        assert!(msg.contains("proj-A"), "origin must be in toast: {msg}");
        assert!(msg.contains("proj-B"), "target must be in toast: {msg}");
    }

    #[test]
    fn test_glance_owner_mismatch_toast_contains_owner_name() {
        let (msg, _) = glance_owner_mismatch_toast("owner-X", "active-Y");
        assert!(msg.contains("owner-X"), "owner must be in toast: {msg}");
        assert!(msg.contains("active-Y"), "active must be in toast: {msg}");
    }

    #[test]
    fn test_form_mismatch_toast_contains_field_label() {
        let (msg, _) = form_mismatch_toast("network", "proj-B", "proj-A");
        assert!(msg.contains("network"), "field label must be in toast: {msg}");
        assert!(msg.contains("proj-B"));
        assert!(msg.contains("proj-A"));
    }

    #[test]
    fn test_toast_level_is_error() {
        // Phase 7 Step 11c parity — cross-project blocks land on the
        // Error TTL so the message survives the standard Success/Info
        // auto-dismiss windows.
        let (_, lvl1) = origin_mismatch_toast("a", "b");
        let (_, lvl2) = glance_owner_mismatch_toast("o", "a");
        let (_, lvl3) = form_mismatch_toast("f", "s", "a");
        assert_eq!(lvl1, ToastLevel::Error);
        assert_eq!(lvl2, ToastLevel::Error);
        assert_eq!(lvl3, ToastLevel::Error);
    }

    #[test]
    fn test_toast_respects_60char_truncate() {
        // 100-char project id should not survive verbatim in the toast.
        let long = "a".repeat(100);
        let (msg, _) = origin_mismatch_toast(&long, "proj-B");
        // Original 100-char run must not appear; the truncated form
        // (59 'a' chars + '…') will.
        assert!(!msg.contains(&"a".repeat(100)));
        let truncated = format!("{}…", "a".repeat(59));
        assert!(
            msg.contains(&truncated),
            "expected 60-char truncate with ellipsis, got: {msg}"
        );
    }
}
