use std::collections::{HashSet, VecDeque};

use crate::action::Action;
use crate::port::types::EvacuateParams;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvacState {
    Idle,
    Executing,
    Completed,
}

pub struct EvacTask {
    state: EvacState,
    queue: VecDeque<String>,
    in_flight: HashSet<String>,
    completed: Vec<(String, Result<(), String>)>,
    cancel_requested: bool,
    params: EvacuateParams,
    max_concurrent: usize,
}

impl EvacTask {
    pub fn new(server_ids: Vec<String>, params: EvacuateParams, max_concurrent: usize) -> Self {
        Self {
            state: EvacState::Idle,
            queue: server_ids.into(),
            in_flight: HashSet::new(),
            completed: Vec::new(),
            cancel_requested: false,
            params,
            max_concurrent: max_concurrent.max(1),
        }
    }

    pub fn start(&mut self) {
        self.state = EvacState::Executing;
    }

    pub fn is_idle(&self) -> bool {
        self.state == EvacState::Idle
    }

    pub fn is_executing(&self) -> bool {
        self.state == EvacState::Executing
    }

    pub fn is_completed(&self) -> bool {
        self.state == EvacState::Completed
    }

    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    pub fn poll_next(&mut self) -> Vec<Action> {
        if self.state != EvacState::Executing {
            return vec![];
        }
        let mut actions = vec![];
        while self.in_flight.len() < self.max_concurrent && !self.cancel_requested {
            match self.queue.pop_front() {
                Some(id) => {
                    self.in_flight.insert(id.clone());
                    actions.push(Action::EvacuateServer {
                        id,
                        params: self.params.clone(),
                    });
                }
                None => break,
            }
        }
        if self.queue.is_empty() && self.in_flight.is_empty() {
            self.state = EvacState::Completed;
        }
        actions
    }

    pub fn on_completed(&mut self, server_id: &str, result: Result<(), String>) {
        if !self.in_flight.remove(server_id) {
            return; // ignore phantom completions
        }
        self.completed.push((server_id.to_string(), result));
        if self.queue.is_empty() && self.in_flight.is_empty() {
            self.state = EvacState::Completed;
        }
    }

    pub fn request_cancel(&mut self) {
        self.cancel_requested = true;
        while let Some(id) = self.queue.pop_front() {
            self.completed.push((id, Err("Aborted".into())));
        }
        if self.in_flight.is_empty() {
            self.state = EvacState::Completed;
        }
    }

    pub fn progress(&self) -> (usize, usize) {
        let total = self.completed.len() + self.in_flight.len() + self.queue.len();
        (self.completed.len(), total)
    }

    pub fn results(&self) -> &[(String, Result<(), String>)] {
        &self.completed
    }

    pub fn succeeded_count(&self) -> usize {
        self.completed.iter().filter(|(_, r)| r.is_ok()).count()
    }

    pub fn failed_results(&self) -> Vec<&(String, Result<(), String>)> {
        self.completed.iter().filter(|(_, r)| r.is_err()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evac_task_idle_to_executing() {
        let mut task = EvacTask::new(vec!["s1".into(), "s2".into()], EvacuateParams::default(), 2);
        assert!(task.is_idle());

        task.start();
        assert!(task.is_executing());

        let actions = task.poll_next();
        assert_eq!(actions.len(), 2);
        assert_eq!(task.in_flight_count(), 2);
    }

    #[test]
    fn test_evac_task_max_concurrent_limit() {
        let mut task = EvacTask::new(
            vec!["s1".into(), "s2".into(), "s3".into()],
            EvacuateParams::default(),
            2,
        );
        task.start();
        let batch1 = task.poll_next();
        assert_eq!(batch1.len(), 2);
        assert_eq!(task.poll_next().len(), 0); // in_flight full

        task.on_completed("s1", Ok(()));
        let batch2 = task.poll_next();
        assert_eq!(batch2.len(), 1); // s3 dispatched
    }

    #[test]
    fn test_evac_task_completes_when_all_done() {
        let mut task = EvacTask::new(vec!["s1".into()], EvacuateParams::default(), 2);
        task.start();
        task.poll_next();
        assert!(!task.is_completed());

        task.on_completed("s1", Ok(()));
        assert!(task.is_completed());
        assert_eq!(task.succeeded_count(), 1);
        assert_eq!(task.progress(), (1, 1));
    }

    #[test]
    fn test_evac_task_tracks_failures() {
        let mut task = EvacTask::new(vec!["s1".into(), "s2".into()], EvacuateParams::default(), 2);
        task.start();
        task.poll_next();

        task.on_completed("s1", Ok(()));
        task.on_completed("s2", Err("server locked".into()));

        assert!(task.is_completed());
        assert_eq!(task.succeeded_count(), 1);
        assert_eq!(task.failed_results().len(), 1);
    }

    #[test]
    fn test_evac_task_cancel() {
        let mut task = EvacTask::new(
            vec!["s1".into(), "s2".into(), "s3".into()],
            EvacuateParams::default(),
            1,
        );
        task.start();
        task.poll_next(); // s1 in_flight
        task.request_cancel();
        assert_eq!(task.poll_next().len(), 0); // no more dispatches
        // s2, s3 are Aborted
        task.on_completed("s1", Ok(()));
        assert!(task.is_completed());
        assert_eq!(task.succeeded_count(), 1);
        assert_eq!(task.failed_results().len(), 2); // s2, s3 aborted
    }

    #[test]
    fn test_evac_task_empty_queue() {
        let mut task = EvacTask::new(vec![], EvacuateParams::default(), 2);
        task.start();
        let actions = task.poll_next();
        assert!(actions.is_empty());
        assert!(task.is_completed());
    }

    #[test]
    fn test_evac_task_progress() {
        let mut task = EvacTask::new(
            vec!["s1".into(), "s2".into(), "s3".into()],
            EvacuateParams::default(),
            1,
        );
        assert_eq!(task.progress(), (0, 3));
        task.start();
        task.poll_next(); // s1 in flight
        assert_eq!(task.progress(), (0, 3));
        task.on_completed("s1", Ok(()));
        assert_eq!(task.progress(), (1, 3));
    }

    #[test]
    fn test_evac_task_does_not_dispatch_when_idle() {
        let mut task = EvacTask::new(vec!["s1".into()], EvacuateParams::default(), 2);
        // Don't call start()
        let actions = task.poll_next();
        assert!(actions.is_empty());
        assert!(task.is_idle());
    }

    #[test]
    fn test_evac_task_max_concurrent_zero_clamped_to_one() {
        let mut task = EvacTask::new(
            vec!["s1".into(), "s2".into()],
            EvacuateParams::default(),
            0, // should be clamped to 1
        );
        task.start();
        let batch = task.poll_next();
        assert_eq!(batch.len(), 1); // dispatches 1, not 0
        assert_eq!(task.in_flight_count(), 1);
    }

    #[test]
    fn test_evac_task_phantom_completion_ignored() {
        let mut task = EvacTask::new(vec!["s1".into()], EvacuateParams::default(), 2);
        task.start();
        task.poll_next(); // s1 in flight

        // Phantom completion for unknown server — should be ignored
        task.on_completed("unknown-server", Ok(()));
        assert_eq!(task.in_flight_count(), 1); // s1 still in flight
        assert_eq!(task.succeeded_count(), 0); // not counted

        // Real completion
        task.on_completed("s1", Ok(()));
        assert!(task.is_completed());
        assert_eq!(task.succeeded_count(), 1);
    }
}
