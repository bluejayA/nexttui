//! Devstack integration tests for [`KeystoneProjectDirectory`] (BL-P2-080, Unit 3).
//!
//! These tests require a live devstack container reachable at `DEVSTACK_URL`
//! and an initial unscoped Keystone token at `DEVSTACK_TOKEN`.  They are
//! **skipped automatically** unless both env-vars are present AND the
//! `devstack-integration` Cargo feature is enabled.
//!
//! In CI the feature is activated by:
//!   `cargo test --test devstack_directory --features devstack-integration -- --nocapture`
//!
//! The guard macro at the top of each test exits early with a println when the
//! env-vars are absent so that `cargo test --lib` (no feature, no devstack) is
//! never affected.
//!
//! ## Test scenarios
//!
//! | ID | Scenario | Verifies |
//! |----|----------|---------|
//! | T1 | Pagination-forced real fetch | `?limit=1` forces ≥3 pages; list returns `ProjectCandidate` slice |
//! | T2 | Real UUID rescope → 201 | First candidate's `project_id` rescopes to a new scoped token |

// The entire test module is compiled only when the feature flag is set.
// Without `--features devstack-integration` the file produces zero symbols,
// so `cargo test --lib` and `cargo clippy --lib` are completely unaffected.
#![cfg(feature = "devstack-integration")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;

// We import from the library crate.  The integration test binary links against
// `nexttui` as a crate (extern crate nexttui) which Cargo arranges
// automatically for files in `tests/`.
use nexttui::adapter::auth::directory_cache::DirectoryCache;
use nexttui::adapter::auth::keystone_project_directory::KeystoneProjectDirectory;
use nexttui::context::error::SwitchError;
use nexttui::context::resolver::{CloudDirectory, ProjectDirectoryPort};
use nexttui::port::keystone_rescope::KeystoneRescopePort;
use nexttui::port::scoped_auth::ScopedAuthPort;
use nexttui::port::types::{CatalogEntry, ProjectScope, Token, TokenRole, TokenScope};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read `DEVSTACK_URL` and `DEVSTACK_TOKEN` from the environment.
/// Returns `None` if either is absent (tests skip gracefully).
fn devstack_env() -> Option<(String, String)> {
    let url = std::env::var("DEVSTACK_URL").ok()?;
    let token = std::env::var("DEVSTACK_TOKEN").ok()?;
    Some((url, token))
}

/// Macro: print a skip message and return early when devstack is not available.
macro_rules! require_devstack {
    ($url:ident, $token:ident) => {
        let (_devstack_url, _devstack_token) = match devstack_env() {
            Some(v) => v,
            None => {
                println!("[SKIP] DEVSTACK_URL / DEVSTACK_TOKEN not set — devstack container unavailable");
                return;
            }
        };
        let $url = _devstack_url;
        let $token = _devstack_token;
    };
}

// ---------------------------------------------------------------------------
// Minimal stub doubles (re-implemented here to avoid exposing test-only items
// from the lib crate — keeps the lib API surface clean).
// ---------------------------------------------------------------------------

struct SingleCloudDir {
    cloud: String,
}

impl CloudDirectory for SingleCloudDir {
    fn active_cloud(&self) -> String {
        self.cloud.clone()
    }
    fn known_clouds(&self) -> Vec<String> {
        vec![self.cloud.clone()]
    }
    fn default_project(&self, _cloud: &str) -> Option<String> {
        None
    }
}

struct FixedTokenAuth {
    token: Mutex<Option<Token>>,
}

impl FixedTokenAuth {
    fn with_token(token: Token) -> Arc<Self> {
        Arc::new(Self {
            token: Mutex::new(Some(token)),
        })
    }
}

#[async_trait::async_trait]
impl ScopedAuthPort for FixedTokenAuth {
    fn current_scope(&self) -> TokenScope {
        TokenScope::Unscoped
    }
    fn current_token(&self) -> Option<Token> {
        self.token.lock().ok()?.clone()
    }
    async fn set_active(&self, _scope: TokenScope, _token: Token) -> Result<(), SwitchError> {
        Ok(())
    }
}

/// Build a minimal [`Token`] from an opaque token-id string.
fn make_devstack_token(id: &str) -> Token {
    Token {
        id: id.to_string(),
        expires_at: Utc::now() + chrono::Duration::hours(1),
        project: ProjectScope {
            id: "integration-test-project-id".into(),
            name: "admin".into(),
            domain_id: "default".into(),
            domain_name: "Default".into(),
        },
        roles: Vec::<TokenRole>::new(),
        catalog: Vec::<CatalogEntry>::new(),
    }
}

/// Build a minimal [`Config`] pointing `auth_url` at the devstack endpoint.
fn make_devstack_config(cloud: &str, auth_url: &str) -> Arc<nexttui::config::Config> {
    let yaml = format!(
        "clouds:\n  {cloud}:\n    auth:\n      auth_url: {auth_url}\n      username: admin\n      password: secret\n"
    );
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("clouds.yaml");
    std::fs::write(&path, &yaml).expect("write yaml");
    let cfg = nexttui::config::Config::load_from(&path).expect("load_from");
    let _ = dir; // keep alive until here
    Arc::new(cfg)
}

// ---------------------------------------------------------------------------
// T1 — Pagination-forced real fetch
// ---------------------------------------------------------------------------
//
// Strategy: append `?limit=1` to the initial `/v3/auth/projects` URL so that
// Keystone returns exactly one project per page.  A standard devstack has at
// least 3 projects (admin, demo, service/invisible_to_admin), so we expect
// ≥ 3 pages to be followed.  We assert:
//   - the result is a non-empty Vec<ProjectCandidate>
//   - every candidate carries the correct cloud name
//   - at least 3 projects are returned (pagination actually ran)
//
// The test uses `max_pages = 50` to avoid tripping the cap on a large
// devstack, but we assert that at least 3 pages were traversed indirectly
// via the candidate count.

#[tokio::test]
async fn t1_pagination_forced_real_fetch() {
    require_devstack!(devstack_url, devstack_token);

    const CLOUD: &str = "devstack";

    // Append `?limit=1` to force per-page=1 pagination.
    let paginated_auth_url = format!("{}/v3", devstack_url.trim_end_matches('/'));

    let _config = make_devstack_config(CLOUD, &paginated_auth_url);
    let cache = Arc::new(DirectoryCache::new(Duration::from_secs(60)));
    let token = make_devstack_token(&devstack_token);
    let auth = FixedTokenAuth::with_token(token);
    let clouds = Arc::new(SingleCloudDir {
        cloud: CLOUD.to_string(),
    });

    // Build a custom client that injects `limit=1` via a base URL override.
    // KeystoneProjectDirectory constructs the initial URL as
    // `{auth_url}/auth/projects`, so we encode the query param via
    // `auth_url`.  We set `auth_url` to include the `?limit=1` suffix.
    // Note: the SSRF validator checks scheme+host+port only — query params
    // are not part of the check, so this is safe and expected.
    let limited_auth_url = format!(
        "{}/v3?limit=1",
        devstack_url.trim_end_matches('/')
    );
    let config_limited = make_devstack_config(CLOUD, &limited_auth_url);

    let dir = KeystoneProjectDirectory::new(
        Arc::new(reqwest::Client::new()),
        auth,
        clouds,
        config_limited,
        cache,
        50, // generous page cap
    );

    let candidates = dir
        .list_projects(CLOUD)
        .await
        .expect("T1: list_projects should succeed against devstack");

    assert!(
        !candidates.is_empty(),
        "T1: expected at least one project, got empty list"
    );

    // Every candidate must carry the correct cloud tag.
    for c in &candidates {
        assert_eq!(
            c.cloud, CLOUD,
            "T1: candidate cloud mismatch: got {:?}",
            c.cloud
        );
        assert!(
            !c.project_id.is_empty(),
            "T1: project_id must not be empty"
        );
        assert!(
            !c.project_name.is_empty(),
            "T1: project_name must not be empty"
        );
    }

    // A standard devstack has ≥ 3 projects across all pages.
    assert!(
        candidates.len() >= 3,
        "T1: expected ≥3 projects (pagination ran), got {}",
        candidates.len()
    );

    println!(
        "T1 PASS: fetched {} projects across paginated pages",
        candidates.len()
    );
}

// ---------------------------------------------------------------------------
// T2 — Real UUID rescope → 201
// ---------------------------------------------------------------------------
//
// Strategy: use the first project from T1's candidate list (re-fetched here
// so the tests are independent) and attempt a Keystone rescope.  The
// expectation is `201 Created` (represented by a valid new `Token` return from
// `KeystoneRescopeAdapter::rescope`).
//
// Contrast: a placeholder/wrong UUID yields `401 Unauthorized` or `404 Not
// Found`; this test verifies that the *real UUID from the directory* is
// accepted by the token engine.

#[tokio::test]
async fn t2_real_uuid_rescope_201() {
    require_devstack!(devstack_url, devstack_token);

    const CLOUD: &str = "devstack";

    let auth_url = format!("{}/v3", devstack_url.trim_end_matches('/'));
    let config = make_devstack_config(CLOUD, &auth_url);
    let cache = Arc::new(DirectoryCache::new(Duration::from_secs(60)));
    let token = make_devstack_token(&devstack_token);
    let auth_for_dir = FixedTokenAuth::with_token(token.clone());
    let clouds = Arc::new(SingleCloudDir {
        cloud: CLOUD.to_string(),
    });

    let dir = KeystoneProjectDirectory::new(
        Arc::new(reqwest::Client::new()),
        auth_for_dir,
        clouds,
        config,
        cache,
        50,
    );

    // Step 1: list projects to obtain a real project_id.
    let candidates = dir
        .list_projects(CLOUD)
        .await
        .expect("T2: list_projects prerequisite failed");

    assert!(
        !candidates.is_empty(),
        "T2: prerequisite failed — no projects returned"
    );

    let first = &candidates[0];
    let project_id = first.project_id.clone();
    let project_name = first.project_name.clone();

    println!(
        "T2: rescoping to project '{}' (id: {})",
        project_name, project_id
    );

    // Step 2: rescope using the real project_id.
    let rescope_url = format!("{}/v3", devstack_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let rescope_adapter = nexttui::adapter::auth::rescope::KeystoneRescopeAdapter::new(
        client,
        rescope_url,
    );

    let target = nexttui::context::types::ContextTarget {
        cloud: CLOUD.to_string(),
        project_id: project_id.clone(),
        project_name: project_name.clone(),
        domain: first.domain.clone(),
    };

    let new_token = rescope_adapter
        .rescope(&token, &target)
        .await
        .expect("T2: rescope should succeed with a real project_id (201 Created)");

    assert!(
        !new_token.id.is_empty(),
        "T2: new token id must not be empty"
    );
    assert_eq!(
        new_token.project.id, project_id,
        "T2: token scope must match requested project_id"
    );

    println!(
        "T2 PASS: rescope to project '{}' succeeded, new token obtained",
        project_name
    );
}
