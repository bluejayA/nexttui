//! Cross-project block event + AuditLogger integration.
//!
//! Reuses the production `AuditLogger` (see `src/infra/audit.rs`) via
//! `to_audit_entry()` — this BL avoids introducing a parallel audit subsystem.
//! Specialized fields (`fingerprint`, `guard_layer`, `correlation_id`,
//! `asserted_origin_project_id`, `target_project_id`) are packed into the
//! `details` JSON. The `result` field uses `AuditResult::Failed("cross_project_block:<reason>")`
//! so analysts can grep by the stable reason as_str.
//!
//! Fingerprint v1 canonical format (LOCKED — bump to v2 on any change):
//!   "v1|" + actor_user_id + "|" + active + "|" + origin + "|" + target
//!        + "|" + action_type + "|" + resource_id
//! → sha256 → first 6 bytes → 12 lowercase hex chars.

use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

use crate::infra::audit::{AuditEntry, AuditLogger, AuditResult};
use crate::infra::cross_project_guard::{CrossProjectReason, GuardLayer};

#[derive(Debug, Clone)]
pub struct CrossProjectBlockEvent {
    pub timestamp: DateTime<Utc>,
    pub actor_user_id: String,
    pub actor_cloud: String,
    pub active_project_id: Option<String>,
    pub asserted_origin_project_id: Option<String>,
    pub target_project_id: Option<String>,
    pub action_type: String,
    pub resource_kind: String,
    pub resource_id: Option<String>,
    pub resource_name: Option<String>,
    pub reason: CrossProjectReason,
    pub guard_layer: GuardLayer,
    pub correlation_id: u64,
}

impl CrossProjectBlockEvent {
    /// Convenience constructor (BL-P2-085 Step 11b). Stamps `timestamp = Utc::now()`,
    /// fills required fields from the worker's view of the dispatched action,
    /// and leaves resource-bound optional fields (`target_project_id`,
    /// `resource_id`, `resource_name`) as `None`. Callers with a resource-bound
    /// action can mutate them directly after construction.
    ///
    /// `resource_kind` is a free-form string (e.g. `"server"`, `"volume"`); the
    /// worker enriches it at the call site so this struct stays decoupled from
    /// the `Action` enum.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        reason: CrossProjectReason,
        guard_layer: GuardLayer,
        action_type: impl Into<String>,
        resource_kind: impl Into<String>,
        actor_cloud: impl Into<String>,
        actor_user_id: impl Into<String>,
        active_project_id: Option<String>,
        asserted_origin_project_id: Option<String>,
        correlation_id: u64,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            actor_user_id: actor_user_id.into(),
            actor_cloud: actor_cloud.into(),
            active_project_id,
            asserted_origin_project_id,
            target_project_id: None,
            action_type: action_type.into(),
            resource_kind: resource_kind.into(),
            resource_id: None,
            resource_name: None,
            reason,
            guard_layer,
            correlation_id,
        }
    }

    /// v1 canonical fingerprint. Schema-stable: any change must bump to v2
    /// and migrate downstream audit analysts.
    pub fn fingerprint(&self) -> String {
        let canonical = format!(
            "v1|{user}|{active}|{origin}|{target}|{action}|{resource}",
            user = self.actor_user_id,
            active = self.active_project_id.as_deref().unwrap_or(""),
            origin = self.asserted_origin_project_id.as_deref().unwrap_or(""),
            target = self.target_project_id.as_deref().unwrap_or(""),
            action = self.action_type,
            resource = self.resource_id.as_deref().unwrap_or(""),
        );
        let digest = Sha256::digest(canonical.as_bytes());
        let mut hex = String::with_capacity(12);
        for b in &digest[..6] {
            // write! to a String never errors, but unwrap is denied; fall back to format!
            let _ = write!(&mut hex, "{b:02x}");
        }
        hex
    }

    /// Map this event onto the production `AuditEntry` shape so it can be
    /// written via the existing `AuditLogger` (rotation + sensitive masking).
    pub fn to_audit_entry(&self) -> AuditEntry {
        AuditEntry {
            timestamp: self.timestamp.to_rfc3339(),
            cloud: self.actor_cloud.clone(),
            user: self.actor_user_id.clone(),
            project: self.active_project_id.clone(),
            action: self.action_type.clone(),
            resource_type: self.resource_kind.clone(),
            resource_id: self.resource_id.clone().unwrap_or_default(),
            resource_name: self.resource_name.clone(),
            details: Some(serde_json::json!({
                "fingerprint": self.fingerprint(),
                "guard_layer": self.guard_layer.as_str(),
                "correlation_id": self.correlation_id,
                "asserted_origin_project_id": self.asserted_origin_project_id,
                "target_project_id": self.target_project_id,
            })),
            result: AuditResult::Failed(format!("cross_project_block:{}", self.reason.as_str())),
        }
    }
}

/// Best-effort emit. If `logger` is provided and `log_entry` succeeds, the
/// event is persisted to the audit log; otherwise the event is recorded via
/// `tracing::warn!` so it still surfaces in process logs. Never panics.
///
/// Rotation parity: matches `App::record_audit` (src/app.rs:780) by invoking
/// `rotate_if_needed` after a successful write so cross-project events stay
/// within `MAX_LOG_SIZE`/`MAX_ROTATED_FILES` even under sustained block storms.
pub fn emit(event: &CrossProjectBlockEvent, logger: Option<&AuditLogger>) {
    if let Some(logger) = logger {
        match logger.log_entry(event.to_audit_entry()) {
            Ok(()) => {
                if let Err(e) = logger.rotate_if_needed() {
                    tracing::warn!(
                        error = %e,
                        fingerprint = %event.fingerprint(),
                        "cross_project_block: AuditLogger.rotate_if_needed failed",
                    );
                }
                return;
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    fingerprint = %event.fingerprint(),
                    reason = event.reason.as_str(),
                    guard_layer = event.guard_layer.as_str(),
                    "cross_project_block: AuditLogger.log_entry failed, falling back to tracing",
                );
            }
        }
    }
    tracing::warn!(
        fingerprint = %event.fingerprint(),
        reason = event.reason.as_str(),
        guard_layer = event.guard_layer.as_str(),
        correlation_id = event.correlation_id,
        actor_user = %event.actor_user_id,
        actor_cloud = %event.actor_cloud,
        action = %event.action_type,
        resource_kind = %event.resource_kind,
        "cross_project_block",
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use sha2::{Digest, Sha256};

    use crate::infra::audit::{AuditLogger, AuditResult};
    use crate::infra::cross_project_guard::{CrossProjectReason, GuardLayer};

    fn sample_event() -> CrossProjectBlockEvent {
        CrossProjectBlockEvent {
            timestamp: chrono::Utc.with_ymd_and_hms(2026, 4, 27, 12, 0, 0).unwrap(),
            actor_user_id: "user-1".to_string(),
            actor_cloud: "devstack".to_string(),
            active_project_id: Some("active-1".to_string()),
            asserted_origin_project_id: Some("origin-2".to_string()),
            target_project_id: Some("target-3".to_string()),
            action_type: "CreateServer".to_string(),
            resource_kind: "server".to_string(),
            resource_id: Some("res-99".to_string()),
            resource_name: Some("web-01".to_string()),
            reason: CrossProjectReason::OriginScopeMismatch {
                origin: "origin-2".to_string(),
                active: "active-1".to_string(),
            },
            guard_layer: GuardLayer::Fr2Worker,
            correlation_id: 42,
        }
    }

    fn hex12(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(12);
        for b in &bytes[..6] {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }

    #[test]
    fn test_event_to_audit_entry_field_mapping() {
        let event = sample_event();
        let entry = event.to_audit_entry();

        assert_eq!(entry.timestamp, "2026-04-27T12:00:00+00:00");
        assert_eq!(entry.cloud, "devstack");
        assert_eq!(entry.user, "user-1");
        assert_eq!(entry.project.as_deref(), Some("active-1"));
        assert_eq!(entry.action, "CreateServer");
        assert_eq!(entry.resource_type, "server");
        assert_eq!(entry.resource_id, "res-99");
        assert_eq!(entry.resource_name.as_deref(), Some("web-01"));
        assert!(entry.details.is_some(), "details must be packed");
    }

    #[test]
    fn test_audit_entry_details_contains_fingerprint_guard_layer_correlation_id() {
        let event = sample_event();
        let entry = event.to_audit_entry();
        let details = entry.details.expect("details present");

        assert!(details.get("fingerprint").is_some(), "fingerprint missing");
        assert_eq!(details["guard_layer"], "fr2_worker");
        assert_eq!(details["correlation_id"], 42);
        assert_eq!(details["asserted_origin_project_id"], "origin-2");
        assert_eq!(details["target_project_id"], "target-3");
    }

    #[test]
    fn test_audit_entry_result_is_failed_with_reason_string() {
        let event = sample_event();
        let entry = event.to_audit_entry();
        match entry.result {
            AuditResult::Failed(s) => {
                assert_eq!(s, "cross_project_block:origin_scope_mismatch");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn test_fingerprint_v1_canonical_format() {
        let event = sample_event();
        let fp = event.fingerprint();

        // length + lowercase hex contract
        assert_eq!(fp.len(), 12, "fingerprint must be 12 hex chars");
        assert!(
            fp.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "fingerprint must be lowercase hex: {fp}"
        );

        // canonical formula re-derived independently of impl
        let canonical = "v1|user-1|active-1|origin-2|target-3|CreateServer|res-99";
        let mut h = Sha256::new();
        h.update(canonical.as_bytes());
        let expected = hex12(&h.finalize());
        assert_eq!(
            fp, expected,
            "fingerprint canonical changed — bump v1→v2 + migrate analysts"
        );
    }

    #[test]
    fn test_fingerprint_boundary_collision_free() {
        let mut e1 = sample_event();
        e1.actor_user_id = "ab".to_string();
        e1.active_project_id = Some(String::new());
        let mut e2 = sample_event();
        e2.actor_user_id = "a".to_string();
        e2.active_project_id = Some("b".to_string());

        assert_ne!(
            e1.fingerprint(),
            e2.fingerprint(),
            "delimiter must prevent ('ab','') vs ('a','b') collision"
        );
    }

    #[test]
    fn test_fingerprint_none_resource_id_uses_empty() {
        let mut e_none = sample_event();
        e_none.resource_id = None;
        let mut e_empty = sample_event();
        e_empty.resource_id = Some(String::new());

        assert_eq!(
            e_none.fingerprint(),
            e_empty.fingerprint(),
            "None resource_id and empty string must hash identically"
        );
    }

    #[test]
    fn test_emit_with_logger_writes_audit_entry() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("audit.log");
        let logger = AuditLogger::new(path.clone()).unwrap();

        let event = sample_event();
        emit(&event, Some(&logger));

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(!content.is_empty(), "audit log must contain entry");
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed["action"], "CreateServer");
        assert_eq!(parsed["result"], serde_json::json!({ "failed": "cross_project_block:origin_scope_mismatch" }));
        assert_eq!(parsed["details"]["guard_layer"], "fr2_worker");
        assert_eq!(parsed["details"]["correlation_id"], 42);
    }

    #[test]
    fn test_emit_without_logger_fallback_to_tracing() {
        let event = sample_event();
        // Must not panic. Tracing subscriber is not asserted (no tracing-test dep).
        emit(&event, None);
    }

    // --- BL-P2-085 Step 11b: convenience constructor ---

    #[test]
    fn test_new_convenience_constructor_fills_required_fields_and_now_timestamp() {
        let before = Utc::now();
        let event = CrossProjectBlockEvent::new(
            CrossProjectReason::OriginScopeMismatch {
                origin: "p-stale".into(),
                active: "p-active".into(),
            },
            GuardLayer::Fr2Worker,
            "DeleteServer",
            "server",
            "devstack",
            "user-uuid",
            Some("p-active".into()),
            Some("p-stale".into()),
            7,
        );
        let after = Utc::now();

        assert_eq!(event.actor_cloud, "devstack");
        assert_eq!(event.actor_user_id, "user-uuid");
        assert_eq!(event.action_type, "DeleteServer");
        assert_eq!(event.resource_kind, "server");
        assert_eq!(event.active_project_id.as_deref(), Some("p-active"));
        assert_eq!(event.asserted_origin_project_id.as_deref(), Some("p-stale"));
        assert_eq!(event.guard_layer, GuardLayer::Fr2Worker);
        assert_eq!(event.correlation_id, 7);
        // Optional fields default to None — caller can override after construction.
        assert!(event.target_project_id.is_none());
        assert!(event.resource_id.is_none());
        assert!(event.resource_name.is_none());
        // Timestamp is "now" — within the call window.
        assert!(
            event.timestamp >= before && event.timestamp <= after,
            "timestamp must reflect Utc::now() at construction"
        );
    }
}
