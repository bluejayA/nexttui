//! HTTP-backed [`ProjectDirectoryPort`] via Keystone `/v3/auth/projects` (BL-P2-080).

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use tracing::{debug, error, info, warn};

use crate::adapter::auth::directory_cache::DirectoryCache;
use crate::adapter::auth::rescope::sanitize_rescope_body;
use crate::adapter::auth::token_scope_fingerprint::TokenScopeFingerprint;
use crate::config::Config;
use crate::context::error::SwitchError;
use crate::context::resolver::{CloudDirectory, ProjectCandidate, ProjectDirectoryPort};
use crate::port::error::ApiError;
use crate::port::scoped_auth::ScopedAuthPort;

// --- Keystone /v3/auth/projects response types ---

#[derive(Debug, Deserialize)]
struct ProjectsResponse {
    projects: Vec<KeystoneProject>,
    #[serde(default)]
    links: ResponseLinks,
}

#[derive(Debug, Deserialize)]
struct KeystoneProject {
    id: String,
    name: String,
    domain_id: String,
}

#[derive(Debug, Default, Deserialize)]
struct ResponseLinks {
    next: Option<String>,
}

/// HTTP-backed implementation of [`ProjectDirectoryPort`] that fetches the
/// list of projects a token can reach from Keystone `/v3/auth/projects` with
/// pagination support.
pub struct KeystoneProjectDirectory {
    client: Arc<reqwest::Client>,
    scoped_auth: Arc<dyn ScopedAuthPort>,
    clouds: Arc<dyn CloudDirectory>,
    config: Arc<Config>,
    cache: Arc<DirectoryCache>,
    max_pages: usize,
}

impl KeystoneProjectDirectory {
    pub fn new(
        client: Arc<reqwest::Client>,
        scoped_auth: Arc<dyn ScopedAuthPort>,
        clouds: Arc<dyn CloudDirectory>,
        config: Arc<Config>,
        cache: Arc<DirectoryCache>,
        max_pages: usize,
    ) -> Self {
        Self {
            client,
            scoped_auth,
            clouds,
            config,
            cache,
            max_pages,
        }
    }

    /// Validate that `next_url` is safe to follow: must be http/https and must
    /// share the same host+port as `auth_url` (the configured Keystone endpoint).
    /// Rejects anything else to prevent SSRF via a malicious `links.next` value.
    fn validate_next_url(next_url: &str, auth_url: &str) -> Result<(), SwitchError> {
        let parsed = reqwest::Url::parse(next_url).map_err(|_| {
            SwitchError::Api(ApiError::Parse(
                "pagination next URL is not a valid URL".into(),
            ))
        })?;

        // Only allow http / https — reject file://, data://, etc.
        if parsed.scheme() != "http" && parsed.scheme() != "https" {
            warn!(next_url, "directory_next_url_rejected");
            return Err(SwitchError::Api(ApiError::Parse(
                "pagination next URL scheme/host mismatch".into(),
            )));
        }

        let allowed = reqwest::Url::parse(auth_url).map_err(|_| {
            SwitchError::Api(ApiError::Parse("auth_url is not a valid URL".into()))
        })?;

        let next_host = parsed.host_str().unwrap_or("");
        let allowed_host = allowed.host_str().unwrap_or("");
        let next_port = parsed.port_or_known_default();
        let allowed_port = allowed.port_or_known_default();

        if next_host != allowed_host || next_port != allowed_port {
            warn!(next_url, "directory_next_url_rejected");
            return Err(SwitchError::Api(ApiError::Parse(
                "pagination next URL scheme/host mismatch".into(),
            )));
        }

        Ok(())
    }

    /// Fetch all pages from Keystone starting at `initial_url`.
    async fn fetch_all_pages(
        &self,
        initial_url: String,
        token_id: &str,
        max_pages: usize,
        auth_url: &str,
    ) -> Result<(Vec<KeystoneProject>, usize), SwitchError> {
        let mut all_projects: Vec<KeystoneProject> = Vec::new();
        let mut next_url: Option<String> = Some(initial_url);
        let mut page_count = 0usize;

        while let Some(url) = next_url.take() {
            if page_count >= max_pages {
                warn!(page_count, max_pages, "directory_pagination_runaway");
                return Err(SwitchError::Api(ApiError::Parse(format!(
                    "pagination runaway, >{max_pages} pages"
                ))));
            }

            let resp = self
                .client
                .get(&url)
                .header("X-Auth-Token", token_id)
                .send()
                .await
                .map_err(|e| SwitchError::Api(ApiError::Network(e)))?;

            let status = resp.status();
            if status.as_u16() == 401 {
                let body = sanitize_rescope_body(&resp.text().await.unwrap_or_default());
                warn!(status = 401, "directory_lookup_401");
                return Err(SwitchError::RescopeRejected(format!(
                    "directory lookup 401 — token rejected: {body}"
                )));
            }
            if !status.is_success() {
                let body = sanitize_rescope_body(&resp.text().await.unwrap_or_default());
                error!(status = status.as_u16(), "directory_lookup_http_error");
                return Err(SwitchError::Api(ApiError::ServiceUnavailable {
                    service: format!("keystone directory ({status}): {body}"),
                }));
            }

            let parsed: ProjectsResponse = resp.json().await.map_err(|e| {
                SwitchError::Api(ApiError::Parse(format!(
                    "failed to parse /v3/auth/projects response: {e}"
                )))
            })?;

            all_projects.extend(parsed.projects);

            // Validate next URL before following (SSRF guard)
            if let Some(candidate) = parsed.links.next.filter(|s| !s.is_empty()) {
                Self::validate_next_url(&candidate, auth_url)?;
                next_url = Some(candidate);
            }

            page_count += 1;
        }

        Ok((all_projects, page_count))
    }
}

#[async_trait]
impl ProjectDirectoryPort for KeystoneProjectDirectory {
    async fn list_projects(&self, cloud: &str) -> Result<Vec<ProjectCandidate>, SwitchError> {
        // Cross-cloud guard
        if cloud != self.clouds.active_cloud() {
            warn!(cloud, active = %self.clouds.active_cloud(), "cross_cloud_directory_rejected");
            return Err(SwitchError::Unsupported(
                "cross-cloud directory not supported — see BL-P2-081".into(),
            ));
        }

        // Get current token
        let token = self
            .scoped_auth
            .current_token()
            .ok_or_else(|| SwitchError::Unsupported("no active token for directory lookup".into()))?;

        // Compute fingerprint for cache keying
        let fp = TokenScopeFingerprint::new().compute(&token);

        // Cache hit?
        if let Some(cached) = self.cache.get(cloud, &fp) {
            debug!(cloud, fp, "directory_cache_hit");
            return Ok(cached);
        }

        // Get auth_url for this cloud
        let auth_url = self
            .config
            .cloud_config(cloud)
            .ok_or_else(|| SwitchError::Unsupported(format!("cloud '{cloud}' not found in config")))?
            .auth
            .auth_url
            .trim_end_matches('/')
            .to_string();

        let initial_url = format!("{auth_url}/auth/projects");

        // Fetch all pages
        let (raw, page_count) = self
            .fetch_all_pages(initial_url, &token.id, self.max_pages, &auth_url)
            .await?;

        let count = raw.len();

        let candidates: Vec<ProjectCandidate> = raw
            .into_iter()
            .map(|p| ProjectCandidate {
                cloud: cloud.to_string(),
                project_id: p.id,
                project_name: p.name,
                domain: p.domain_id,
            })
            .collect();

        info!(cloud, pages = page_count, count, "directory_fetched");

        self.cache.put(cloud, &fp, candidates.clone());
        Ok(candidates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use chrono::Utc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use crate::port::types::{CatalogEntry, ProjectScope, Token, TokenRole, TokenScope};

    // --- Minimal stub impls ---

    struct FakeCloudDir {
        active: String,
    }

    impl CloudDirectory for FakeCloudDir {
        fn active_cloud(&self) -> String {
            self.active.clone()
        }
        fn known_clouds(&self) -> Vec<String> {
            vec![self.active.clone()]
        }
        fn default_project(&self, _cloud: &str) -> Option<String> {
            None
        }
    }

    struct FakeScopedAuth {
        token: Mutex<Option<Token>>,
    }

    impl FakeScopedAuth {
        fn with_token(token: Token) -> Self {
            Self {
                token: Mutex::new(Some(token)),
            }
        }
        fn no_token() -> Self {
            Self {
                token: Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl ScopedAuthPort for FakeScopedAuth {
        fn current_scope(&self) -> TokenScope {
            TokenScope::Unscoped
        }
        fn current_token(&self) -> Option<Token> {
            self.token.lock().ok()?.clone()
        }
        async fn set_active(
            &self,
            _scope: TokenScope,
            _token: Token,
        ) -> Result<(), SwitchError> {
            Ok(())
        }
    }

    // --- Helpers ---

    fn make_token(id: &str) -> Token {
        Token {
            id: id.to_string(),
            expires_at: Utc::now(),
            project: ProjectScope {
                id: "proj-1".into(),
                name: "admin".into(),
                domain_id: "default".into(),
                domain_name: "Default".into(),
            },
            roles: Vec::<TokenRole>::new(),
            catalog: Vec::<CatalogEntry>::new(),
        }
    }

    fn make_config(cloud: &str, auth_url: &str) -> Arc<Config> {
        // load_from validates auth credentials; provide minimal valid password auth
        let yaml = format!(
            "clouds:\n  {cloud}:\n    auth:\n      auth_url: {auth_url}\n      username: admin\n      password: secret\n"
        );
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("clouds.yaml");
        std::fs::write(&path, &yaml).expect("write yaml");
        let cfg = Config::load_from(&path).expect("load_from");
        let _ = dir; // keep alive until here
        Arc::new(cfg)
    }

    // --- Mock HTTP server (one-shot per request) ---

    struct CannedResponse {
        status_line: &'static str,
        body: String,
    }

    async fn spawn_one_shot(resp: CannedResponse) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");
        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf).await.unwrap();
            let wire = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\n\r\n{body}",
                status = resp.status_line,
                len = resp.body.len(),
                body = resp.body,
            );
            stream.write_all(wire.as_bytes()).await.unwrap();
            let _ = stream.shutdown().await;
        });
        (base_url, handle)
    }

    fn single_project_body(next_url: Option<&str>) -> String {
        let next_json = match next_url {
            Some(u) => format!(r#""next": "{u}""#),
            None => r#""next": null"#.into(),
        };
        format!(
            r#"{{"projects":[{{"id":"p1","name":"admin","domain_id":"default"}}],"links":{{{next_json}}}}}"#
        )
    }

    // --- Tests ---

    #[tokio::test]
    async fn cross_cloud_rejected() {
        let config = make_config("devstack", "http://127.0.0.1:1/v3");
        let cache = Arc::new(DirectoryCache::new(Duration::from_secs(60)));
        let auth = Arc::new(FakeScopedAuth::with_token(make_token("tok")));
        let clouds = Arc::new(FakeCloudDir {
            active: "devstack".into(),
        });
        let dir = KeystoneProjectDirectory::new(
            Arc::new(reqwest::Client::new()),
            auth,
            clouds,
            config,
            cache,
            10,
        );

        let err = dir.list_projects("prod").await.unwrap_err();
        assert!(
            matches!(err, SwitchError::Unsupported(_)),
            "expected Unsupported, got {err:?}"
        );
    }

    #[tokio::test]
    async fn no_active_token_rejected() {
        let config = make_config("devstack", "http://127.0.0.1:1/v3");
        let cache = Arc::new(DirectoryCache::new(Duration::from_secs(60)));
        let auth = Arc::new(FakeScopedAuth::no_token());
        let clouds = Arc::new(FakeCloudDir {
            active: "devstack".into(),
        });
        let dir = KeystoneProjectDirectory::new(
            Arc::new(reqwest::Client::new()),
            auth,
            clouds,
            config,
            cache,
            10,
        );

        let err = dir.list_projects("devstack").await.unwrap_err();
        assert!(
            matches!(err, SwitchError::Unsupported(_)),
            "expected Unsupported, got {err:?}"
        );
    }

    #[tokio::test]
    async fn fetch_single_page_parses_candidates() {
        let body = single_project_body(None);
        let resp = CannedResponse {
            status_line: "200 OK",
            body,
        };
        let (base_url, _handle) = spawn_one_shot(resp).await;
        let config = make_config("devstack", &format!("{base_url}/v3"));
        let cache = Arc::new(DirectoryCache::new(Duration::from_secs(60)));
        let auth = Arc::new(FakeScopedAuth::with_token(make_token("tok-123")));
        let clouds = Arc::new(FakeCloudDir {
            active: "devstack".into(),
        });
        let dir = KeystoneProjectDirectory::new(
            Arc::new(reqwest::Client::new()),
            auth,
            clouds,
            config,
            cache,
            10,
        );

        let candidates = dir.list_projects("devstack").await.unwrap();
        assert_eq!(candidates.len(), 1);
        let c = &candidates[0];
        assert_eq!(c.project_id, "p1");
        assert_eq!(c.project_name, "admin");
        assert_eq!(c.domain, "default"); // domain = domain_id verbatim
        assert_eq!(c.cloud, "devstack");
    }

    #[tokio::test]
    async fn fetch_multi_page_follows_next_link() {
        // 3 pages, each with 1 project; page1 links to page2, page2 links to page3
        // We use a single TcpListener that accepts 3 connections
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");

        let page2_url = format!("{base}/v3/auth/projects?page=2");
        let page3_url = format!("{base}/v3/auth/projects?page=3");

        let bodies = vec![
            format!(r#"{{"projects":[{{"id":"p1","name":"proj1","domain_id":"d1"}}],"links":{{"next":"{page2_url}"}}}}"#),
            format!(r#"{{"projects":[{{"id":"p2","name":"proj2","domain_id":"d1"}}],"links":{{"next":"{page3_url}"}}}}"#),
            r#"{"projects":[{"id":"p3","name":"proj3","domain_id":"d1"}],"links":{"next":null}}"#.into(),
        ];

        let _handle = tokio::spawn(async move {
            for body in bodies {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buf = [0u8; 8192];
                let _ = stream.read(&mut buf).await.unwrap();
                let wire = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {len}\r\n\r\n{body}",
                    len = body.len(),
                );
                stream.write_all(wire.as_bytes()).await.unwrap();
                let _ = stream.shutdown().await;
            }
        });

        let config = make_config("devstack", &format!("{base}/v3"));
        let cache = Arc::new(DirectoryCache::new(Duration::from_secs(60)));
        let auth = Arc::new(FakeScopedAuth::with_token(make_token("tok-multi")));
        let clouds = Arc::new(FakeCloudDir {
            active: "devstack".into(),
        });
        let dir = KeystoneProjectDirectory::new(
            Arc::new(reqwest::Client::new()),
            auth,
            clouds,
            config,
            cache,
            10,
        );

        let candidates = dir.list_projects("devstack").await.unwrap();
        assert_eq!(candidates.len(), 3);
        let ids: Vec<&str> = candidates.iter().map(|c| c.project_id.as_str()).collect();
        assert!(ids.contains(&"p1"));
        assert!(ids.contains(&"p2"));
        assert!(ids.contains(&"p3"));
    }

    #[tokio::test]
    async fn max_pages_cap_trips_error() {
        // Server keeps returning a next link — cap at max_pages=2
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        let self_url = format!("{base}/v3/auth/projects");

        let _handle = tokio::spawn(async move {
            // Serve many responses all with self-referencing next
            for _ in 0..5 {
                let Ok((mut stream, _)) = listener.accept().await else { break };
                let mut buf = [0u8; 8192];
                let _ = stream.read(&mut buf).await.unwrap();
                let body = format!(
                    r#"{{"projects":[{{"id":"px","name":"p","domain_id":"d"}}],"links":{{"next":"{self_url}"}}}}"#
                );
                let wire = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(wire.as_bytes()).await.unwrap();
                let _ = stream.shutdown().await;
            }
        });

        let config = make_config("devstack", &format!("{base}/v3"));
        let cache = Arc::new(DirectoryCache::new(Duration::from_secs(60)));
        let auth = Arc::new(FakeScopedAuth::with_token(make_token("tok-cap")));
        let clouds = Arc::new(FakeCloudDir {
            active: "devstack".into(),
        });
        let dir = KeystoneProjectDirectory::new(
            Arc::new(reqwest::Client::new()),
            auth,
            clouds,
            config,
            cache,
            2, // cap at 2 pages
        );

        let err = dir.list_projects("devstack").await.unwrap_err();
        match err {
            SwitchError::Api(ApiError::Parse(msg)) => {
                assert!(msg.contains("pagination runaway"), "unexpected msg: {msg}");
            }
            other => panic!("expected Api(Parse) for runaway, got {other:?}"),
        }
    }

    // --- Security: body truncation + sanitization ---

    #[tokio::test]
    async fn http_5xx_body_is_truncated_and_sanitized() {
        // large body containing token-like header material
        let large_body = format!(
            "X-Auth-Token: gAAAALeakedTokenMaterial\n{}",
            "x".repeat(2048)
        );
        let resp = CannedResponse {
            status_line: "503 Service Unavailable",
            body: large_body,
        };
        let (base_url, _handle) = spawn_one_shot(resp).await;
        let config = make_config("devstack", &format!("{base_url}/v3"));
        let cache = Arc::new(DirectoryCache::new(Duration::from_secs(60)));
        let auth = Arc::new(FakeScopedAuth::with_token(make_token("tok-503")));
        let clouds = Arc::new(FakeCloudDir {
            active: "devstack".into(),
        });
        let dir = KeystoneProjectDirectory::new(
            Arc::new(reqwest::Client::new()),
            auth,
            clouds,
            config,
            cache,
            10,
        );

        let err = dir.list_projects("devstack").await.unwrap_err();
        let msg = err.to_string();
        // token material must not appear in the error message
        assert!(
            !msg.contains("gAAAALeakedTokenMaterial"),
            "token leaked in 5xx error: {msg}"
        );
        // message must be truncated (not more than ~400 chars)
        assert!(
            msg.chars().count() < 400,
            "5xx error message too long ({} chars): {msg}",
            msg.chars().count()
        );
    }

    #[tokio::test]
    async fn http_401_body_is_truncated_and_sanitized() {
        let large_body = format!(
            "X-Auth-Token: gAAAALeaked401Token\n{}",
            "y".repeat(2048)
        );
        let resp = CannedResponse {
            status_line: "401 Unauthorized",
            body: large_body,
        };
        let (base_url, _handle) = spawn_one_shot(resp).await;
        let config = make_config("devstack", &format!("{base_url}/v3"));
        let cache = Arc::new(DirectoryCache::new(Duration::from_secs(60)));
        let auth = Arc::new(FakeScopedAuth::with_token(make_token("tok-401-big")));
        let clouds = Arc::new(FakeCloudDir {
            active: "devstack".into(),
        });
        let dir = KeystoneProjectDirectory::new(
            Arc::new(reqwest::Client::new()),
            auth,
            clouds,
            config,
            cache,
            10,
        );

        let err = dir.list_projects("devstack").await.unwrap_err();
        let msg = err.to_string();
        assert!(
            !msg.contains("gAAAALeaked401Token"),
            "token leaked in 401 error: {msg}"
        );
        assert!(
            msg.chars().count() < 400,
            "401 error message too long ({} chars): {msg}",
            msg.chars().count()
        );
    }

    // --- Security: SSRF guard on links.next ---

    #[tokio::test]
    async fn links_next_rejects_different_host() {
        // Page 1 returns links.next pointing to IMDS (169.254.169.254) — SSRF candidate
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");

        let ssrf_url = "http://169.254.169.254/v3/auth/projects";
        let page1_body = format!(
            r#"{{"projects":[{{"id":"p1","name":"proj1","domain_id":"d1"}}],"links":{{"next":"{ssrf_url}"}}}}"#
        );

        let _handle = tokio::spawn(async move {
            // Serve only 1 response — if adapter follows the SSRF URL it would
            // try a different host, but we just verify it doesn't request again.
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf).await.unwrap();
            let wire = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{page1_body}",
                page1_body.len()
            );
            stream.write_all(wire.as_bytes()).await.unwrap();
            let _ = stream.shutdown().await;
        });

        let config = make_config("devstack", &format!("{base}/v3"));
        let cache = Arc::new(DirectoryCache::new(Duration::from_secs(60)));
        let auth = Arc::new(FakeScopedAuth::with_token(make_token("tok-ssrf")));
        let clouds = Arc::new(FakeCloudDir {
            active: "devstack".into(),
        });
        let dir = KeystoneProjectDirectory::new(
            Arc::new(reqwest::Client::new()),
            auth,
            clouds,
            config,
            cache,
            10,
        );

        let err = dir.list_projects("devstack").await.unwrap_err();
        assert!(
            matches!(err, SwitchError::Api(ApiError::Parse(_))),
            "expected Api(Parse) for SSRF host mismatch, got {err:?}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("mismatch") || msg.contains("SSRF") || msg.contains("scheme") || msg.contains("host"),
            "error message should mention the rejection reason: {msg}"
        );
    }

    #[tokio::test]
    async fn links_next_rejects_file_scheme() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");

        let file_url = "file:///etc/passwd";
        let page1_body = format!(
            r#"{{"projects":[{{"id":"p1","name":"proj1","domain_id":"d1"}}],"links":{{"next":"{file_url}"}}}}"#
        );

        let _handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf).await.unwrap();
            let wire = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{page1_body}",
                page1_body.len()
            );
            stream.write_all(wire.as_bytes()).await.unwrap();
            let _ = stream.shutdown().await;
        });

        let config = make_config("devstack", &format!("{base}/v3"));
        let cache = Arc::new(DirectoryCache::new(Duration::from_secs(60)));
        let auth = Arc::new(FakeScopedAuth::with_token(make_token("tok-file")));
        let clouds = Arc::new(FakeCloudDir {
            active: "devstack".into(),
        });
        let dir = KeystoneProjectDirectory::new(
            Arc::new(reqwest::Client::new()),
            auth,
            clouds,
            config,
            cache,
            10,
        );

        let err = dir.list_projects("devstack").await.unwrap_err();
        assert!(
            matches!(err, SwitchError::Api(ApiError::Parse(_))),
            "expected Api(Parse) for file:// scheme, got {err:?}"
        );
    }

    #[tokio::test]
    async fn http_401_maps_to_rescope_rejected() {
        let resp = CannedResponse {
            status_line: "401 Unauthorized",
            body: r#"{"error":{"message":"The request you have made requires authentication.","code":401,"title":"Unauthorized"}}"#.into(),
        };
        let (base_url, _handle) = spawn_one_shot(resp).await;
        let config = make_config("devstack", &format!("{base_url}/v3"));
        let cache = Arc::new(DirectoryCache::new(Duration::from_secs(60)));
        let auth = Arc::new(FakeScopedAuth::with_token(make_token("tok-401")));
        let clouds = Arc::new(FakeCloudDir {
            active: "devstack".into(),
        });
        let dir = KeystoneProjectDirectory::new(
            Arc::new(reqwest::Client::new()),
            auth,
            clouds,
            config,
            cache,
            10,
        );

        let err = dir.list_projects("devstack").await.unwrap_err();
        assert!(
            matches!(err, SwitchError::RescopeRejected(_)),
            "expected RescopeRejected for 401, got {err:?}"
        );
    }

    #[tokio::test]
    async fn http_500_maps_to_api_error() {
        let resp = CannedResponse {
            status_line: "500 Internal Server Error",
            body: r#"{"error":"internal server error"}"#.into(),
        };
        let (base_url, _handle) = spawn_one_shot(resp).await;
        let config = make_config("devstack", &format!("{base_url}/v3"));
        let cache = Arc::new(DirectoryCache::new(Duration::from_secs(60)));
        let auth = Arc::new(FakeScopedAuth::with_token(make_token("tok-500")));
        let clouds = Arc::new(FakeCloudDir {
            active: "devstack".into(),
        });
        let dir = KeystoneProjectDirectory::new(
            Arc::new(reqwest::Client::new()),
            auth,
            clouds,
            config,
            cache,
            10,
        );

        let err = dir.list_projects("devstack").await.unwrap_err();
        assert!(
            matches!(err, SwitchError::Api(_)),
            "expected Api for 500, got {err:?}"
        );
    }

    #[tokio::test]
    async fn partial_page_error_discards_all() {
        // Page 1 succeeds, page 2 returns 500 → no candidates, no cache put
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        let page2_url = format!("{base}/v3/auth/projects?page=2");

        let _handle = tokio::spawn(async move {
            // Page 1
            {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buf = [0u8; 8192];
                let _ = stream.read(&mut buf).await.unwrap();
                let body = format!(
                    r#"{{"projects":[{{"id":"p1","name":"a","domain_id":"d"}}],"links":{{"next":"{page2_url}"}}}}"#
                );
                let wire = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(wire.as_bytes()).await.unwrap();
                let _ = stream.shutdown().await;
            }
            // Page 2 → 500
            {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buf = [0u8; 8192];
                let _ = stream.read(&mut buf).await.unwrap();
                let body = r#"{"error":"boom"}"#;
                let wire = format!(
                    "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(wire.as_bytes()).await.unwrap();
                let _ = stream.shutdown().await;
            }
        });

        let config = make_config("devstack", &format!("{base}/v3"));
        let cache = Arc::new(DirectoryCache::new(Duration::from_secs(60)));
        let auth = Arc::new(FakeScopedAuth::with_token(make_token("tok-partial")));
        let clouds = Arc::new(FakeCloudDir {
            active: "devstack".into(),
        });
        let dir = KeystoneProjectDirectory::new(
            Arc::new(reqwest::Client::new()),
            auth,
            clouds,
            config,
            cache.clone(),
            10,
        );

        let err = dir.list_projects("devstack").await.unwrap_err();
        assert!(
            matches!(err, SwitchError::Api(_)),
            "expected Api error on partial failure, got {err:?}"
        );

        // Cache must NOT have been written
        let fp = TokenScopeFingerprint::new().compute(&make_token("tok-partial"));
        assert!(
            cache.get("devstack", &fp).is_none(),
            "cache must not be written on partial failure"
        );
    }

    #[tokio::test]
    async fn cache_hit_avoids_http() {
        // Pre-warm cache; server should never be contacted
        let config = make_config("devstack", "http://127.0.0.1:1/v3"); // unreachable port
        let cache = Arc::new(DirectoryCache::new(Duration::from_secs(60)));
        let token = make_token("tok-cached");
        let fp = TokenScopeFingerprint::new().compute(&token);
        let pre_cached = vec![ProjectCandidate {
            cloud: "devstack".into(),
            project_id: "pre-p1".into(),
            project_name: "pre-admin".into(),
            domain: "default".into(),
        }];
        cache.put("devstack", &fp, pre_cached.clone());

        let auth = Arc::new(FakeScopedAuth::with_token(token));
        let clouds = Arc::new(FakeCloudDir {
            active: "devstack".into(),
        });
        let dir = KeystoneProjectDirectory::new(
            Arc::new(reqwest::Client::new()),
            auth,
            clouds,
            config,
            cache,
            10,
        );

        let candidates = dir.list_projects("devstack").await.unwrap();
        assert_eq!(candidates, pre_cached);
    }

    #[tokio::test]
    async fn cache_miss_calls_http_and_puts() {
        let body = single_project_body(None);
        let resp = CannedResponse {
            status_line: "200 OK",
            body,
        };
        let (base_url, _handle) = spawn_one_shot(resp).await;
        let config = make_config("devstack", &format!("{base_url}/v3"));
        let cache = Arc::new(DirectoryCache::new(Duration::from_secs(60)));
        let token = make_token("tok-miss");
        let fp = TokenScopeFingerprint::new().compute(&token);
        let auth = Arc::new(FakeScopedAuth::with_token(token));
        let clouds = Arc::new(FakeCloudDir {
            active: "devstack".into(),
        });
        let dir = KeystoneProjectDirectory::new(
            Arc::new(reqwest::Client::new()),
            auth,
            clouds,
            config,
            cache.clone(),
            10,
        );

        // First call — cache miss → fetch
        let candidates = dir.list_projects("devstack").await.unwrap();
        assert_eq!(candidates.len(), 1);

        // Cache should now have the result
        let cached = cache.get("devstack", &fp);
        assert!(cached.is_some(), "cache must be populated after fetch");
        assert_eq!(cached.unwrap().len(), 1);
    }
}
