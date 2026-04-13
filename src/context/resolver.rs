//! User-input → authoritative `ContextTarget` resolution.
//!
//! BL-P2-031 Unit 4. The resolver has two jobs:
//! 1. Turn the unresolved [`ContextRequest`] into a [`ContextTarget`] by
//!    matching the request against the set of projects the current user can
//!    reach in the target cloud.
//! 2. Expose a list of the user's reachable projects so the picker widget
//!    and command tab-completion share one source of truth.
//!
//! Disambiguation policy:
//! - Zero matches → [`SwitchError::NotFound`] with the requested name.
//! - Multiple matches → [`SwitchError::Ambiguous`] with the candidates.
//!   If the caller supplied a domain, it is used to narrow the match before
//!   declaring ambiguity.
//! - Single match → [`ContextTarget`] with authoritative `project_id` and
//!   `domain` filled in from the directory.
//!
//! Cloud-prefix syntax: `ByName` requests may encode the cloud inline via
//! `"cloud/project"`. The explicit `cloud` field wins if both are present.
//!
//! The resolver intentionally does not depend on a concrete Keystone
//! adapter — Unit 3b plugs the real HTTP impl in later. The port trait
//! [`ProjectDirectoryPort`] is the single seam, so the picker, parser, and
//! switcher all share one disambiguation path.

use std::sync::Arc;

use async_trait::async_trait;

use super::error::SwitchError;
use super::types::{ContextRequest, ContextTarget};

/// Project reachable by the current user. `cloud` is populated by the
/// directory implementation; the resolver does not need to know how to
/// retrieve it, only that it is authoritative.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectCandidate {
    pub cloud: String,
    pub project_id: String,
    pub project_name: String,
    pub domain: String,
}

impl From<ProjectCandidate> for ContextTarget {
    fn from(p: ProjectCandidate) -> Self {
        ContextTarget {
            cloud: p.cloud,
            project_id: p.project_id,
            project_name: p.project_name,
            domain: p.domain,
        }
    }
}

/// Read-only view over the configured clouds. Kept narrow so tests can
/// supply a trivial double without pulling in the full `Config`.
pub trait CloudDirectory: Send + Sync {
    fn active_cloud(&self) -> String;
    fn known_clouds(&self) -> Vec<String>;
}

/// Port over "which projects can the current user reach in a given cloud".
/// The real implementation calls Keystone's `/v3/auth/projects`; tests use
/// an in-memory double.
#[async_trait]
pub trait ProjectDirectoryPort: Send + Sync {
    async fn list_projects(&self, cloud: &str) -> Result<Vec<ProjectCandidate>, SwitchError>;
}

pub struct ContextTargetResolver {
    clouds: Arc<dyn CloudDirectory>,
    directory: Arc<dyn ProjectDirectoryPort>,
}

impl ContextTargetResolver {
    pub fn new(clouds: Arc<dyn CloudDirectory>, directory: Arc<dyn ProjectDirectoryPort>) -> Self {
        Self { clouds, directory }
    }

    /// Resolve a request to its authoritative target. Performs cloud
    /// resolution, inline `cloud/project` parsing, and disambiguation
    /// against [`ProjectDirectoryPort`].
    pub async fn resolve(&self, request: ContextRequest) -> Result<ContextTarget, SwitchError> {
        match request {
            ContextRequest::ByName { cloud, project, domain } => {
                let (cloud, project_name) = normalize_cloud_project(cloud, project, &*self.clouds)?;
                let candidates = self.directory.list_projects(&cloud).await?;
                let filtered: Vec<ProjectCandidate> = candidates
                    .into_iter()
                    .filter(|c| c.project_name == project_name)
                    .filter(|c| match &domain {
                        Some(d) => &c.domain == d,
                        None => true,
                    })
                    .collect();
                match filtered.len() {
                    0 => Err(SwitchError::NotFound(project_name)),
                    1 => Ok(filtered.into_iter().next().unwrap().into()),
                    _ => Err(SwitchError::Ambiguous {
                        candidates: filtered.into_iter().map(Into::into).collect(),
                    }),
                }
            }
            ContextRequest::ById { cloud, project_id } => {
                let cloud = cloud.unwrap_or_else(|| self.clouds.active_cloud());
                self.validate_cloud(&cloud)?;
                let candidates = self.directory.list_projects(&cloud).await?;
                candidates
                    .into_iter()
                    .find(|c| c.project_id == project_id)
                    .map(Into::into)
                    .ok_or(SwitchError::NotFound(project_id))
            }
        }
    }

    /// List every project the current user can reach across every cloud.
    /// Used by the picker UI and by `:switch-project` tab-completion. A
    /// per-cloud failure aborts the whole call — the caller has no better
    /// answer than to surface the first error.
    pub async fn list_user_projects(&self) -> Result<Vec<ContextTarget>, SwitchError> {
        let mut out = Vec::new();
        for cloud in self.clouds.known_clouds() {
            let projects = self.directory.list_projects(&cloud).await?;
            out.extend(projects.into_iter().map(Into::into));
        }
        Ok(out)
    }

    fn validate_cloud(&self, cloud: &str) -> Result<(), SwitchError> {
        if self.clouds.known_clouds().iter().any(|c| c == cloud) {
            Ok(())
        } else {
            Err(SwitchError::NotFound(format!("cloud '{cloud}'")))
        }
    }
}

/// Splits `cloud/project` inline syntax and resolves the cloud. Explicit
/// `cloud` arg wins over the inline prefix; absent both, the active cloud
/// is used.
fn normalize_cloud_project(
    cloud_arg: Option<String>,
    project: String,
    clouds: &dyn CloudDirectory,
) -> Result<(String, String), SwitchError> {
    let (prefix_cloud, project_name) = match project.split_once('/') {
        Some((c, p)) if !c.is_empty() && !p.is_empty() => (Some(c.to_string()), p.to_string()),
        _ => (None, project),
    };
    let cloud = cloud_arg
        .or(prefix_cloud)
        .unwrap_or_else(|| clouds.active_cloud());
    if !clouds.known_clouds().iter().any(|c| c == &cloud) {
        return Err(SwitchError::NotFound(format!("cloud '{cloud}'")));
    }
    Ok((cloud, project_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct FakeClouds {
        active: String,
        known: Vec<String>,
    }

    impl CloudDirectory for FakeClouds {
        fn active_cloud(&self) -> String {
            self.active.clone()
        }
        fn known_clouds(&self) -> Vec<String> {
            self.known.clone()
        }
    }

    #[derive(Default)]
    struct FakeDirectory {
        // cloud -> project list
        data: Mutex<std::collections::HashMap<String, Vec<ProjectCandidate>>>,
        failure: Mutex<Option<SwitchError>>,
    }

    impl FakeDirectory {
        fn with(data: std::collections::HashMap<String, Vec<ProjectCandidate>>) -> Self {
            Self {
                data: Mutex::new(data),
                failure: Mutex::new(None),
            }
        }
        fn fail_with(self, err: SwitchError) -> Self {
            *self.failure.lock().unwrap() = Some(err);
            self
        }
    }

    #[async_trait]
    impl ProjectDirectoryPort for FakeDirectory {
        async fn list_projects(&self, cloud: &str) -> Result<Vec<ProjectCandidate>, SwitchError> {
            if let Some(err) = self.failure.lock().unwrap().take() {
                return Err(err);
            }
            Ok(self
                .data
                .lock()
                .unwrap()
                .get(cloud)
                .cloned()
                .unwrap_or_default())
        }
    }

    fn candidate(cloud: &str, name: &str, id: &str, domain: &str) -> ProjectCandidate {
        ProjectCandidate {
            cloud: cloud.into(),
            project_id: id.into(),
            project_name: name.into(),
            domain: domain.into(),
        }
    }

    fn clouds(active: &str, known: &[&str]) -> Arc<dyn CloudDirectory> {
        Arc::new(FakeClouds {
            active: active.into(),
            known: known.iter().map(|s| s.to_string()).collect(),
        })
    }

    fn directory(
        data: &[(&str, Vec<ProjectCandidate>)],
    ) -> Arc<dyn ProjectDirectoryPort> {
        let mut map = std::collections::HashMap::new();
        for (cloud, projects) in data {
            map.insert((*cloud).to_string(), projects.clone());
        }
        Arc::new(FakeDirectory::with(map))
    }

    #[tokio::test]
    async fn resolve_by_name_single_match_returns_target() {
        let resolver = ContextTargetResolver::new(
            clouds("devstack", &["devstack"]),
            directory(&[(
                "devstack",
                vec![candidate("devstack", "admin", "id-1", "default")],
            )]),
        );
        let target = resolver
            .resolve(ContextRequest::ByName {
                cloud: None,
                project: "admin".into(),
                domain: None,
            })
            .await
            .unwrap();
        assert_eq!(target.project_name, "admin");
        assert_eq!(target.project_id, "id-1");
        assert_eq!(target.cloud, "devstack");
    }

    #[tokio::test]
    async fn resolve_by_name_ambiguous_returns_all_candidates() {
        let resolver = ContextTargetResolver::new(
            clouds("devstack", &["devstack"]),
            directory(&[(
                "devstack",
                vec![
                    candidate("devstack", "admin", "id-1", "default"),
                    candidate("devstack", "admin", "id-2", "heat"),
                ],
            )]),
        );
        let err = resolver
            .resolve(ContextRequest::ByName {
                cloud: None,
                project: "admin".into(),
                domain: None,
            })
            .await
            .unwrap_err();
        match err {
            SwitchError::Ambiguous { candidates } => {
                assert_eq!(candidates.len(), 2);
                let domains: Vec<_> = candidates.iter().map(|t| t.domain.as_str()).collect();
                assert!(domains.contains(&"default"));
                assert!(domains.contains(&"heat"));
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn resolve_by_name_with_domain_disambiguates() {
        let resolver = ContextTargetResolver::new(
            clouds("devstack", &["devstack"]),
            directory(&[(
                "devstack",
                vec![
                    candidate("devstack", "admin", "id-1", "default"),
                    candidate("devstack", "admin", "id-2", "heat"),
                ],
            )]),
        );
        let target = resolver
            .resolve(ContextRequest::ByName {
                cloud: None,
                project: "admin".into(),
                domain: Some("heat".into()),
            })
            .await
            .unwrap();
        assert_eq!(target.project_id, "id-2");
    }

    #[tokio::test]
    async fn resolve_by_name_not_found_returns_name() {
        let resolver = ContextTargetResolver::new(
            clouds("devstack", &["devstack"]),
            directory(&[("devstack", vec![])]),
        );
        let err = resolver
            .resolve(ContextRequest::ByName {
                cloud: None,
                project: "ghost".into(),
                domain: None,
            })
            .await
            .unwrap_err();
        match err {
            SwitchError::NotFound(s) => assert_eq!(s, "ghost"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn resolve_cloud_prefix_syntax_routes_to_named_cloud() {
        let resolver = ContextTargetResolver::new(
            clouds("devstack", &["devstack", "prod"]),
            directory(&[
                ("devstack", vec![candidate("devstack", "admin", "d-1", "default")]),
                ("prod", vec![candidate("prod", "admin", "p-1", "default")]),
            ]),
        );
        let target = resolver
            .resolve(ContextRequest::ByName {
                cloud: None,
                project: "prod/admin".into(),
                domain: None,
            })
            .await
            .unwrap();
        assert_eq!(target.cloud, "prod");
        assert_eq!(target.project_id, "p-1");
    }

    #[tokio::test]
    async fn explicit_cloud_arg_wins_over_inline_prefix() {
        let resolver = ContextTargetResolver::new(
            clouds("devstack", &["devstack", "prod"]),
            directory(&[
                ("devstack", vec![candidate("devstack", "admin", "d-1", "default")]),
                ("prod", vec![candidate("prod", "admin", "p-1", "default")]),
            ]),
        );
        let target = resolver
            .resolve(ContextRequest::ByName {
                cloud: Some("devstack".into()),
                // prefix says "prod" but explicit cloud arg is devstack
                project: "prod/admin".into(),
                domain: None,
            })
            .await
            .unwrap();
        assert_eq!(target.cloud, "devstack");
        assert_eq!(target.project_id, "d-1");
    }

    #[tokio::test]
    async fn unknown_cloud_returns_not_found() {
        let resolver = ContextTargetResolver::new(
            clouds("devstack", &["devstack"]),
            directory(&[("devstack", vec![])]),
        );
        let err = resolver
            .resolve(ContextRequest::ByName {
                cloud: Some("nope".into()),
                project: "admin".into(),
                domain: None,
            })
            .await
            .unwrap_err();
        match err {
            SwitchError::NotFound(s) => assert!(s.contains("cloud 'nope'")),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn resolve_by_id_returns_target() {
        let resolver = ContextTargetResolver::new(
            clouds("devstack", &["devstack"]),
            directory(&[(
                "devstack",
                vec![candidate("devstack", "admin", "id-1", "default")],
            )]),
        );
        let target = resolver
            .resolve(ContextRequest::ById {
                cloud: None,
                project_id: "id-1".into(),
            })
            .await
            .unwrap();
        assert_eq!(target.project_name, "admin");
    }

    #[tokio::test]
    async fn resolve_by_id_not_found() {
        let resolver = ContextTargetResolver::new(
            clouds("devstack", &["devstack"]),
            directory(&[("devstack", vec![])]),
        );
        let err = resolver
            .resolve(ContextRequest::ById {
                cloud: None,
                project_id: "ghost-id".into(),
            })
            .await
            .unwrap_err();
        match err {
            SwitchError::NotFound(s) => assert_eq!(s, "ghost-id"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn list_user_projects_aggregates_across_clouds() {
        let resolver = ContextTargetResolver::new(
            clouds("devstack", &["devstack", "prod"]),
            directory(&[
                (
                    "devstack",
                    vec![candidate("devstack", "admin", "d-1", "default")],
                ),
                (
                    "prod",
                    vec![
                        candidate("prod", "admin", "p-1", "default"),
                        candidate("prod", "demo", "p-2", "default"),
                    ],
                ),
            ]),
        );
        let projects = resolver.list_user_projects().await.unwrap();
        assert_eq!(projects.len(), 3);
    }

    #[tokio::test]
    async fn list_user_projects_surfaces_directory_error() {
        let clouds = clouds("devstack", &["devstack"]);
        let dir = Arc::new(
            FakeDirectory::with(std::collections::HashMap::new())
                .fail_with(SwitchError::RescopeRejected("no token".into())),
        );
        let resolver = ContextTargetResolver::new(clouds, dir);
        let err = resolver.list_user_projects().await.unwrap_err();
        assert!(matches!(err, SwitchError::RescopeRejected(_)));
    }
}
