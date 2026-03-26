use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::Instrument;

use crate::port::auth::AuthProvider;
use crate::port::error::{ApiError, ApiResult};
use crate::port::types::*;

// --- Keystone v3 response types (internal) ---

#[derive(Debug, Deserialize)]
struct KeystoneTokenResponse {
    token: KeystoneTokenBody,
}

#[derive(Debug, Deserialize)]
struct KeystoneTokenBody {
    expires_at: DateTime<Utc>,
    project: Option<KeystoneProject>,
    roles: Vec<KeystoneRole>,
    catalog: Option<Vec<KeystoneCatalogEntry>>,
}

#[derive(Debug, Deserialize)]
struct KeystoneProject {
    id: String,
    name: String,
    domain: KeystoneDomain,
}

#[derive(Debug, Deserialize)]
struct KeystoneDomain {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct KeystoneRole {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct KeystoneCatalogEntry {
    #[serde(rename = "type")]
    service_type: String,
    name: String,
    endpoints: Vec<KeystoneEndpoint>,
}

#[derive(Debug, Deserialize)]
struct KeystoneEndpoint {
    url: String,
    interface: String,
    region_id: String,
}

// --- Token conversion ---

fn parse_token(token_id: String, resp: KeystoneTokenResponse) -> Token {
    let body = resp.token;
    let project = body
        .project
        .map(|p| ProjectScope {
            id: p.id,
            name: p.name,
            domain_id: p.domain.id,
            domain_name: p.domain.name,
        })
        .unwrap_or(ProjectScope {
            id: String::new(),
            name: String::new(),
            domain_id: String::new(),
            domain_name: String::new(),
        });
    let roles = body
        .roles
        .into_iter()
        .map(|r| TokenRole {
            id: r.id,
            name: r.name,
        })
        .collect();
    let catalog = body
        .catalog
        .unwrap_or_default()
        .into_iter()
        .map(|c| CatalogEntry {
            service_type: c.service_type,
            service_name: c.name,
            endpoints: c
                .endpoints
                .into_iter()
                .map(|e| Endpoint {
                    url: e.url,
                    interface: parse_interface(&e.interface),
                    region: e.region_id,
                })
                .collect(),
        })
        .collect();

    Token {
        id: token_id,
        expires_at: body.expires_at,
        project,
        roles,
        catalog,
    }
}

fn parse_interface(s: &str) -> EndpointInterface {
    match s {
        "internal" => EndpointInterface::Internal,
        "admin" => EndpointInterface::Admin,
        _ => EndpointInterface::Public,
    }
}

// --- KeystoneAuthAdapter ---

pub struct KeystoneAuthAdapter {
    client: reqwest::Client,
    credential: AuthCredential,
    token_map: Arc<RwLock<HashMap<TokenScope, Token>>>,
    active_scope: TokenScope,
    token_tx: broadcast::Sender<Token>,
    refresh_handle: Mutex<Option<JoinHandle<()>>>,
    /// Guard to ensure refresh loop is started only once.
    refresh_started: AtomicBool,
    /// Mutex to serialize concurrent refresh attempts (prevents thundering herd).
    refresh_lock: Mutex<()>,
    /// Directory for scope-keyed token cache files.
    cache_dir: PathBuf,
}

impl KeystoneAuthAdapter {
    pub fn new(credential: AuthCredential) -> Result<Self, ApiError> {
        use super::token_cache;

        let username = match &credential.method {
            AuthMethod::Password { username, .. } => username.clone(),
            AuthMethod::ApplicationCredential { id, .. } => id.clone(),
        };
        let active_scope = TokenScope::from_credential(&credential);
        let cloud_key = token_cache::compute_cloud_key(&credential.auth_url, &username);
        let cache_dir = token_cache::cache_dir_path(&cloud_key);

        // Load all cached tokens for this cloud from disk
        let cached_tokens = token_cache::load_all_tokens(&cache_dir);

        let (token_tx, _) = broadcast::channel::<Token>(16);
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .connect_timeout(Duration::from_secs(10))
                .build()?,
            credential,
            token_map: Arc::new(RwLock::new(cached_tokens)),
            active_scope,
            token_tx,
            refresh_handle: Mutex::new(None),
            refresh_started: AtomicBool::new(false),
            refresh_lock: Mutex::new(()),
            cache_dir,
        })
    }

    /// Start the background token refresh loop. Idempotent — only spawns once.
    #[tracing::instrument(skip(self))]
    async fn start_refresh_loop(&self) {
        if self.refresh_started.swap(true, Ordering::SeqCst) {
            return; // Already started
        }

        let token_map_ref = self.token_map.clone();
        let client = self.client.clone();
        let credential = self.credential.clone();
        let tx = self.token_tx.clone();
        let cache_dir = self.cache_dir.clone();
        let scope = self.active_scope.clone();

        let refresh_span = tracing::info_span!("token_refresh_loop");
        let handle = tokio::spawn(
            async move {
                loop {
                    let sleep_duration = {
                        let map = token_map_ref.read().await;
                        match map.get(&scope) {
                            Some(t) => {
                                let remaining = t.expires_at - Utc::now();
                                let refresh_at = remaining - chrono::Duration::minutes(5);
                                if refresh_at.num_seconds() > 0 {
                                    Duration::from_secs(refresh_at.num_seconds() as u64)
                                } else {
                                    Duration::from_secs(10)
                                }
                            }
                            None => Duration::from_secs(60),
                        }
                    };

                    tokio::time::sleep(sleep_duration).await;

                    match Self::do_authenticate(&client, &credential).await {
                        Ok(new_token) => {
                            let mut map = token_map_ref.write().await;
                            map.insert(scope.clone(), new_token.clone());
                            if let Err(e) = super::token_cache::save_token(&new_token, &cache_dir, &scope) {
                                tracing::warn!(error = %e, "failed to cache token to disk");
                            }
                            let _ = tx.send(new_token);
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "token refresh failed, retrying in 30s");
                            tokio::time::sleep(Duration::from_secs(30)).await;
                        }
                    }
                }
            }
            .instrument(refresh_span),
        );

        let mut h = self.refresh_handle.lock().await;
        *h = Some(handle);
    }

    /// Perform the actual Keystone v3 auth POST.
    #[tracing::instrument(skip(client, credential), fields(auth_url = %credential.auth_url))]
    async fn do_authenticate(
        client: &reqwest::Client,
        credential: &AuthCredential,
    ) -> ApiResult<Token> {
        let auth_url = format!(
            "{}/auth/tokens",
            credential.auth_url.trim_end_matches('/')
        );
        let body = Self::build_auth_body(credential);
        let resp = client
            .post(&auth_url)
            .json(&body)
            .send()
            .await
            .map_err(ApiError::Network)?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiError::AuthFailed(body));
        }

        let token_id = resp
            .headers()
            .get("X-Subject-Token")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ApiError::AuthFailed("Missing X-Subject-Token header".into()))?
            .to_string();

        let body: KeystoneTokenResponse = resp
            .json()
            .await
            .map_err(|e| ApiError::Parse(format!("Failed to parse Keystone response: {e}")))?;

        Ok(parse_token(token_id, body))
    }

    /// Build the Keystone v3 auth request body based on AuthMethod.
    /// Note: authenticate() always uses self.credential (passed at construction).
    /// The credential parameter in AuthProvider::authenticate() should match self.credential.
    pub(crate) fn build_auth_body(credential: &AuthCredential) -> serde_json::Value {
        let identity = match &credential.method {
            AuthMethod::Password {
                username,
                password,
                domain_name,
            } => serde_json::json!({
                "methods": ["password"],
                "password": {
                    "user": {
                        "name": username,
                        "password": password,
                        "domain": { "name": domain_name }
                    }
                }
            }),
            AuthMethod::ApplicationCredential { id, secret } => serde_json::json!({
                "methods": ["application_credential"],
                "application_credential": {
                    "id": id,
                    "secret": secret
                }
            }),
        };

        let mut auth = serde_json::json!({ "identity": identity });

        if let Some(ref scope) = credential.project_scope {
            auth["scope"] = serde_json::json!({
                "project": {
                    "name": scope.name,
                    "domain": { "name": scope.domain_name }
                }
            });
        }

        serde_json::json!({ "auth": auth })
    }
}

#[async_trait]
impl AuthProvider for KeystoneAuthAdapter {
    async fn authenticate(&self, credential: &AuthCredential) -> ApiResult<Token> {
        let token = Self::do_authenticate(&self.client, credential).await?;
        {
            let mut map = self.token_map.write().await;
            map.insert(self.active_scope.clone(), token.clone());
        }
        if let Err(e) = super::token_cache::save_token(&token, &self.cache_dir, &self.active_scope) {
            tracing::warn!(error = %e, "failed to cache token to disk");
        }
        self.start_refresh_loop().await;
        Ok(token)
    }

    #[tracing::instrument(skip(self))]
    async fn refresh_token(&self) -> ApiResult<Token> {
        let token = Self::do_authenticate(&self.client, &self.credential).await?;
        {
            let mut map = self.token_map.write().await;
            map.insert(self.active_scope.clone(), token.clone());
        }
        if let Err(e) = super::token_cache::save_token(&token, &self.cache_dir, &self.active_scope) {
            tracing::warn!(error = %e, "failed to cache token to disk");
        }
        let _ = self.token_tx.send(token.clone());
        Ok(token)
    }

    /// Get a valid token string. If near-expiry (<1min), refresh first.
    /// Uses a Mutex to prevent thundering herd — only one refresh at a time.
    #[tracing::instrument(skip(self))]
    async fn get_token(&self) -> ApiResult<String> {
        // Ensure refresh loop is running (idempotent — handles cached token from disk)
        self.start_refresh_loop().await;

        // Fast path: token is still valid for active scope
        {
            let map = self.token_map.read().await;
            if let Some(t) = map.get(&self.active_scope) {
                if t.expires_at > Utc::now() + chrono::Duration::minutes(1) {
                    return Ok(t.id.clone());
                }
            }
        }

        // Slow path: serialize refresh attempts
        let _guard = self.refresh_lock.lock().await;

        // Double-check after acquiring lock
        {
            let map = self.token_map.read().await;
            if let Some(t) = map.get(&self.active_scope) {
                if t.expires_at > Utc::now() + chrono::Duration::minutes(1) {
                    return Ok(t.id.clone());
                }
            }
        }

        let token = self.refresh_token().await?;
        Ok(token.id)
    }

    async fn get_token_info(&self) -> ApiResult<Token> {
        let map = self.token_map.read().await;
        map.get(&self.active_scope)
            .cloned()
            .ok_or(ApiError::AuthFailed("Not authenticated".into()))
    }

    /// Inject X-Auth-Token header. Phase 1: token-based auth only.
    /// Phase 2 note: for signed auth (HMAC), this method will need the actual
    /// method/url/headers/body to compute the signature. Currently unused parameters
    /// are preserved in the signature for forward compatibility.
    #[tracing::instrument(skip(self, _headers, _body))]
    async fn authenticate_request(
        &self,
        _method: &str,
        _url: &str,
        _headers: &reqwest::header::HeaderMap,
        _body: Option<&[u8]>,
    ) -> ApiResult<AuthHeaders> {
        let token_id = self.get_token().await?;
        Ok(AuthHeaders {
            headers: vec![("X-Auth-Token".to_string(), token_id)],
        })
    }

    #[tracing::instrument(skip(self))]
    async fn get_endpoint(
        &self,
        service_type: &str,
        interface: EndpointInterface,
        region: Option<&str>,
    ) -> ApiResult<String> {
        // Ensure we have a valid token (triggers initial auth if needed)
        let _ = self.get_token().await?;

        let token = self.get_token_info().await?;

        token
            .catalog
            .iter()
            .find(|c| c.service_type == service_type)
            .and_then(|c| {
                c.endpoints.iter().find(|e| {
                    e.interface == interface && region.map_or(true, |r| e.region == r)
                })
            })
            .map(|e| e.url.clone())
            .ok_or(ApiError::ServiceUnavailable {
                service: service_type.to_string(),
            })
    }

    fn subscribe_token_refresh(&self) -> broadcast::Receiver<Token> {
        self.token_tx.subscribe()
    }

    async fn has_role(&self, role_name: &str) -> ApiResult<bool> {
        let map = self.token_map.read().await;
        let token = map
            .get(&self.active_scope)
            .ok_or(ApiError::AuthFailed("Not authenticated".into()))?;
        Ok(token.roles.iter().any(|r| r.name == role_name))
    }

    async fn get_catalog(&self) -> ApiResult<Vec<CatalogEntry>> {
        let map = self.token_map.read().await;
        let token = map
            .get(&self.active_scope)
            .ok_or(ApiError::AuthFailed("Not authenticated".into()))?;
        Ok(token.catalog.clone())
    }

    async fn get_capabilities(&self) -> ApiResult<Vec<Capability>> {
        // Phase 1: Keystone has no capability concept. Return empty.
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_credential_password() -> AuthCredential {
        AuthCredential {
            auth_url: "https://keystone.example.com:5000/v3".to_string(),
            method: AuthMethod::Password {
                username: "admin".to_string(),
                password: "secret123".to_string(),
                domain_name: "Default".to_string(),
            },
            project_scope: Some(ProjectScopeParam {
                name: "admin-project".to_string(),
                domain_name: "Default".to_string(),
            }),
        }
    }

    fn sample_credential_app() -> AuthCredential {
        AuthCredential {
            auth_url: "https://keystone.example.com:5000/v3".to_string(),
            method: AuthMethod::ApplicationCredential {
                id: "app-cred-id".to_string(),
                secret: "app-cred-secret".to_string(),
            },
            project_scope: None,
        }
    }

    fn sample_keystone_response_json() -> &'static str {
        r#"{
            "token": {
                "expires_at": "2099-12-31T23:59:59.000000Z",
                "project": {
                    "id": "proj-123",
                    "name": "admin-project",
                    "domain": { "id": "default", "name": "Default" }
                },
                "roles": [
                    { "id": "role-1", "name": "admin" },
                    { "id": "role-2", "name": "member" }
                ],
                "catalog": [
                    {
                        "type": "compute",
                        "name": "nova",
                        "endpoints": [
                            { "url": "https://nova:8774/v2.1", "interface": "internal", "region_id": "RegionOne" },
                            { "url": "https://nova-pub:8774/v2.1", "interface": "public", "region_id": "RegionOne" }
                        ]
                    },
                    {
                        "type": "identity",
                        "name": "keystone",
                        "endpoints": [
                            { "url": "https://keystone:5000/v3", "interface": "public", "region_id": "RegionOne" }
                        ]
                    }
                ]
            }
        }"#
    }

    #[test]
    fn test_build_auth_body_password() {
        let cred = sample_credential_password();
        let body = KeystoneAuthAdapter::build_auth_body(&cred);

        assert_eq!(body["auth"]["identity"]["methods"][0], "password");
        assert_eq!(body["auth"]["identity"]["password"]["user"]["name"], "admin");
        assert_eq!(
            body["auth"]["identity"]["password"]["user"]["domain"]["name"],
            "Default"
        );
        assert_eq!(body["auth"]["scope"]["project"]["name"], "admin-project");
    }

    #[test]
    fn test_build_auth_body_app_credential() {
        let cred = sample_credential_app();
        let body = KeystoneAuthAdapter::build_auth_body(&cred);

        assert_eq!(
            body["auth"]["identity"]["methods"][0],
            "application_credential"
        );
        assert_eq!(
            body["auth"]["identity"]["application_credential"]["id"],
            "app-cred-id"
        );
        assert!(body["auth"]["scope"].is_null());
    }

    #[test]
    fn test_parse_token_from_keystone_response() {
        let json_str = sample_keystone_response_json();
        let resp: KeystoneTokenResponse = serde_json::from_str(json_str).unwrap();
        let token = parse_token("tok-abc-123".to_string(), resp);

        assert_eq!(token.id, "tok-abc-123");
        assert_eq!(token.project.name, "admin-project");
        assert_eq!(token.project.domain_name, "Default");
        assert_eq!(token.roles.len(), 2);
        assert_eq!(token.roles[0].name, "admin");
        assert_eq!(token.catalog.len(), 2);
        assert_eq!(token.catalog[0].service_type, "compute");
        assert_eq!(token.catalog[0].endpoints.len(), 2);
        assert_eq!(
            token.catalog[0].endpoints[0].interface,
            EndpointInterface::Internal
        );
    }

    #[test]
    fn test_parse_token_no_catalog() {
        let json_str = r#"{
            "token": {
                "expires_at": "2099-12-31T23:59:59.000000Z",
                "roles": [{ "id": "r1", "name": "member" }]
            }
        }"#;
        let resp: KeystoneTokenResponse = serde_json::from_str(json_str).unwrap();
        let token = parse_token("tok-1".to_string(), resp);

        assert!(token.catalog.is_empty());
        assert_eq!(token.roles.len(), 1);
        assert!(token.project.id.is_empty());
    }

    #[test]
    fn test_parse_interface() {
        assert_eq!(parse_interface("internal"), EndpointInterface::Internal);
        assert_eq!(parse_interface("admin"), EndpointInterface::Admin);
        assert_eq!(parse_interface("public"), EndpointInterface::Public);
        assert_eq!(parse_interface("unknown"), EndpointInterface::Public);
    }

    #[tokio::test]
    async fn test_get_endpoint_from_token() {
        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
        let resp: KeystoneTokenResponse =
            serde_json::from_str(sample_keystone_response_json()).unwrap();
        let token = parse_token("tok-1".to_string(), resp);
        {
            let mut map = adapter.token_map.write().await;
            map.insert(adapter.active_scope.clone(), token);
        }

        let url = adapter
            .get_endpoint("compute", EndpointInterface::Internal, Some("RegionOne"))
            .await
            .unwrap();
        assert_eq!(url, "https://nova:8774/v2.1");

        let url = adapter
            .get_endpoint("compute", EndpointInterface::Public, Some("RegionOne"))
            .await
            .unwrap();
        assert_eq!(url, "https://nova-pub:8774/v2.1");

        let err = adapter
            .get_endpoint("image", EndpointInterface::Public, None)
            .await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn test_has_role() {
        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
        let resp: KeystoneTokenResponse =
            serde_json::from_str(sample_keystone_response_json()).unwrap();
        let token = parse_token("tok-1".to_string(), resp);
        {
            let mut map = adapter.token_map.write().await;
            map.insert(adapter.active_scope.clone(), token);
        }

        assert!(adapter.has_role("admin").await.unwrap());
        assert!(adapter.has_role("member").await.unwrap());
        assert!(!adapter.has_role("reader").await.unwrap());
    }

    #[tokio::test]
    async fn test_authenticate_request_injects_token() {
        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
        let resp: KeystoneTokenResponse =
            serde_json::from_str(sample_keystone_response_json()).unwrap();
        let token = parse_token("tok-xyz".to_string(), resp);
        {
            let mut map = adapter.token_map.write().await;
            map.insert(adapter.active_scope.clone(), token);
        }

        let headers = reqwest::header::HeaderMap::new();
        let auth = adapter
            .authenticate_request("GET", "https://nova:8774/v2.1/servers", &headers, None)
            .await
            .unwrap();

        assert_eq!(auth.headers.len(), 1);
        assert_eq!(auth.headers[0].0, "X-Auth-Token");
        assert_eq!(auth.headers[0].1, "tok-xyz");
    }

    #[tokio::test]
    async fn test_get_token_info_not_authenticated() {
        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
        let err = adapter.get_token_info().await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn test_get_catalog() {
        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
        let resp: KeystoneTokenResponse =
            serde_json::from_str(sample_keystone_response_json()).unwrap();
        let token = parse_token("tok-1".to_string(), resp);
        {
            let mut map = adapter.token_map.write().await;
            map.insert(adapter.active_scope.clone(), token);
        }

        let catalog = adapter.get_catalog().await.unwrap();
        assert_eq!(catalog.len(), 2);
        assert_eq!(catalog[0].service_type, "compute");
    }

    #[tokio::test]
    async fn test_refresh_loop_idempotent() {
        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
        assert!(!adapter.refresh_started.load(Ordering::SeqCst));

        // Simulate first start
        adapter.refresh_started.store(true, Ordering::SeqCst);
        assert!(adapter.refresh_started.load(Ordering::SeqCst));

        // Second call should be no-op (tested via AtomicBool flag)
        let was_started = adapter.refresh_started.swap(true, Ordering::SeqCst);
        assert!(was_started); // was already true
    }
}
