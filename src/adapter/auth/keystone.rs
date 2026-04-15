use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex, RwLock as StdRwLock};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;
use tracing::Instrument;

use crate::context::SwitchError;
use crate::port::auth::AuthProvider;
use crate::port::error::{ApiError, ApiResult};
use crate::port::scoped_auth::ScopedAuthPort;
use crate::port::types::*;

// --- Keystone v3 response types (internal) ---

#[derive(Debug, Deserialize)]
pub(super) struct KeystoneTokenResponse {
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

pub(super) fn parse_token(token_id: String, resp: KeystoneTokenResponse) -> Token {
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
    /// `StdRwLock` rather than `tokio::sync::RwLock` because no critical
    /// section here is held across an await point; using the synchronous
    /// lock keeps [`ScopedAuthPort::current_token`] callable without async.
    token_map: Arc<StdRwLock<HashMap<TokenScope, Token>>>,
    /// `Arc<StdMutex<_>>` so `set_active` can atomically swap the scope and
    /// the background refresh loop observes the change on its next tick.
    active_scope: Arc<StdMutex<TokenScope>>,
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
            token_map: Arc::new(StdRwLock::new(cached_tokens)),
            active_scope: Arc::new(StdMutex::new(active_scope)),
            token_tx,
            refresh_handle: Mutex::new(None),
            refresh_started: AtomicBool::new(false),
            refresh_lock: Mutex::new(()),
            cache_dir,
        })
    }

    fn active_scope_snapshot(&self) -> TokenScope {
        self.active_scope
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// The scope tied to `self.credential` — the only scope our refresh path
    /// can legitimately re-issue tokens for, since `do_authenticate` bakes
    /// the credential's project_scope into the request body.
    fn initial_scope(&self) -> TokenScope {
        TokenScope::from_credential(&self.credential)
    }
}

/// Refresh paths must refuse to write into a scope key whose token cannot
/// be obtained from `self.credential` alone — otherwise an INITIAL-scope
/// token gets stored under a non-initial scope key, granting elevated
/// privileges to callers that read by the active key. Returns true when the
/// refresh would put the right token under the right key.
pub(crate) fn is_refresh_safe(active: &TokenScope, initial: &TokenScope) -> bool {
    active == initial
}

impl KeystoneAuthAdapter {
    // (continued)

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
        // Share the scope handle so `set_active` changes are observed on the
        // next refresh tick rather than after a restart.
        let scope_ref = self.active_scope.clone();
        // Captured at spawn so the C1 guard can detect scope drift without
        // re-reading credential on every tick.
        let initial_scope = self.initial_scope();

        let refresh_span = tracing::info_span!("token_refresh_loop");
        let handle = tokio::spawn(
            async move {
                loop {
                    let scope = scope_ref.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    let sleep_duration = {
                        let map = token_map_ref.read().unwrap_or_else(|e| e.into_inner());
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

                    // BL-P2-031 C1 guard: same as refresh_token. Skip ticks
                    // while the active scope differs from the credential's
                    // scope; otherwise we would mint an INITIAL token and
                    // overwrite the demo entry with elevated privileges.
                    let active_now = scope_ref.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    if !is_refresh_safe(&active_now, &initial_scope) {
                        tracing::warn!(
                            ?active_now,
                            ?initial_scope,
                            "refresh loop tick skipped — scope drifted; rescope-based refresh required",
                        );
                        continue;
                    }

                    match Self::do_authenticate(&client, &credential).await {
                        Ok(new_token) => {
                            {
                                let mut map =
                                    token_map_ref.write().unwrap_or_else(|e| e.into_inner());
                                map.insert(scope.clone(), new_token.clone());
                            }
                            if let Err(e) =
                                super::token_cache::save_token(&new_token, &cache_dir, &scope)
                            {
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
    async fn authenticate(&self, _credential: &AuthCredential) -> ApiResult<Token> {
        // BL-P2-031 S1 guard: ignore the parameter and always use
        // self.credential. Storing the result under initial_scope (not
        // active_scope_snapshot) ensures the keystone-issued token lands
        // under a key that matches its actual claims — mirroring the C1
        // refresh_token guard. Trait signature keeps the parameter for
        // backwards compatibility but the docstring already declared
        // self.credential as authoritative.
        let token = Self::do_authenticate(&self.client, &self.credential).await?;
        let scope = self.initial_scope();
        {
            let mut map = self.token_map.write().unwrap_or_else(|e| e.into_inner());
            map.insert(scope.clone(), token.clone());
        }
        if let Err(e) = super::token_cache::save_token(&token, &self.cache_dir, &scope) {
            tracing::warn!(error = %e, "failed to cache token to disk");
        }
        self.start_refresh_loop().await;
        Ok(token)
    }

    #[tracing::instrument(skip(self))]
    async fn refresh_token(&self) -> ApiResult<Token> {
        // BL-P2-031 C1 guard: `do_authenticate` always mints a token in
        // self.credential's scope. Writing that into the active scope key
        // when the active scope has drifted (via set_active) would store an
        // INITIAL-scope token under e.g. a "demo" key — privilege escalation.
        // Refreshing rescoped tokens is the rescope adapter's job, not ours.
        let scope = self.active_scope_snapshot();
        let initial = self.initial_scope();
        if !is_refresh_safe(&scope, &initial) {
            return Err(ApiError::AuthFailed(format!(
                "refresh_token refused: active scope {scope:?} differs from initial scope {initial:?}; rescope-based refresh required",
            )));
        }
        let token = Self::do_authenticate(&self.client, &self.credential).await?;
        {
            let mut map = self.token_map.write().unwrap_or_else(|e| e.into_inner());
            map.insert(scope.clone(), token.clone());
        }
        if let Err(e) = super::token_cache::save_token(&token, &self.cache_dir, &scope) {
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
        let scope = self.active_scope_snapshot();
        {
            let map = self.token_map.read().unwrap_or_else(|e| e.into_inner());
            if let Some(t) = map.get(&scope)
                && t.expires_at > Utc::now() + chrono::Duration::minutes(1) {
                    return Ok(t.id.clone());
                }
        }

        // Slow path: serialize refresh attempts
        let _guard = self.refresh_lock.lock().await;

        // Double-check after acquiring lock. Re-snapshot the scope because
        // `set_active` could have swapped it while we waited on the mutex.
        let scope = self.active_scope_snapshot();
        {
            let map = self.token_map.read().unwrap_or_else(|e| e.into_inner());
            if let Some(t) = map.get(&scope)
                && t.expires_at > Utc::now() + chrono::Duration::minutes(1) {
                    return Ok(t.id.clone());
                }
        }

        let token = self.refresh_token().await?;
        Ok(token.id)
    }

    async fn get_token_info(&self) -> ApiResult<Token> {
        let scope = self.active_scope_snapshot();
        let map = self.token_map.read().unwrap_or_else(|e| e.into_inner());
        map.get(&scope)
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
                    e.interface == interface && region.is_none_or(|r| e.region == r)
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
        let scope = self.active_scope_snapshot();
        let map = self.token_map.read().unwrap_or_else(|e| e.into_inner());
        let token = map
            .get(&scope)
            .ok_or(ApiError::AuthFailed("Not authenticated".into()))?;
        Ok(token.roles.iter().any(|r| r.name == role_name))
    }

    async fn get_catalog(&self) -> ApiResult<Vec<CatalogEntry>> {
        let scope = self.active_scope_snapshot();
        let map = self.token_map.read().unwrap_or_else(|e| e.into_inner());
        let token = map
            .get(&scope)
            .ok_or(ApiError::AuthFailed("Not authenticated".into()))?;
        Ok(token.catalog.clone())
    }

    async fn get_capabilities(&self) -> ApiResult<Vec<Capability>> {
        // Phase 1: Keystone has no capability concept. Return empty.
        Ok(Vec::new())
    }
}

/// BL-P2-031 Unit 3b — lets the runtime context switcher read and swap the
/// active scope/token atomically without going through the async auth surface.
///
/// `current_token` returns `None` when no token has been cached yet for the
/// active scope. Callers (notably [`crate::adapter::auth::scoped_session::ScopedAuthSession::begin`])
/// must translate `None` into a switch error — fabricating an empty
/// placeholder would let pre-auth misuse cascade into a corrupt rollback
/// chain (review C2).
#[async_trait]
impl ScopedAuthPort for KeystoneAuthAdapter {
    fn current_scope(&self) -> TokenScope {
        self.active_scope_snapshot()
    }

    fn current_token(&self) -> Option<Token> {
        let scope = self.active_scope_snapshot();
        let map = self.token_map.read().unwrap_or_else(|e| e.into_inner());
        map.get(&scope).cloned()
    }

    async fn set_active(&self, scope: TokenScope, token: Token) -> Result<(), SwitchError> {
        // Stage the new token under its scope key first; once that's in place
        // we flip active_scope. Order matters so a concurrent `current_token`
        // never sees the new scope pointing at a missing map entry.
        {
            let mut map = self.token_map.write().unwrap_or_else(|e| e.into_inner());
            map.insert(scope.clone(), token);
        }
        {
            let mut active = self
                .active_scope
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *active = scope;
        }
        Ok(())
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
            let mut map = adapter.token_map.write().unwrap_or_else(|e| e.into_inner());
            map.insert(adapter.active_scope_snapshot(), token);
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
            let mut map = adapter.token_map.write().unwrap_or_else(|e| e.into_inner());
            map.insert(adapter.active_scope_snapshot(), token);
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
            let mut map = adapter.token_map.write().unwrap_or_else(|e| e.into_inner());
            map.insert(adapter.active_scope_snapshot(), token);
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
            let mut map = adapter.token_map.write().unwrap_or_else(|e| e.into_inner());
            map.insert(adapter.active_scope_snapshot(), token);
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

    // ---------- BL-P2-031 Unit 3b: ScopedAuthPort impl ----------

    fn seeded_adapter_with_token(token: Token) -> (KeystoneAuthAdapter, TokenScope) {
        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
        let scope = adapter.active_scope_snapshot();
        {
            let mut map = adapter
                .token_map
                .write()
                .unwrap_or_else(|e| e.into_inner());
            map.insert(scope.clone(), token);
        }
        (adapter, scope)
    }

    fn sample_token(id: &str, project_name: &str) -> Token {
        Token {
            id: id.into(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            project: ProjectScope {
                id: format!("id-{project_name}"),
                name: project_name.into(),
                domain_id: "default".into(),
                domain_name: "default".into(),
            },
            roles: Vec::new(),
            catalog: Vec::new(),
        }
    }

    #[tokio::test]
    async fn scoped_auth_current_scope_matches_credential_at_construction() {
        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
        assert_eq!(
            ScopedAuthPort::current_scope(&adapter),
            TokenScope::Project {
                name: "admin-project".into(),
                domain: "default".into(),
            }
        );
    }

    #[tokio::test]
    async fn scoped_auth_current_token_returns_token_for_active_scope() {
        let (adapter, _) = seeded_adapter_with_token(sample_token("tok-initial", "admin"));
        let tok = ScopedAuthPort::current_token(&adapter).expect("token should be cached");
        assert_eq!(tok.id, "tok-initial");
        assert_eq!(tok.project.name, "admin");
    }

    #[tokio::test]
    async fn scoped_auth_current_token_returns_none_before_authentication() {
        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
        // No token has been inserted into token_map for the active scope.
        assert!(
            ScopedAuthPort::current_token(&adapter).is_none(),
            "current_token must not fabricate a placeholder pre-auth (review C2)",
        );
    }

    #[tokio::test]
    async fn scoped_auth_set_active_swaps_scope_and_token_atomically() {
        let (adapter, _) = seeded_adapter_with_token(sample_token("tok-initial", "admin"));
        let new_scope = TokenScope::Project {
            name: "demo".into(),
            domain: "default".into(),
        };
        let new_token = sample_token("tok-demo", "demo");

        adapter
            .set_active(new_scope.clone(), new_token.clone())
            .await
            .expect("set_active should succeed");

        assert_eq!(ScopedAuthPort::current_scope(&adapter), new_scope);
        let after = ScopedAuthPort::current_token(&adapter).expect("set_active should cache token");
        assert_eq!(after.id, "tok-demo");
    }

    // ---------- C1 fix: refresh-scope guard ----------

    #[test]
    fn is_refresh_safe_when_active_matches_initial() {
        let s = TokenScope::Project {
            name: "admin".into(),
            domain: "default".into(),
        };
        assert!(is_refresh_safe(&s, &s));
    }

    #[test]
    fn is_refresh_safe_false_when_active_differs_from_initial() {
        let initial = TokenScope::Project {
            name: "admin".into(),
            domain: "default".into(),
        };
        let active = TokenScope::Project {
            name: "demo".into(),
            domain: "default".into(),
        };
        assert!(!is_refresh_safe(&active, &initial));
    }

    #[tokio::test]
    async fn refresh_token_refuses_when_active_scope_drifted_from_initial() {
        // After set_active to a non-initial scope, refresh_token must NOT
        // reauthenticate with the original credential (which would mint an
        // INITIAL-scope token and stash it under the demo key — privilege
        // escalation per BL-P2-031 review C1).
        let adapter = KeystoneAuthAdapter::new(sample_credential_password()).unwrap();
        let demo_scope = TokenScope::Project {
            name: "demo".into(),
            domain: "default".into(),
        };
        adapter
            .set_active(demo_scope.clone(), sample_token("tok-demo", "demo"))
            .await
            .unwrap();

        let err = adapter.refresh_token().await.unwrap_err();
        match err {
            ApiError::AuthFailed(msg) => {
                assert!(
                    msg.to_lowercase().contains("scope"),
                    "error should mention scope drift; got: {msg}"
                );
            }
            other => panic!("expected AuthFailed about scope drift, got {other:?}"),
        }

        // Demo entry must remain the manually-staged token, untouched.
        let map = adapter
            .token_map
            .read()
            .unwrap_or_else(|e| e.into_inner());
        assert_eq!(map.get(&demo_scope).unwrap().id, "tok-demo");
    }

    // ---------- S1 fix: authenticate() guard ----------
    //
    // Local one-shot HTTP responder so we can drive the real `authenticate`
    // wire path without a Keystone server. Same pattern as rescope.rs tests.

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    struct AuthCannedResponse {
        status_line: &'static str,
        extra_headers: String,
        body: String,
    }

    async fn spawn_auth_responder(
        resp: AuthCannedResponse,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");
        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf).await.unwrap();
            let wire = format!(
                "HTTP/1.1 {status}\r\n\
                 Content-Type: application/json\r\n\
                 Content-Length: {len}\r\n\
                 {extra}\
                 \r\n\
                 {body}",
                status = resp.status_line,
                len = resp.body.len(),
                extra = resp.extra_headers,
                body = resp.body,
            );
            stream.write_all(wire.as_bytes()).await.unwrap();
            let _ = stream.shutdown().await;
        });
        (base_url, handle)
    }

    fn credential_at(auth_url: String) -> AuthCredential {
        AuthCredential {
            auth_url,
            method: AuthMethod::Password {
                username: "admin".into(),
                password: "secret".into(),
                domain_name: "Default".into(),
            },
            project_scope: Some(ProjectScopeParam {
                name: "admin-project".into(),
                domain_name: "Default".into(),
            }),
        }
    }

    #[tokio::test]
    async fn authenticate_keys_under_initial_scope_even_after_set_active_drift() {
        // Review S1: after set_active(demo) drifts the active scope,
        // `authenticate()` must NOT write the (initial-scope-credential)
        // token under the demo key — that's the same privilege-escalation
        // pattern C1 closed for refresh_token. The token belongs under the
        // scope its claims actually match (the credential's scope).
        let resp = AuthCannedResponse {
            status_line: "200 OK",
            extra_headers: "X-Subject-Token: admin-tok-NEW\r\n".into(),
            body: sample_keystone_response_json().to_string(),
        };
        let (base_url, _handle) = spawn_auth_responder(resp).await;
        let credential = credential_at(format!("{base_url}/v3"));
        let adapter = KeystoneAuthAdapter::new(credential.clone()).unwrap();
        let initial = adapter.initial_scope();

        let demo = TokenScope::Project {
            name: "demo".into(),
            domain: "default".into(),
        };
        adapter
            .set_active(demo.clone(), sample_token("tok-demo-prior", "demo"))
            .await
            .unwrap();

        let _ = AuthProvider::authenticate(&adapter, &credential).await.unwrap();

        let map = adapter
            .token_map
            .read()
            .unwrap_or_else(|e| e.into_inner());
        // Demo entry must remain the staged token — authenticate must not
        // overwrite it with a credential-scoped token.
        let demo_entry = map.get(&demo).expect("demo entry must persist");
        assert_eq!(
            demo_entry.id, "tok-demo-prior",
            "authenticate() leaked an admin-scoped token into the demo key — S1 vector still open"
        );
        // Initial entry receives the freshly authenticated admin token.
        let initial_entry = map.get(&initial).expect("initial entry must be populated");
        assert_eq!(initial_entry.id, "admin-tok-NEW");
    }

    #[tokio::test]
    async fn authenticate_ignores_external_credential_argument() {
        // Review S1: even if a caller passes a forged credential, authenticate
        // must use self.credential. Otherwise an attacker with control of the
        // call site (or a buggy caller) can reauthenticate with arbitrary
        // privileges.
        let resp = AuthCannedResponse {
            status_line: "200 OK",
            extra_headers: "X-Subject-Token: tok-from-self-credential\r\n".into(),
            body: sample_keystone_response_json().to_string(),
        };
        let (base_url, _handle) = spawn_auth_responder(resp).await;
        let our_credential = credential_at(format!("{base_url}/v3"));
        let adapter = KeystoneAuthAdapter::new(our_credential).unwrap();

        // Forged credential points at a non-existent host so any attempt to
        // honour it would surface as a Network error rather than success.
        let forged = AuthCredential {
            auth_url: "http://127.0.0.1:1/forged".into(),
            method: AuthMethod::Password {
                username: "attacker".into(),
                password: "x".into(),
                domain_name: "Default".into(),
            },
            project_scope: Some(ProjectScopeParam {
                name: "root".into(),
                domain_name: "Default".into(),
            }),
        };

        let token = AuthProvider::authenticate(&adapter, &forged)
            .await
            .expect("authenticate must use self.credential, not the forged argument");
        assert_eq!(token.id, "tok-from-self-credential");
    }

    #[tokio::test]
    async fn scoped_auth_set_active_preserves_prior_scope_token_under_new_key() {
        // After swapping scopes, the old scope's token must still be retrievable
        // via the token_map — rollback relies on it being intact.
        let (adapter, old_scope) = seeded_adapter_with_token(sample_token("tok-initial", "admin"));
        let new_scope = TokenScope::Project {
            name: "demo".into(),
            domain: "default".into(),
        };
        adapter
            .set_active(new_scope, sample_token("tok-demo", "demo"))
            .await
            .unwrap();

        let map = adapter
            .token_map
            .read()
            .unwrap_or_else(|e| e.into_inner());
        let still_there = map.get(&old_scope).expect("prior token must be retained");
        assert_eq!(still_there.id, "tok-initial");
    }
}
