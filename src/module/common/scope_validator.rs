//! BL-P2-085 Step 15 — Form selection scope validator (FR4).
//!
//! Pure-function helper used by domain modules to reject form submits
//! whose selected resource(s) live in a project other than the current
//! active scope. Returns the first mismatch — callers translate it to a
//! `CrossProjectBlockEvent` + toast.
//!
//! Distinct from FR2 (worker origin guard) and FR1 (adapter refilter):
//! FR4 fires *before* dispatch, at the form-submit boundary, so the user
//! sees a clear "you selected a resource from another project" message
//! instead of a silent block.

use crate::infra::cross_project_guard::CrossProjectReason;

/// One row of form-selected scope-bearing input. `project_id == None`
/// means the selection has no scope label (e.g. UI didn't load it yet,
/// or the upstream omitted it); under FR4 such inputs are dropped to
/// the caller's fail-safe path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormSelection<'a> {
    /// Logical field name as shown to the user (`"image"`, `"network"`,
    /// `"flavor"`, etc.). Carried into the resulting error so the toast
    /// can highlight which row mismatched.
    pub field: &'a str,
    /// Project id the selected resource belongs to. `None` = fail-safe
    /// deny (treated as mismatch unless caller normalizes earlier).
    pub project_id: Option<&'a str>,
}

/// Error raised when a form selection's project_id disagrees with the
/// active scope. Carries the first offending field plus the canonical
/// `CrossProjectReason::FormSelectionMismatch` so the audit emit path
/// stays unified with FR1/FR2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormValidationError {
    pub field: String,
    pub reason: CrossProjectReason,
}

/// Validate `selections` against `active`. Returns `Err` on the first
/// row whose `project_id != Some(active)`; `None` is treated as
/// mismatch (`selected = ""`) so missing-scope inputs fail safely.
///
/// `active` is expected non-empty — an empty `active` would degrade to
/// `UnscopedFailSafe` semantics, which the worker-side guard already
/// covers. Callers should short-circuit to the unscoped path before
/// invoking this validator.
pub fn validate_form_scope(
    active: &str,
    selections: &[FormSelection<'_>],
) -> Result<(), FormValidationError> {
    for sel in selections {
        let matches = sel.project_id == Some(active);
        if !matches {
            let selected = sel.project_id.unwrap_or("").to_string();
            return Err(FormValidationError {
                field: sel.field.to_string(),
                reason: CrossProjectReason::FormSelectionMismatch {
                    selected,
                    active: active.to_string(),
                },
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_single_selection_match_passes() {
        let selections = [FormSelection {
            field: "image",
            project_id: Some("A"),
        }];
        let result = validate_form_scope("A", &selections);
        assert!(result.is_ok(), "matching project_id must pass");
    }

    #[test]
    fn test_validate_single_selection_mismatch_returns_error() {
        let selections = [FormSelection {
            field: "image",
            project_id: Some("B"),
        }];
        let err = validate_form_scope("A", &selections).expect_err("mismatch must error");
        assert_eq!(err.field, "image");
        match err.reason {
            CrossProjectReason::FormSelectionMismatch { selected, active } => {
                assert_eq!(selected, "B");
                assert_eq!(active, "A");
            }
            other => panic!("expected FormSelectionMismatch, got {other:?}"),
        }
    }

    #[test]
    fn test_validate_multi_selection_first_mismatch_wins() {
        let selections = [
            FormSelection {
                field: "image",
                project_id: Some("A"),
            },
            FormSelection {
                field: "network",
                project_id: Some("B"),
            },
            FormSelection {
                field: "security_group",
                project_id: Some("C"),
            },
        ];
        let err = validate_form_scope("A", &selections).expect_err("must error on first mismatch");
        assert_eq!(
            err.field, "network",
            "first mismatched field must be reported, subsequent mismatches ignored"
        );
        match err.reason {
            CrossProjectReason::FormSelectionMismatch { selected, .. } => {
                assert_eq!(selected, "B");
            }
            other => panic!("expected FormSelectionMismatch, got {other:?}"),
        }
    }

    #[test]
    fn test_validation_error_carries_field_name_and_reason() {
        let selections = [FormSelection {
            field: "flavor",
            project_id: None,
        }];
        let err = validate_form_scope("A", &selections).expect_err("None must fail-safe");
        assert_eq!(err.field, "flavor");
        match err.reason {
            CrossProjectReason::FormSelectionMismatch { selected, active } => {
                assert_eq!(selected, "", "None project_id encodes as empty selected");
                assert_eq!(active, "A");
            }
            other => panic!("expected FormSelectionMismatch, got {other:?}"),
        }
    }
}
