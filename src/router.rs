use std::collections::VecDeque;

use crate::models::common::Route;

const MAX_HISTORY: usize = 20;

#[derive(Debug)]
pub struct Router {
    current: Route,
    history: VecDeque<Route>,
}

impl Router {
    pub fn new(initial: Route) -> Self {
        Self {
            current: initial,
            history: VecDeque::new(),
        }
    }

    /// Navigate to a new route, pushing current onto history stack.
    /// Does nothing if already on the target route.
    pub fn navigate(&mut self, to: Route) {
        if self.current == to {
            return;
        }
        self.history.push_back(self.current);
        if self.history.len() > MAX_HISTORY {
            self.history.pop_front();
        }
        self.current = to;
    }

    /// Pop history stack, return to previous route.
    pub fn back(&mut self) -> Option<Route> {
        if let Some(prev) = self.history.pop_back() {
            self.current = prev;
            Some(prev)
        } else {
            None
        }
    }

    /// Current active route.
    pub fn current(&self) -> Route {
        self.current
    }

    /// Peek at previous route (for breadcrumb display).
    pub fn previous(&self) -> Option<Route> {
        self.history.back().copied()
    }

    /// Replace current route without pushing to history.
    pub fn replace(&mut self, to: Route) {
        self.current = to;
    }

    /// Clear history and navigate (e.g., on cloud context switch).
    pub fn reset(&mut self, to: Route) {
        self.history.clear();
        self.current = to;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_navigate() {
        let mut router = Router::new(Route::Servers);
        router.navigate(Route::Networks);
        assert_eq!(router.current(), Route::Networks);
        assert_eq!(router.previous(), Some(Route::Servers));
    }

    #[test]
    fn test_router_back() {
        let mut router = Router::new(Route::Servers);
        router.navigate(Route::Networks);
        router.navigate(Route::Volumes);

        let prev = router.back();
        assert_eq!(prev, Some(Route::Networks));
        assert_eq!(router.current(), Route::Networks);

        let prev = router.back();
        assert_eq!(prev, Some(Route::Servers));
        assert_eq!(router.current(), Route::Servers);
    }

    #[test]
    fn test_router_back_empty() {
        let mut router = Router::new(Route::Servers);
        assert_eq!(router.back(), None);
        assert_eq!(router.current(), Route::Servers);
    }

    #[test]
    fn test_router_replace() {
        let mut router = Router::new(Route::Servers);
        router.navigate(Route::Networks);
        router.replace(Route::Volumes);

        assert_eq!(router.current(), Route::Volumes);
        // history should still have Servers, not Networks
        let prev = router.back();
        assert_eq!(prev, Some(Route::Servers));
    }

    #[test]
    fn test_router_reset() {
        let mut router = Router::new(Route::Servers);
        router.navigate(Route::Networks);
        router.navigate(Route::Volumes);

        router.reset(Route::Projects);
        assert_eq!(router.current(), Route::Projects);
        assert_eq!(router.back(), None);
    }

    #[test]
    fn test_router_history_limit() {
        let mut router = Router::new(Route::Servers);
        let routes = [
            Route::Networks,
            Route::Volumes,
            Route::Images,
            Route::Projects,
            Route::Users,
            Route::Flavors,
            Route::Aggregates,
            Route::ComputeServices,
            Route::Hypervisors,
            Route::SecurityGroups,
            Route::FloatingIps,
            Route::Agents,
            Route::Snapshots,
            Route::Usage,
            Route::ServerDetail,
            Route::NetworkDetail,
            Route::VolumeDetail,
            Route::ImageDetail,
            Route::SecurityGroupDetail,
            Route::ServerCreate,
            Route::VolumeCreate, // 21st navigate → should evict oldest
        ];
        for &r in &routes {
            router.navigate(r);
        }
        // History should be capped at 20
        assert!(router.history.len() <= MAX_HISTORY);
        assert_eq!(router.current(), Route::VolumeCreate);
        // Oldest (Servers) should have been evicted
        assert!(!router.history.contains(&Route::Servers));
    }

    #[test]
    fn test_router_navigate_same_route() {
        let mut router = Router::new(Route::Servers);
        router.navigate(Route::Servers);
        // Should not push to history
        assert_eq!(router.back(), None);
        assert_eq!(router.current(), Route::Servers);
    }
}
