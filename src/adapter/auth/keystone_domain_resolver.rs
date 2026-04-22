//! Lazy domain_id → domain.name resolver for BL-P2-080 domain fallback.

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use std::sync::Arc;

use serde::Deserialize;
use tracing::{debug, info};

use crate::adapter::auth::rescope::sanitize_rescope_body;
use crate::config::Config;
use crate::context::error::SwitchError;
use crate::port::error::ApiError;

// --- Keystone /v3/domains/{id} response ---

#[derive(Debug, Deserialize)]
struct DomainResponse {
    domain: DomainBody,
}

#[derive(Debug, Deserialize)]
struct DomainBody {
    name: String,
}

/// Resolves a Keystone `domain_id` to a human-readable `domain.name` via
/// `GET /v3/domains/{id}`, with in-memory TTL caching.
pub struct DomainNameResolver {
    client: Arc<reqwest::Client>,
    config: Arc<Config>,
    ttl: Duration,
    cache: RwLock<HashMap<(String, String), (String, Instant)>>,
}

impl DomainNameResolver {
    pub fn new(client: Arc<reqwest::Client>, config: Arc<Config>, ttl: Duration) -> Self {
        Self {
            client,
            config,
            ttl,
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Seed the cache for a given (cloud, domain_id) pair. Used in tests
    /// to inject cached entries without making HTTP calls.
    #[cfg(test)]
    pub fn seed_cache(&self, cloud: &str, domain_id: &str, name: &str) {
        let mut guard = self.cache.write().unwrap();
        guard.insert(
            (cloud.to_string(), domain_id.to_string()),
            (name.to_string(), Instant::now()),
        );
    }

    /// Resolve `domain_id` → `domain.name` for the given `cloud`.
    ///
    /// Returns cached value on hit; otherwise calls Keystone
    /// `GET {auth_url}/v3/domains/{domain_id}` with `X-Auth-Token: token_id`.
    pub async fn resolve_name(
        &self,
        cloud: &str,
        domain_id: &str,
        token_id: &str,
    ) -> Result<String, SwitchError> {
        let key = (cloud.to_string(), domain_id.to_string());

        // Check cache (drop guard before await)
        {
            let guard = self.cache.read().map_err(|_| {
                SwitchError::Api(ApiError::Parse("domain cache lock poisoned".into()))
            })?;
            if let Some((name, inserted_at)) = guard.get(&key)
                && inserted_at.elapsed() < self.ttl
            {
                debug!(cloud, domain_id, cached = true, "domain_resolve_cached");
                return Ok(name.clone());
            }
        }

        info!(cloud, domain_id, "domain_lazy_resolve");

        let auth_url = self
            .config
            .cloud_config(cloud)
            .ok_or_else(|| {
                SwitchError::Api(ApiError::Parse(format!("cloud '{cloud}' not in config")))
            })?
            .auth
            .auth_url
            .trim_end_matches('/')
            .to_string();

        let url = format!("{auth_url}/domains/{domain_id}");

        let resp = self
            .client
            .get(&url)
            .header("X-Auth-Token", token_id)
            .send()
            .await
            .map_err(|e| SwitchError::Api(ApiError::Network(e)))?;

        let status = resp.status();
        if !status.is_success() {
            let body = sanitize_rescope_body(&resp.text().await.unwrap_or_default());
            return Err(SwitchError::Api(ApiError::NotFound {
                resource_type: "domain".into(),
                id: format!("{domain_id} ({status}): {body}"),
            }));
        }

        let parsed: DomainResponse = resp.json().await.map_err(|e| {
            SwitchError::Api(ApiError::Parse(format!(
                "failed to parse /v3/domains/{domain_id} response: {e}"
            )))
        })?;

        let name = parsed.domain.name;

        // Write to cache
        {
            let mut guard = self.cache.write().map_err(|_| {
                SwitchError::Api(ApiError::Parse("domain cache lock poisoned".into()))
            })?;
            guard.insert(key, (name.clone(), Instant::now()));
        }

        Ok(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    // --- Helpers ---

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

    fn make_config(cloud: &str, auth_url: &str) -> Arc<Config> {
        // load_from validates auth credentials; provide minimal valid password auth
        let yaml = format!(
            "clouds:\n  {cloud}:\n    auth:\n      auth_url: {auth_url}\n      username: admin\n      password: secret\n"
        );
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("clouds.yaml");
        std::fs::write(&path, &yaml).expect("write yaml");
        let cfg = Config::load_from(&path).expect("load_from");
        let _ = dir;
        Arc::new(cfg)
    }

    // --- Tests ---

    #[tokio::test]
    async fn resolve_name_parses_domain_name() {
        let body = r#"{"domain":{"id":"did-1","name":"Default","enabled":true}}"#;
        let resp = CannedResponse {
            status_line: "200 OK",
            body: body.into(),
        };
        let (base_url, _handle) = spawn_one_shot(resp).await;
        let config = make_config("devstack", &format!("{base_url}/v3"));
        let resolver = DomainNameResolver::new(
            Arc::new(reqwest::Client::new()),
            config,
            Duration::from_secs(60),
        );

        let name = resolver
            .resolve_name("devstack", "did-1", "tok-123")
            .await
            .unwrap();
        assert_eq!(name, "Default");
    }

    #[tokio::test]
    async fn resolve_name_cache_hit_avoids_http() {
        // No server running; cache hit should prevent any connection attempt
        let config = make_config("devstack", "http://127.0.0.1:1/v3"); // unreachable
        let resolver = DomainNameResolver::new(
            Arc::new(reqwest::Client::new()),
            config,
            Duration::from_secs(60),
        );

        // Manually seed the cache
        {
            let mut guard = resolver.cache.write().unwrap();
            guard.insert(
                ("devstack".into(), "did-cached".into()),
                ("CachedDomain".into(), Instant::now()),
            );
        }

        let name = resolver
            .resolve_name("devstack", "did-cached", "tok-x")
            .await
            .unwrap();
        assert_eq!(name, "CachedDomain");
    }

    #[tokio::test]
    async fn resolve_name_404_maps_to_switch_error() {
        let resp = CannedResponse {
            status_line: "404 Not Found",
            body: r#"{"error":{"message":"domain not found","code":404}}"#.into(),
        };
        let (base_url, _handle) = spawn_one_shot(resp).await;
        let config = make_config("devstack", &format!("{base_url}/v3"));
        let resolver = DomainNameResolver::new(
            Arc::new(reqwest::Client::new()),
            config,
            Duration::from_secs(60),
        );

        let err = resolver
            .resolve_name("devstack", "did-missing", "tok-404")
            .await
            .unwrap_err();
        assert!(
            matches!(err, SwitchError::Api(ApiError::NotFound { .. })),
            "expected Api(NotFound) for 404, got {err:?}"
        );
    }

    #[tokio::test]
    async fn http_error_body_is_truncated_and_sanitized() {
        // large body with token-like header material — must not appear in error
        let large_body = format!(
            "X-Auth-Token: gAAAALeakedDomainToken\n{}",
            "z".repeat(2048)
        );
        let resp = CannedResponse {
            status_line: "500 Internal Server Error",
            body: large_body,
        };
        let (base_url, _handle) = spawn_one_shot(resp).await;
        let config = make_config("devstack", &format!("{base_url}/v3"));
        let resolver = DomainNameResolver::new(
            Arc::new(reqwest::Client::new()),
            config,
            Duration::from_secs(60),
        );

        let err = resolver
            .resolve_name("devstack", "did-500", "tok-500-big")
            .await
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            !msg.contains("gAAAALeakedDomainToken"),
            "token material leaked in domain error: {msg}"
        );
        assert!(
            msg.chars().count() < 400,
            "domain error message too long ({} chars): {msg}",
            msg.chars().count()
        );
    }
}
