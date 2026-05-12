//! Cross-project scoping guard — pure-function decision module.
//!
//! Used by FR2 (worker mutation guard), FR3 (RBAC project-scope check),
//! and FR4 (form selection validator). Returns structured `GuardDecision`s
//! that callers translate into [`AppError::CrossProjectBlocked`] and
//! [`CrossProjectBlockEvent`].

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardDecision {
    Allow,
    Block { reason: CrossProjectReason },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrossProjectReason {
    /// FR2: action stamp 시점의 origin scope ≠ 현재 active scope
    OriginScopeMismatch { origin: String, active: String },
    /// FR4: form-selected resource's project_id ≠ active scope
    FormSelectionMismatch { selected: String, active: String },
    /// FR1: adapter response에 cross-project resource 잠입 (불변 위반)
    AdapterFilterViolation {
        resource_id: String,
        project_id: String,
    },
    /// scope가 unscoped/None이라 비교 불가능. fail-safe deny.
    UnscopedFailSafe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardLayer {
    Fr1Adapter,
    Fr2Worker,
    Fr3Rbac,
    Fr4Form,
}

impl GuardLayer {
    /// Stable string for audit `details.guard_layer` field.
    pub fn as_str(self) -> &'static str {
        match self {
            GuardLayer::Fr1Adapter => "fr1_adapter",
            GuardLayer::Fr2Worker => "fr2_worker",
            GuardLayer::Fr3Rbac => "fr3_rbac",
            GuardLayer::Fr4Form => "fr4_form",
        }
    }
}

impl CrossProjectReason {
    /// Stable string for audit `details.reason` field. Schema-stable.
    pub fn as_str(&self) -> &'static str {
        match self {
            CrossProjectReason::OriginScopeMismatch { .. } => "origin_scope_mismatch",
            CrossProjectReason::FormSelectionMismatch { .. } => "form_selection_mismatch",
            CrossProjectReason::AdapterFilterViolation { .. } => "adapter_filter_violation",
            CrossProjectReason::UnscopedFailSafe => "unscoped_fail_safe",
        }
    }
}

/// FR2 worker hook가 호출. action 발행 시점의 origin과 현재 active scope 비교.
pub fn check_origin_scope(origin: &str, active: &str) -> GuardDecision {
    if origin.is_empty() || active.is_empty() {
        return GuardDecision::Block {
            reason: CrossProjectReason::UnscopedFailSafe,
        };
    }
    if origin == active {
        GuardDecision::Allow
    } else {
        GuardDecision::Block {
            reason: CrossProjectReason::OriginScopeMismatch {
                origin: origin.to_string(),
                active: active.to_string(),
            },
        }
    }
}

/// FR4 form validator가 호출. selected resource의 project_id와 active scope 비교.
pub fn check_form_selection(selected_project_id: &str, active: &str) -> GuardDecision {
    if active.is_empty() {
        return GuardDecision::Block {
            reason: CrossProjectReason::UnscopedFailSafe,
        };
    }
    if selected_project_id.is_empty() {
        // owner 정보가 없는 리소스 — fail-safe deny (Glance Image.owner=None 등)
        return GuardDecision::Block {
            reason: CrossProjectReason::UnscopedFailSafe,
        };
    }
    if selected_project_id == active {
        GuardDecision::Allow
    } else {
        GuardDecision::Block {
            reason: CrossProjectReason::FormSelectionMismatch {
                selected: selected_project_id.to_string(),
                active: active.to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_origin_scope_match_allows() {
        assert_eq!(
            check_origin_scope("admin-uuid", "admin-uuid"),
            GuardDecision::Allow
        );
    }

    #[test]
    fn test_check_origin_scope_mismatch_blocks() {
        let decision = check_origin_scope("admin-uuid", "demo-uuid");
        match decision {
            GuardDecision::Block {
                reason: CrossProjectReason::OriginScopeMismatch { origin, active },
            } => {
                assert_eq!(origin, "admin-uuid");
                assert_eq!(active, "demo-uuid");
            }
            other => panic!("expected origin mismatch, got {other:?}"),
        }
    }

    #[test]
    fn test_check_origin_scope_empty_origin_fail_safe() {
        let decision = check_origin_scope("", "demo");
        assert!(matches!(
            decision,
            GuardDecision::Block {
                reason: CrossProjectReason::UnscopedFailSafe
            }
        ));
    }

    #[test]
    fn test_check_origin_scope_empty_active_fail_safe() {
        let decision = check_origin_scope("admin", "");
        assert!(matches!(
            decision,
            GuardDecision::Block {
                reason: CrossProjectReason::UnscopedFailSafe
            }
        ));
    }

    #[test]
    fn test_check_form_selection_match_allows() {
        assert_eq!(
            check_form_selection("admin-uuid", "admin-uuid"),
            GuardDecision::Allow
        );
    }

    #[test]
    fn test_check_form_selection_mismatch_blocks() {
        let decision = check_form_selection("demo-uuid", "admin-uuid");
        match decision {
            GuardDecision::Block {
                reason: CrossProjectReason::FormSelectionMismatch { selected, active },
            } => {
                assert_eq!(selected, "demo-uuid");
                assert_eq!(active, "admin-uuid");
            }
            other => panic!("expected form selection mismatch, got {other:?}"),
        }
    }

    #[test]
    fn test_check_form_selection_empty_selected_fail_safe() {
        // Glance Image.owner=None 시나리오
        let decision = check_form_selection("", "admin-uuid");
        assert!(matches!(
            decision,
            GuardDecision::Block {
                reason: CrossProjectReason::UnscopedFailSafe
            }
        ));
    }

    #[test]
    fn test_guard_layer_as_str_stable() {
        assert_eq!(GuardLayer::Fr1Adapter.as_str(), "fr1_adapter");
        assert_eq!(GuardLayer::Fr2Worker.as_str(), "fr2_worker");
        assert_eq!(GuardLayer::Fr3Rbac.as_str(), "fr3_rbac");
        assert_eq!(GuardLayer::Fr4Form.as_str(), "fr4_form");
    }

    #[test]
    fn test_reason_as_str_stable() {
        let r = CrossProjectReason::OriginScopeMismatch {
            origin: "a".into(),
            active: "b".into(),
        };
        assert_eq!(r.as_str(), "origin_scope_mismatch");

        let r = CrossProjectReason::FormSelectionMismatch {
            selected: "x".into(),
            active: "y".into(),
        };
        assert_eq!(r.as_str(), "form_selection_mismatch");

        let r = CrossProjectReason::AdapterFilterViolation {
            resource_id: "rid".into(),
            project_id: "pid".into(),
        };
        assert_eq!(r.as_str(), "adapter_filter_violation");

        let r = CrossProjectReason::UnscopedFailSafe;
        assert_eq!(r.as_str(), "unscoped_fail_safe");
    }
}
