use std::collections::HashMap;
use std::time::{Duration, Instant};

const TOAST_TTL_SUCCESS: Duration = Duration::from_secs(5);
const TOAST_TTL_ERROR: Duration = Duration::from_secs(30);
const TOAST_TTL_INFO: Duration = Duration::from_secs(5);
const GC_AGE: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub struct BackgroundTracker {
    operations: HashMap<String, OperationInfo>,
    toasts: Vec<Toast>,
}

#[derive(Debug)]
pub struct OperationInfo {
    pub description: String,
    pub started_at: Instant,
    pub finished_at: Option<Instant>,
    pub status: OperationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationStatus {
    InProgress,
    Completed,
    Failed(String),
}

#[derive(Debug)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub created_at: Instant,
    pub ttl: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Success,
    Error,
    Info,
}

#[derive(Debug)]
pub enum TrackingEvent {
    Started {
        operation_id: String,
        description: String,
    },
    Completed {
        operation_id: String,
    },
    Failed {
        operation_id: String,
        error: String,
    },
}

impl BackgroundTracker {
    pub fn new() -> Self {
        Self {
            operations: HashMap::new(),
            toasts: Vec::new(),
        }
    }

    /// Process a tracking event, updating internal state.
    pub fn handle_tracking_event(&mut self, event: TrackingEvent) {
        match event {
            TrackingEvent::Started {
                operation_id,
                description,
            } => {
                self.operations.insert(
                    operation_id,
                    OperationInfo {
                        description,
                        started_at: Instant::now(),
                        finished_at: None,
                        status: OperationStatus::InProgress,
                    },
                );
            }
            TrackingEvent::Completed { operation_id } => {
                if let Some(op) = self.operations.get_mut(&operation_id) {
                    op.status = OperationStatus::Completed;
                    op.finished_at = Some(Instant::now());
                    self.toasts.push(Toast {
                        message: format!("{} completed", op.description),
                        level: ToastLevel::Success,
                        created_at: Instant::now(),
                        ttl: TOAST_TTL_SUCCESS,
                    });
                }
            }
            TrackingEvent::Failed {
                operation_id,
                error,
            } => {
                if let Some(op) = self.operations.get_mut(&operation_id) {
                    op.status = OperationStatus::Failed(error.clone());
                    op.finished_at = Some(Instant::now());
                    self.toasts.push(Toast {
                        message: format!("{} failed: {}", op.description, error),
                        level: ToastLevel::Error,
                        created_at: Instant::now(),
                        ttl: TOAST_TTL_ERROR,
                    });
                }
            }
        }
    }

    /// Add a toast directly (not from tracking).
    pub fn add_toast(&mut self, message: String, level: ToastLevel) {
        let ttl = match level {
            ToastLevel::Success => TOAST_TTL_SUCCESS,
            ToastLevel::Error => TOAST_TTL_ERROR,
            ToastLevel::Info => TOAST_TTL_INFO,
        };
        self.toasts.push(Toast {
            message,
            level,
            created_at: Instant::now(),
            ttl,
        });
    }

    /// Remove expired toasts.
    pub fn expire_toasts(&mut self) {
        let now = Instant::now();
        self.toasts
            .retain(|t| now.duration_since(t.created_at) < t.ttl);
    }

    /// Get active (non-expired) toasts for rendering.
    pub fn active_toasts(&self) -> &[Toast] {
        &self.toasts
    }

    /// Get all currently in-progress operations.
    pub fn in_progress(&self) -> Vec<&OperationInfo> {
        self.operations
            .values()
            .filter(|op| op.status == OperationStatus::InProgress)
            .collect()
    }

    /// Get count of in-progress operations.
    pub fn in_progress_count(&self) -> usize {
        self.operations
            .values()
            .filter(|op| op.status == OperationStatus::InProgress)
            .count()
    }

    /// Clean up completed/failed entries older than 60s.
    pub fn gc_old_entries(&mut self) {
        let now = Instant::now();
        self.operations.retain(|_, op| {
            if op.status == OperationStatus::InProgress {
                return true;
            }
            // GC based on finished_at, not started_at
            match op.finished_at {
                Some(finished) => now.duration_since(finished) < GC_AGE,
                None => true, // shouldn't happen, but keep if no finish time
            }
        });
    }
}

impl Default for BackgroundTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracker_started() {
        let mut tracker = BackgroundTracker::new();
        tracker.handle_tracking_event(TrackingEvent::Started {
            operation_id: "op1".into(),
            description: "Deleting server web-01".into(),
        });
        assert_eq!(tracker.in_progress_count(), 1);
        let ops = tracker.in_progress();
        assert_eq!(ops[0].description, "Deleting server web-01");
    }

    #[test]
    fn test_tracker_completed() {
        let mut tracker = BackgroundTracker::new();
        tracker.handle_tracking_event(TrackingEvent::Started {
            operation_id: "op1".into(),
            description: "Deleting server".into(),
        });
        tracker.handle_tracking_event(TrackingEvent::Completed {
            operation_id: "op1".into(),
        });
        assert_eq!(tracker.in_progress_count(), 0);
        assert_eq!(tracker.active_toasts().len(), 1);
        assert_eq!(tracker.active_toasts()[0].level, ToastLevel::Success);
    }

    #[test]
    fn test_tracker_failed() {
        let mut tracker = BackgroundTracker::new();
        tracker.handle_tracking_event(TrackingEvent::Started {
            operation_id: "op1".into(),
            description: "Deleting server".into(),
        });
        tracker.handle_tracking_event(TrackingEvent::Failed {
            operation_id: "op1".into(),
            error: "not found".into(),
        });
        assert_eq!(tracker.in_progress_count(), 0);
        assert_eq!(tracker.active_toasts().len(), 1);
        assert_eq!(tracker.active_toasts()[0].level, ToastLevel::Error);
        assert!(tracker.active_toasts()[0].message.contains("not found"));
    }

    #[test]
    fn test_toast_expiry() {
        let mut tracker = BackgroundTracker::new();
        // Add a toast with very short TTL
        tracker.toasts.push(Toast {
            message: "old".into(),
            level: ToastLevel::Info,
            created_at: Instant::now() - Duration::from_secs(10),
            ttl: Duration::from_millis(1),
        });
        tracker.add_toast("new".into(), ToastLevel::Info);

        tracker.expire_toasts();
        assert_eq!(tracker.active_toasts().len(), 1);
        assert_eq!(tracker.active_toasts()[0].message, "new");
    }

    #[test]
    fn test_tracker_gc() {
        let mut tracker = BackgroundTracker::new();
        // Insert a completed operation with old timestamp
        tracker.operations.insert(
            "old_op".into(),
            OperationInfo {
                description: "old".into(),
                started_at: Instant::now() - Duration::from_secs(180),
                finished_at: Some(Instant::now() - Duration::from_secs(120)),
                status: OperationStatus::Completed,
            },
        );
        // Insert an in-progress operation
        tracker.handle_tracking_event(TrackingEvent::Started {
            operation_id: "active_op".into(),
            description: "active".into(),
        });

        tracker.gc_old_entries();
        assert_eq!(tracker.operations.len(), 1);
        assert!(tracker.operations.contains_key("active_op"));
    }

    #[test]
    fn test_in_progress_count() {
        let mut tracker = BackgroundTracker::new();
        assert_eq!(tracker.in_progress_count(), 0);

        tracker.handle_tracking_event(TrackingEvent::Started {
            operation_id: "op1".into(),
            description: "op1".into(),
        });
        tracker.handle_tracking_event(TrackingEvent::Started {
            operation_id: "op2".into(),
            description: "op2".into(),
        });
        assert_eq!(tracker.in_progress_count(), 2);

        tracker.handle_tracking_event(TrackingEvent::Completed {
            operation_id: "op1".into(),
        });
        assert_eq!(tracker.in_progress_count(), 1);
    }
}
