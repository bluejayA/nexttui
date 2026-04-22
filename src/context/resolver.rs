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
use tracing::Instrument;

use super::error::SwitchError;
use super::types::{ContextRequest, ContextTarget};
use crate::adapter::auth::DomainNameResolver;
use crate::port::scoped_auth::ScopedAuthPort;

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
    /// Returns the runtime-switch default project for the given cloud, if
    /// configured. BL-P2-074: `:switch-cloud <name>` resolves via this.
    /// Intentionally **not** `auth.project_name` (Keystone bootstrap scope)
    /// — see application-design D3.
    fn default_project(&self, cloud: &str) -> Option<String>;

    /// Check whether a cloud is known without allocating a `Vec<String>`.
    /// Default impl delegates to `known_clouds()` for back-compat;
    /// implementations with a map-like backing store (e.g.
    /// [`ConfigCloudDirectory`]) should override for O(1) lookup.
    fn contains_cloud(&self, name: &str) -> bool {
        self.known_clouds().iter().any(|c| c == name)
    }
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
    scoped_auth: Option<Arc<dyn ScopedAuthPort>>,
    domain_resolver: Option<Arc<DomainNameResolver>>,
}

impl ContextTargetResolver {
    pub fn new(
        clouds: Arc<dyn CloudDirectory>,
        directory: Arc<dyn ProjectDirectoryPort>,
        scoped_auth: Option<Arc<dyn ScopedAuthPort>>,
        domain_resolver: Option<Arc<DomainNameResolver>>,
    ) -> Self {
        Self {
            clouds,
            directory,
            scoped_auth,
            domain_resolver,
        }
    }

    /// Resolve a request to its authoritative target. Performs cloud
    /// resolution, inline `cloud/project` parsing, and disambiguation
    /// against [`ProjectDirectoryPort`].
    pub async fn resolve(&self, request: ContextRequest) -> Result<ContextTarget, SwitchError> {
        match request {
            ContextRequest::ByName {
                cloud,
                project,
                domain,
            } => self.resolve_by_name_inner(cloud, project, domain).await,
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
            ContextRequest::CloudOnly { cloud } => {
                // BL-P2-074: delegate to default_project + project disambiguation.
                // - Span attached via `Instrument` (not `.enter()`) so it does
                //   not leak across `.await` points on a Tokio worker.
                // - Skips `normalize_cloud_project`: cloud is already authoritative
                //   and `default_project` values must be passed through verbatim
                //   (they may contain `/`, e.g. `team/foo` hierarchical names).
                let span = tracing::info_span!("resolve_cloud_only", cloud = %cloud);
                async move {
                    self.validate_cloud(&cloud)?;
                    let project = self.clouds.default_project(&cloud).ok_or_else(|| {
                        tracing::warn!(cloud = %cloud, "cloud_no_default_project");
                        SwitchError::NotConfigured {
                            cloud: cloud.clone(),
                        }
                    })?;
                    tracing::debug!(resolved_project = %project, "cloud_only_resolved");
                    self.disambiguate_by_name(cloud, project, None).await
                }
                .instrument(span)
                .await
            }
        }
    }

    /// Shared `ByName` entry point: normalizes the cloud prefix then
    /// disambiguates. Kept so the external `ByName` arm still runs the
    /// inline `cloud/project` parsing that BL-P2-031 introduced.
    async fn resolve_by_name_inner(
        &self,
        cloud: Option<String>,
        project: String,
        domain: Option<String>,
    ) -> Result<ContextTarget, SwitchError> {
        let (cloud, project_name) = normalize_cloud_project(cloud, project, &*self.clouds)?;
        self.disambiguate_by_name(cloud, project_name, domain).await
    }

    /// Disambiguation body only — takes an already-resolved cloud and a
    /// verbatim project name. `CloudOnly` calls this directly so a
    /// `default_project` of e.g. `"team/foo"` is not re-split on `/` by
    /// [`normalize_cloud_project`] (BL-P2-074 Codex P2 #2).
    async fn disambiguate_by_name(
        &self,
        cloud: String,
        project_name: String,
        domain: Option<String>,
    ) -> Result<ContextTarget, SwitchError> {
        let candidates = self.directory.list_projects(&cloud).await?;

        // --- Primary filter: exact project_name + domain match ---
        // Collect all name-matched candidates first (needed for fallback too).
        let name_matched: Vec<ProjectCandidate> = candidates
            .into_iter()
            .filter(|c| c.project_name == project_name)
            .collect();

        let primary: Vec<&ProjectCandidate> = name_matched
            .iter()
            .filter(|c| match &domain {
                Some(d) => &c.domain == d,
                None => true,
            })
            .collect();

        // If primary found at least one candidate, use it — no fallback needed.
        if !primary.is_empty() {
            return match primary.len() {
                1 => Ok(primary[0].clone().into()),
                _ => {
                    let ambiguous: Vec<ContextTarget> =
                        primary.into_iter().cloned().map(Into::into).collect();
                    Err(SwitchError::Ambiguous {
                        candidates: ambiguous,
                    })
                }
            };
        }

        // Primary returned zero candidates. Three cases:
        //   A. domain == None: name itself not found.
        //   B. domain == Some but no resolver: NotFound.
        //   C. domain == Some and resolver available: try domain_id fallback.

        if let Some(d) = &domain
            && let (Some(scoped_auth), Some(resolver)) =
                (&self.scoped_auth, &self.domain_resolver)
        {
            // --- Fallback: domain_id → name lazy resolution (FR-5 D4) ---
            // A valid token is required for the X-Auth-Token header. If none is
            // available, skip the HTTP fallback and return NotFound immediately
            // to avoid an empty-string token being sent to Keystone.
            let Some(token) = scoped_auth.current_token() else {
                return Err(SwitchError::NotFound(project_name));
            };
            let token_id = token.id;

            let mut resolved: Vec<ProjectCandidate> = Vec::new();
            for candidate in &name_matched {
                if candidate.domain.is_empty() {
                    continue;
                }
                match resolver
                    .resolve_name(&cloud, &candidate.domain, &token_id)
                    .await
                {
                    Ok(resolved_name) => {
                        tracing::info!(
                            cloud = %cloud,
                            domain_id = %candidate.domain,
                            "domain_lazy_resolve_trigger"
                        );
                        if resolved_name == *d {
                            resolved.push(candidate.clone());
                        }
                    }
                    Err(_) => {
                        // Resolution failed — skip this candidate silently
                    }
                }
            }

            let mut resolved_iter = resolved.into_iter();
            return match (resolved_iter.next(), resolved_iter.next()) {
                (None, _) => Err(SwitchError::NotFound(project_name)),
                (Some(only), None) => Ok(only.into()),
                (Some(first), Some(second)) => {
                    let mut ambiguous: Vec<ContextTarget> =
                        vec![first.into(), second.into()];
                    ambiguous.extend(resolved_iter.map(Into::into));
                    Err(SwitchError::Ambiguous {
                        candidates: ambiguous,
                    })
                }
            };
        }

        // No fallback available (domain == None or resolvers not wired) → NotFound.
        Err(SwitchError::NotFound(project_name))
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
        if self.clouds.contains_cloud(cloud) {
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
    if !clouds.contains_cloud(&cloud) {
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
        defaults: std::collections::HashMap<String, String>,
    }

    impl CloudDirectory for FakeClouds {
        fn active_cloud(&self) -> String {
            self.active.clone()
        }
        fn known_clouds(&self) -> Vec<String> {
            self.known.clone()
        }
        fn default_project(&self, cloud: &str) -> Option<String> {
            self.defaults.get(cloud).cloned()
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
            defaults: std::collections::HashMap::new(),
        })
    }

    fn clouds_with_defaults(
        active: &str,
        known: &[&str],
        defaults: &[(&str, &str)],
    ) -> Arc<dyn CloudDirectory> {
        Arc::new(FakeClouds {
            active: active.into(),
            known: known.iter().map(|s| s.to_string()).collect(),
            defaults: defaults
                .iter()
                .map(|(c, p)| ((*c).to_string(), (*p).to_string()))
                .collect(),
        })
    }

    fn directory(data: &[(&str, Vec<ProjectCandidate>)]) -> Arc<dyn ProjectDirectoryPort> {
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
            None,
            None,
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
            None,
            None,
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
            None,
            None,
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
            None,
            None,
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
                (
                    "devstack",
                    vec![candidate("devstack", "admin", "d-1", "default")],
                ),
                ("prod", vec![candidate("prod", "admin", "p-1", "default")]),
            ]),
            None,
            None,
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
                (
                    "devstack",
                    vec![candidate("devstack", "admin", "d-1", "default")],
                ),
                ("prod", vec![candidate("prod", "admin", "p-1", "default")]),
            ]),
            None,
            None,
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
            None,
            None,
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
            None,
            None,
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
            None,
            None,
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
            None,
            None,
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
        let resolver = ContextTargetResolver::new(clouds, dir, None, None);
        let err = resolver.list_user_projects().await.unwrap_err();
        assert!(matches!(err, SwitchError::RescopeRejected(_)));
    }

    #[test]
    fn test_cloud_directory_default_project_reflects_config() {
        let clouds = clouds_with_defaults(
            "devstack",
            &["devstack", "prod"],
            &[("devstack", "admin"), ("prod", "my_workload")],
        );
        assert_eq!(clouds.default_project("devstack"), Some("admin".into()));
        assert_eq!(clouds.default_project("prod"), Some("my_workload".into()));
        assert_eq!(clouds.default_project("unknown"), None);
    }

    #[tokio::test]
    async fn test_resolve_cloud_only_returns_default_project_target() {
        let resolver = ContextTargetResolver::new(
            clouds_with_defaults("devstack", &["devstack"], &[("devstack", "my_workload")]),
            directory(&[(
                "devstack",
                vec![candidate("devstack", "my_workload", "id-1", "default")],
            )]),
            None,
            None,
        );
        let target = resolver
            .resolve(ContextRequest::CloudOnly {
                cloud: "devstack".into(),
            })
            .await
            .unwrap();
        assert_eq!(target.cloud, "devstack");
        assert_eq!(target.project_name, "my_workload");
        assert_eq!(target.project_id, "id-1");
    }

    #[tokio::test]
    async fn test_resolve_cloud_only_unknown_cloud_returns_not_found() {
        let resolver = ContextTargetResolver::new(
            clouds_with_defaults("devstack", &["devstack"], &[("devstack", "admin")]),
            directory(&[(
                "devstack",
                vec![candidate("devstack", "admin", "id-1", "default")],
            )]),
            None,
            None,
        );
        let err = resolver
            .resolve(ContextRequest::CloudOnly {
                cloud: "nope".into(),
            })
            .await
            .unwrap_err();
        match err {
            SwitchError::NotFound(s) => assert!(s.contains("cloud 'nope'")),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_resolve_cloud_only_no_default_returns_not_configured() {
        let resolver = ContextTargetResolver::new(
            clouds("devstack", &["devstack"]),
            directory(&[(
                "devstack",
                vec![candidate("devstack", "admin", "id-1", "default")],
            )]),
            None,
            None,
        );
        let err = resolver
            .resolve(ContextRequest::CloudOnly {
                cloud: "devstack".into(),
            })
            .await
            .unwrap_err();
        match err {
            SwitchError::NotConfigured { cloud } => assert_eq!(cloud, "devstack"),
            other => panic!("expected NotConfigured, got {other:?}"),
        }
    }

    /// Codex P2 #2: `default_project` values must be passed through
    /// verbatim. A hierarchical name like `team/foo` must NOT be split
    /// on `/` as if it were inline `cloud/project` syntax.
    #[tokio::test]
    async fn test_resolve_cloud_only_preserves_slash_in_default_project() {
        let resolver = ContextTargetResolver::new(
            clouds_with_defaults("devstack", &["devstack"], &[("devstack", "team/foo")]),
            directory(&[(
                "devstack",
                vec![candidate("devstack", "team/foo", "id-1", "default")],
            )]),
            None,
            None,
        );
        let target = resolver
            .resolve(ContextRequest::CloudOnly {
                cloud: "devstack".into(),
            })
            .await
            .expect("default project 'team/foo' must resolve, not be split on '/'");
        assert_eq!(target.project_name, "team/foo");
        assert_eq!(target.project_id, "id-1");
    }

    #[tokio::test]
    async fn test_resolve_cloud_only_stale_default_returns_not_found() {
        let resolver = ContextTargetResolver::new(
            clouds_with_defaults("devstack", &["devstack"], &[("devstack", "ghost")]),
            directory(&[("devstack", vec![])]),
            None,
            None,
        );
        let err = resolver
            .resolve(ContextRequest::CloudOnly {
                cloud: "devstack".into(),
            })
            .await
            .unwrap_err();
        match err {
            SwitchError::NotFound(s) => assert_eq!(s, "ghost"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // BL-P2-080: domain_lazy_resolve fallback tests
    // -----------------------------------------------------------------------

    use crate::adapter::auth::DomainNameResolver;
    use crate::port::scoped_auth::ScopedAuthPort;
    use crate::port::types::{Token, TokenScope};
    use async_trait::async_trait as async_trait_inner;

    struct FakeScopedAuth {
        token_id: String,
    }

    #[async_trait_inner]
    impl ScopedAuthPort for FakeScopedAuth {
        fn current_scope(&self) -> TokenScope {
            TokenScope::Project {
                name: "admin".into(),
                domain: "default".into(),
            }
        }
        fn current_token(&self) -> Option<Token> {
            use chrono::{TimeZone, Utc};
            use crate::port::types::{CatalogEntry, ProjectScope};
            Some(Token {
                id: self.token_id.clone(),
                expires_at: Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap(),
                project: ProjectScope {
                    id: "p-id".into(),
                    name: "admin".into(),
                    domain_id: "default-id".into(),
                    domain_name: "default".into(),
                },
                roles: Vec::new(),
                catalog: Vec::<CatalogEntry>::new(),
            })
        }
        async fn set_active(
            &self,
            _scope: TokenScope,
            _token: Token,
        ) -> Result<(), SwitchError> {
            Ok(())
        }
    }

    fn make_domain_resolver_with_cache(
        cloud: &str,
        domain_id: &str,
        domain_name: &str,
    ) -> Arc<DomainNameResolver> {
        use std::io::Write;
        use tempfile::NamedTempFile;
        let mut f = NamedTempFile::with_suffix(".yaml").unwrap();
        let yaml = format!(
            "clouds:\n  {cloud}:\n    auth:\n      auth_url: http://127.0.0.1:1/v3\n      username: admin\n      password: secret\n"
        );
        f.write_all(yaml.as_bytes()).unwrap();
        let cfg = crate::config::Config::load_from(f.path()).unwrap();
        let resolver = DomainNameResolver::new(
            Arc::new(reqwest::Client::new()),
            Arc::new(cfg),
            std::time::Duration::from_secs(60),
        );
        // Seed the cache via the test helper so no HTTP is needed
        resolver.seed_cache(cloud, domain_id, domain_name);
        Arc::new(resolver)
    }

    /// When domain_resolver is None, existing behaviour is preserved.
    #[tokio::test]
    async fn disambiguate_by_name_domain_match_no_fallback_when_no_resolver() {
        // domain filter "Default" but candidate has "default" — no resolver, returns NotFound
        let resolver = ContextTargetResolver::new(
            clouds("devstack", &["devstack"]),
            directory(&[(
                "devstack",
                vec![candidate("devstack", "admin", "id-1", "did-default")],
            )]),
            None,
            None,
        );
        // "Default" != "did-default" and no resolver → NotFound
        let err = resolver
            .resolve(ContextRequest::ByName {
                cloud: None,
                project: "admin".into(),
                domain: Some("Default".into()),
            })
            .await
            .unwrap_err();
        assert!(
            matches!(err, SwitchError::NotFound(_)),
            "expected NotFound, got {err:?}"
        );
    }

    /// With domain_resolver injected, 1st-pass 0-match triggers fallback
    /// that resolves domain_id → name and finds the candidate.
    #[tokio::test]
    async fn disambiguate_by_name_fallback_resolves_domain_id_to_name_and_matches() {
        let scoped_auth: Arc<dyn ScopedAuthPort> = Arc::new(FakeScopedAuth {
            token_id: "tok-123".into(),
        });
        let domain_res = make_domain_resolver_with_cache("devstack", "did-default", "Default");

        let resolver = ContextTargetResolver::new(
            clouds("devstack", &["devstack"]),
            directory(&[(
                "devstack",
                // domain field is a UUID-like domain_id, not a name
                vec![candidate("devstack", "admin", "id-1", "did-default")],
            )]),
            Some(scoped_auth),
            Some(domain_res),
        );
        let target = resolver
            .resolve(ContextRequest::ByName {
                cloud: None,
                project: "admin".into(),
                domain: Some("Default".into()),
            })
            .await
            .expect("fallback should resolve domain_id to name and match");
        assert_eq!(target.project_id, "id-1");
        assert_eq!(target.domain, "did-default");
    }

    /// Fallback is invoked but the resolved name still doesn't match → NotFound.
    #[tokio::test]
    async fn disambiguate_by_name_fallback_fails_returns_notfound() {
        let scoped_auth: Arc<dyn ScopedAuthPort> = Arc::new(FakeScopedAuth {
            token_id: "tok-xyz".into(),
        });
        // Resolver maps "did-other" → "OtherDomain", not "Default"
        let domain_res = make_domain_resolver_with_cache("devstack", "did-other", "OtherDomain");

        let resolver = ContextTargetResolver::new(
            clouds("devstack", &["devstack"]),
            directory(&[(
                "devstack",
                vec![candidate("devstack", "admin", "id-1", "did-other")],
            )]),
            Some(scoped_auth),
            Some(domain_res),
        );
        let err = resolver
            .resolve(ContextRequest::ByName {
                cloud: None,
                project: "admin".into(),
                domain: Some("Default".into()),
            })
            .await
            .unwrap_err();
        assert!(
            matches!(err, SwitchError::NotFound(_)),
            "expected NotFound after fallback mismatch, got {err:?}"
        );
    }

    /// When current_token() returns None, fallback returns NotFound immediately
    /// without making any HTTP calls (domain_resolver must NOT be invoked).
    #[tokio::test]
    async fn disambiguate_by_name_fallback_no_token_returns_notfound() {
        struct NoTokenAuth;

        #[async_trait_inner]
        impl ScopedAuthPort for NoTokenAuth {
            fn current_scope(&self) -> TokenScope {
                TokenScope::Project {
                    name: "admin".into(),
                    domain: "default".into(),
                }
            }
            fn current_token(&self) -> Option<Token> {
                None // ← simulates missing token
            }
            async fn set_active(
                &self,
                _scope: TokenScope,
                _token: Token,
            ) -> Result<(), SwitchError> {
                Ok(())
            }
        }

        // The domain resolver is wired but should never be called (port 1 = unreachable).
        let domain_res = make_domain_resolver_with_cache("devstack", "did-default", "Default");
        // Seed an entry but port 1 is unreachable — if HTTP is attempted the test panics or errors.
        let resolver = ContextTargetResolver::new(
            clouds("devstack", &["devstack"]),
            directory(&[(
                "devstack",
                // 1st-pass: domain field "did-default" != "Default" → no match
                vec![candidate("devstack", "admin", "id-1", "did-default")],
            )]),
            Some(Arc::new(NoTokenAuth) as Arc<dyn ScopedAuthPort>),
            Some(domain_res),
        );

        let err = resolver
            .resolve(ContextRequest::ByName {
                cloud: None,
                project: "admin".into(),
                domain: Some("Default".into()),
            })
            .await
            .unwrap_err();
        assert!(
            matches!(err, SwitchError::NotFound(_)),
            "expected NotFound when token is None, got {err:?}"
        );
    }

    /// When domain is None, fallback is never triggered.
    #[tokio::test]
    async fn disambiguate_by_name_no_domain_specified_skips_fallback() {
        // If fallback were called, it would panic (unreachable server).
        // Since domain=None, fallback must not be triggered.
        let scoped_auth: Arc<dyn ScopedAuthPort> = Arc::new(FakeScopedAuth {
            token_id: "tok-abc".into(),
        });
        let domain_res = make_domain_resolver_with_cache("devstack", "did-x", "SomeDomain");

        let resolver = ContextTargetResolver::new(
            clouds("devstack", &["devstack"]),
            directory(&[(
                "devstack",
                vec![candidate("devstack", "admin", "id-1", "did-x")],
            )]),
            Some(scoped_auth),
            Some(domain_res),
        );
        // domain=None → no fallback → plain name match succeeds
        let target = resolver
            .resolve(ContextRequest::ByName {
                cloud: None,
                project: "admin".into(),
                domain: None,
            })
            .await
            .expect("domain=None should use plain name match");
        assert_eq!(target.project_id, "id-1");
    }
}
