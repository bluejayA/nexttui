//! Keystone v3 rescope adapter. Implements [`KeystoneRescopePort`] by POSTing
//! a token-method auth request scoped to the target project. Kept separate
//! from [`KeystoneAuthAdapter`] so the rescope handshake can be swapped or
//! mocked independently of the broader auth surface.
//!
//! BL-P2-031 Unit 3b.

use async_trait::async_trait;

use crate::context::{ContextTarget, SwitchError};
use crate::port::error::ApiError;
use crate::port::keystone_rescope::KeystoneRescopePort;
use crate::port::types::Token;

pub struct KeystoneRescopeAdapter {
    client: reqwest::Client,
    auth_url: String,
}

impl KeystoneRescopeAdapter {
    pub fn new(client: reqwest::Client, auth_url: String) -> Self {
        Self { client, auth_url }
    }
}

#[async_trait]
impl KeystoneRescopePort for KeystoneRescopeAdapter {
    #[tracing::instrument(skip(self, current_token), fields(project_id = %target.project_id))]
    async fn rescope(
        &self,
        current_token: &Token,
        target: &ContextTarget,
    ) -> Result<Token, SwitchError> {
        let url = format!("{}/auth/tokens", self.auth_url.trim_end_matches('/'));
        let body = build_rescope_body(&current_token.id, target);
        let resp = self
            .client
            .post(&url)
            .header("X-Auth-Token", &current_token.id)
            .json(&body)
            .send()
            .await
            .map_err(|e| SwitchError::Api(ApiError::Network(e)))?;

        let status = resp.status();
        if !status.is_success() {
            let retry_after_secs = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0);
            let body_text = resp.text().await.unwrap_or_default();
            return Err(map_rescope_http_error(
                status,
                body_text,
                target,
                retry_after_secs,
            ));
        }

        let new_token_id = resp
            .headers()
            .get("X-Subject-Token")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| protocol_error("missing X-Subject-Token header"))?
            .to_string();

        let parsed: super::keystone::KeystoneTokenResponse = resp
            .json()
            .await
            .map_err(|e| protocol_error(format!("failed to parse rescope response: {e}")))?;

        let new_token = super::keystone::parse_token(new_token_id, parsed);
        ensure_scope_matches_target(&new_token, target)?;
        Ok(new_token)
    }
}

/// Keystone v3 token-method rescope body. Always scopes by project id since
/// [`ContextTarget`] carries the resolver's authoritative id — using the name
/// would break across concurrent renames.
pub(crate) fn build_rescope_body(
    current_token_id: &str,
    target: &ContextTarget,
) -> serde_json::Value {
    serde_json::json!({
        "auth": {
            "identity": {
                "methods": ["token"],
                "token": { "id": current_token_id }
            },
            "scope": {
                "project": { "id": target.project_id }
            }
        }
    })
}

/// Truncates and scrubs a Keystone error body before it enters [`SwitchError`]
/// messages. Without this, the body would flow through `err.to_string()` into
/// UI toasts and `tracing::warn!` — leaking any echoed `Set-Cookie`,
/// `X-Auth-Token`, or `X-Subject-Token` material that proxies/buggy
/// middleware might surface.
pub(crate) fn sanitize_rescope_body(body: &str) -> String {
    const MAX_CHARS: usize = 256;
    const SENSITIVE: &[&str] = &[
        "X-Auth-Token",
        "X-Subject-Token",
        "Set-Cookie",
        "Authorization",
    ];

    let mut redacted = body.to_string();
    for needle in SENSITIVE {
        while let Some(idx) = redacted.find(needle) {
            let rel = redacted[idx..].find('\n').unwrap_or(redacted.len() - idx);
            redacted.replace_range(idx..idx + rel, "[REDACTED]");
        }
    }

    if redacted.chars().count() > MAX_CHARS {
        let truncated: String = redacted.chars().take(MAX_CHARS).collect();
        format!("{truncated}...[truncated]")
    } else {
        redacted
    }
}

/// Wire-contract violations (missing headers, unparseable body) are protocol
/// failures, not policy decisions. Surfacing them as [`ApiError::Parse`]
/// keeps [`SwitchError::RescopeRejected`] reserved for explicit Keystone RBAC
/// rejections so callers can distinguish "will not retry" from "retry after
/// investigating the transport".
pub(crate) fn protocol_error(reason: impl Into<String>) -> SwitchError {
    SwitchError::Api(ApiError::Parse(reason.into()))
}

/// Splits non-2xx responses by Keystone semantics so the caller can pick the
/// right retry policy:
/// - **401/403** → [`SwitchError::RescopeRejected`]: policy decision, retry is
///   pointless without re-authentication.
/// - **404** → [`SwitchError::NotFound`]: target project/endpoint gone, retry
///   after resolver refresh.
/// - **429** → [`ApiError::RateLimited`]: back off, retry later.
/// - **5xx** → [`ApiError::ServiceUnavailable`]: transient, retry with backoff.
/// - **400** → [`ApiError::BadRequest`]: caller built a bad request; no retry.
/// - **other** → [`ApiError::Unexpected`]: unknown status, preserve for audit.
pub(crate) fn map_rescope_http_error(
    status: reqwest::StatusCode,
    body: String,
    target: &ContextTarget,
    retry_after_secs: u64,
) -> SwitchError {
    let body = sanitize_rescope_body(&body);
    match status.as_u16() {
        401 | 403 => SwitchError::RescopeRejected(format!("{status}: {body}")),
        404 => SwitchError::NotFound(format!(
            "project {} ({}) — keystone: {}",
            target.project_name, target.project_id, body,
        )),
        400 => SwitchError::Api(ApiError::BadRequest(format!("{status}: {body}"))),
        429 => SwitchError::Api(ApiError::RateLimited { retry_after_secs }),
        500..=599 => SwitchError::Api(ApiError::ServiceUnavailable {
            service: format!("keystone ({status}): {body}"),
        }),
        code => SwitchError::Api(ApiError::Unexpected { status: code, body }),
    }
}

/// Protects against the "Keystone returned 200 but scoped to a different
/// project" quirk: commit only if the rescoped token actually names the
/// target project. Surfaces mismatches as [`ApiError::Parse`] since the
/// contract violation is protocol-level, not a policy decision.
pub(crate) fn ensure_scope_matches_target(
    token: &Token,
    target: &ContextTarget,
) -> Result<(), SwitchError> {
    if token.project.id == target.project_id && !token.project.id.is_empty() {
        Ok(())
    } else {
        Err(SwitchError::Api(ApiError::Parse(format!(
            "rescope response scoped to project {:?} but target was {:?}",
            token.project.id, target.project_id,
        ))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::port::types::{ProjectScope, Token};
    use chrono::{TimeZone, Utc};

    fn sample_target() -> ContextTarget {
        ContextTarget {
            cloud: "devstack".into(),
            project_id: "proj-xyz".into(),
            project_name: "demo".into(),
            domain: "default".into(),
        }
    }

    fn sample_current_token() -> Token {
        Token {
            id: "tok-current".into(),
            expires_at: Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap(),
            project: ProjectScope {
                id: "proj-admin".into(),
                name: "admin".into(),
                domain_id: "default".into(),
                domain_name: "default".into(),
            },
            roles: Vec::new(),
            catalog: Vec::new(),
        }
    }

    #[test]
    fn build_rescope_body_uses_token_method_with_project_id() {
        let body = build_rescope_body(&sample_current_token().id, &sample_target());

        assert_eq!(body["auth"]["identity"]["methods"][0], "token");
        assert_eq!(body["auth"]["identity"]["token"]["id"], "tok-current");
        assert_eq!(body["auth"]["scope"]["project"]["id"], "proj-xyz");
        // Project name must NOT be the scope key — id is authoritative so
        // rescope survives project renames between discovery and rescope.
        assert!(body["auth"]["scope"]["project"]["name"].is_null());
    }

    #[test]
    fn sanitize_body_passes_through_short_clean_body() {
        let out = sanitize_rescope_body("Keystone policy denied");
        assert_eq!(out, "Keystone policy denied");
    }

    #[test]
    fn sanitize_body_truncates_long_input() {
        let input = "a".repeat(1_024);
        let out = sanitize_rescope_body(&input);
        assert!(
            out.chars().count() <= 300,
            "expected truncated output, got {} chars",
            out.chars().count()
        );
        assert!(
            out.contains("truncated"),
            "truncation marker missing: {out}"
        );
    }

    #[test]
    fn sanitize_body_preserves_multibyte_characters() {
        // Slicing on bytes would panic on "한" boundaries; truncation must be char-safe.
        let input: String = "한글 테스트 ".repeat(100);
        let out = sanitize_rescope_body(&input);
        // Just assert it doesn't panic and stays UTF-8 valid.
        assert!(out.chars().count() <= 300);
    }

    #[test]
    fn sanitize_body_redacts_x_auth_token_value() {
        let input = "error: X-Auth-Token: gAAAAABsecretMATERIAL details here";
        let out = sanitize_rescope_body(input);
        assert!(
            !out.contains("gAAAAABsecretMATERIAL"),
            "token material leaked: {out}"
        );
        assert!(
            out.contains("[REDACTED]"),
            "expected redaction marker in {out}"
        );
    }

    #[test]
    fn sanitize_body_redacts_x_subject_token_value() {
        let input = "X-Subject-Token: abcXYZsecret\n(other info)";
        let out = sanitize_rescope_body(input);
        assert!(!out.contains("abcXYZsecret"));
    }

    #[test]
    fn sanitize_body_redacts_set_cookie_value() {
        let input = "Set-Cookie: session=deadbeef; HttpOnly";
        let out = sanitize_rescope_body(input);
        assert!(!out.contains("deadbeef"));
    }

    #[test]
    fn sanitize_body_applied_inside_map_rescope_http_error_404() {
        let target = sample_target();
        let err = map_rescope_http_error(
            reqwest::StatusCode::NOT_FOUND,
            "not found X-Auth-Token: LEAKED_VALUE trailing".into(),
            &target,
            0,
        );
        let msg = err.to_string();
        assert!(
            !msg.contains("LEAKED_VALUE"),
            "token leaked through NotFound: {msg}"
        );
    }

    #[test]
    fn protocol_error_surfaces_as_api_parse() {
        let err = protocol_error("missing X-Subject-Token");
        match err {
            SwitchError::Api(ApiError::Parse(msg)) => {
                assert!(msg.contains("missing X-Subject-Token"));
            }
            other => panic!("expected Api(Parse), got {other:?}"),
        }
    }

    #[test]
    fn http_error_403_maps_to_rescope_rejected() {
        let err = map_rescope_http_error(
            reqwest::StatusCode::FORBIDDEN,
            "policy denied".into(),
            &sample_target(),
            0,
        );
        match err {
            SwitchError::RescopeRejected(msg) => {
                assert!(msg.contains("403"));
                assert!(msg.contains("policy denied"));
            }
            other => panic!("expected RescopeRejected, got {other:?}"),
        }
    }

    #[test]
    fn http_error_401_maps_to_rescope_rejected() {
        let err = map_rescope_http_error(
            reqwest::StatusCode::UNAUTHORIZED,
            "token expired".into(),
            &sample_target(),
            0,
        );
        assert!(matches!(err, SwitchError::RescopeRejected(_)));
    }

    #[test]
    fn http_error_404_maps_to_not_found_with_target_info() {
        let target = sample_target();
        let err = map_rescope_http_error(
            reqwest::StatusCode::NOT_FOUND,
            "project not found".into(),
            &target,
            0,
        );
        match err {
            SwitchError::NotFound(msg) => {
                assert!(
                    msg.contains(&target.project_id) || msg.contains(&target.project_name),
                    "404 message should identify the target; got {msg}"
                );
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn http_error_400_maps_to_bad_request() {
        let err = map_rescope_http_error(
            reqwest::StatusCode::BAD_REQUEST,
            "invalid scope".into(),
            &sample_target(),
            0,
        );
        assert!(matches!(err, SwitchError::Api(ApiError::BadRequest(_))));
    }

    #[test]
    fn http_error_429_maps_to_rate_limited_with_retry_after() {
        let err = map_rescope_http_error(
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            "slow down".into(),
            &sample_target(),
            42,
        );
        match err {
            SwitchError::Api(ApiError::RateLimited { retry_after_secs }) => {
                assert_eq!(retry_after_secs, 42);
            }
            other => panic!("expected Api(RateLimited{{42}}), got {other:?}"),
        }
    }

    #[test]
    fn http_error_500_maps_to_service_unavailable() {
        let err = map_rescope_http_error(
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            "upstream crashed".into(),
            &sample_target(),
            0,
        );
        assert!(matches!(
            err,
            SwitchError::Api(ApiError::ServiceUnavailable { .. })
        ));
    }

    #[test]
    fn http_error_503_maps_to_service_unavailable() {
        let err = map_rescope_http_error(
            reqwest::StatusCode::SERVICE_UNAVAILABLE,
            "overloaded".into(),
            &sample_target(),
            0,
        );
        assert!(matches!(
            err,
            SwitchError::Api(ApiError::ServiceUnavailable { .. })
        ));
    }

    #[test]
    fn http_error_other_maps_to_unexpected_preserving_status() {
        let err = map_rescope_http_error(
            reqwest::StatusCode::IM_A_TEAPOT,
            "?".into(),
            &sample_target(),
            0,
        );
        match err {
            SwitchError::Api(ApiError::Unexpected { status, .. }) => {
                assert_eq!(status, 418);
            }
            other => panic!("expected Api(Unexpected{{418}}), got {other:?}"),
        }
    }

    fn token_with_project_id(id: &str) -> Token {
        let mut t = sample_current_token();
        t.project.id = id.to_string();
        t
    }

    #[test]
    fn ensure_scope_matches_target_accepts_matching_project_id() {
        let target = sample_target();
        let token = token_with_project_id(&target.project_id);
        assert!(ensure_scope_matches_target(&token, &target).is_ok());
    }

    #[test]
    fn ensure_scope_matches_target_rejects_mismatched_project_id() {
        let target = sample_target();
        let token = token_with_project_id("proj-DIFFERENT");
        let err = ensure_scope_matches_target(&token, &target).unwrap_err();
        match err {
            SwitchError::Api(ApiError::Parse(msg)) => {
                assert!(msg.contains("proj-DIFFERENT"));
                assert!(msg.contains(&target.project_id));
            }
            other => panic!("expected Api(Parse), got {other:?}"),
        }
    }

    #[test]
    fn ensure_scope_matches_target_rejects_empty_project_id() {
        let target = sample_target();
        let token = token_with_project_id("");
        let err = ensure_scope_matches_target(&token, &target).unwrap_err();
        assert!(matches!(err, SwitchError::Api(ApiError::Parse(_))));
    }

    #[test]
    fn adapter_constructs_with_client_and_url() {
        let client = reqwest::Client::new();
        let adapter = KeystoneRescopeAdapter::new(client, "https://keystone/v3".into());
        assert_eq!(adapter.auth_url, "https://keystone/v3");
    }

    // --- Loopback HTTP integration tests ---
    //
    // A minimal one-shot HTTP responder backed by [`tokio::net::TcpListener`]
    // lets us exercise the real `rescope()` wire path (request send, header
    // inspection, body parse) without pulling in `wiremock` as a dev-dep.
    // The responder accepts exactly one request per invocation and serves the
    // canned response synchronously.

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    struct CannedResponse {
        status_line: &'static str,
        extra_headers: String,
        body: String,
    }

    async fn spawn_one_shot_server(resp: CannedResponse) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");
        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            // Drain the request until \r\n\r\n (headers end) plus the declared body.
            // For this test we only need enough bytes off the wire that reqwest's
            // write doesn't block — reading once is sufficient.
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

    fn rescope_success_body_for(target: &ContextTarget, expires_at: &str) -> String {
        format!(
            r#"{{"token":{{"expires_at":"{expires_at}","project":{{"id":"{pid}","name":"{pname}","domain":{{"id":"{did}","name":"{dname}"}}}},"roles":[{{"id":"r1","name":"member"}}],"catalog":[]}}}}"#,
            pid = target.project_id,
            pname = target.project_name,
            did = target.domain,
            dname = target.domain,
            expires_at = expires_at,
        )
    }

    #[tokio::test]
    async fn rescope_happy_path_returns_token_with_original_expires_at() {
        let target = sample_target();
        let expires = "2030-06-01T00:00:00Z";
        let body = rescope_success_body_for(&target, expires);
        let resp = CannedResponse {
            status_line: "200 OK",
            extra_headers: "X-Subject-Token: new-scoped-tok\r\n".into(),
            body,
        };
        let (base_url, _handle) = spawn_one_shot_server(resp).await;

        let adapter = KeystoneRescopeAdapter::new(reqwest::Client::new(), format!("{base_url}/v3"));
        let new_token = adapter
            .rescope(&sample_current_token(), &target)
            .await
            .unwrap();

        assert_eq!(new_token.id, "new-scoped-tok");
        assert_eq!(new_token.project.id, target.project_id);
        // expires_at must be Keystone's response verbatim — no TTL inference.
        assert_eq!(
            new_token.expires_at,
            chrono::DateTime::parse_from_rfc3339(expires).unwrap(),
        );
    }

    #[tokio::test]
    async fn rescope_rejects_response_with_mismatched_project_id() {
        let target = sample_target();
        // Keystone returns a token scoped to a DIFFERENT project (e.g. because
        // of a server-side bug or proxy replay). The adapter must refuse it
        // rather than silently commit the wrong scope.
        let mut wrong_target = target.clone();
        wrong_target.project_id = "proj-ATTACKER".into();
        let body = rescope_success_body_for(&wrong_target, "2030-06-01T00:00:00Z");
        let resp = CannedResponse {
            status_line: "200 OK",
            extra_headers: "X-Subject-Token: tok-wrong-scope\r\n".into(),
            body,
        };
        let (base_url, _handle) = spawn_one_shot_server(resp).await;

        let adapter = KeystoneRescopeAdapter::new(reqwest::Client::new(), format!("{base_url}/v3"));
        let err = adapter
            .rescope(&sample_current_token(), &target)
            .await
            .unwrap_err();

        assert!(
            matches!(err, SwitchError::Api(ApiError::Parse(_))),
            "expected Api(Parse) for scope mismatch, got {err:?}",
        );
    }

    #[tokio::test]
    async fn rescope_403_surfaces_as_rescope_rejected() {
        let resp = CannedResponse {
            status_line: "403 Forbidden",
            extra_headers: String::new(),
            body: r#"{"error":{"code":403,"message":"policy denied"}}"#.into(),
        };
        let (base_url, _handle) = spawn_one_shot_server(resp).await;

        let adapter = KeystoneRescopeAdapter::new(reqwest::Client::new(), format!("{base_url}/v3"));
        let err = adapter
            .rescope(&sample_current_token(), &sample_target())
            .await
            .unwrap_err();

        assert!(
            matches!(err, SwitchError::RescopeRejected(_)),
            "got {err:?}"
        );
    }

    #[tokio::test]
    async fn rescope_missing_x_subject_token_is_protocol_error() {
        let target = sample_target();
        let body = rescope_success_body_for(&target, "2030-06-01T00:00:00Z");
        let resp = CannedResponse {
            status_line: "200 OK",
            extra_headers: String::new(), // no X-Subject-Token
            body,
        };
        let (base_url, _handle) = spawn_one_shot_server(resp).await;

        let adapter = KeystoneRescopeAdapter::new(reqwest::Client::new(), format!("{base_url}/v3"));
        let err = adapter
            .rescope(&sample_current_token(), &target)
            .await
            .unwrap_err();

        match err {
            SwitchError::Api(ApiError::Parse(msg)) => {
                assert!(msg.to_ascii_lowercase().contains("x-subject-token"));
            }
            other => panic!("expected Api(Parse) for header miss, got {other:?}"),
        }
    }
}
