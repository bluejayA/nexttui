use std::sync::Arc;
use std::time::Duration;

use reqwest::{Method, RequestBuilder, Response};
use serde::de::DeserializeOwned;
use tokio::sync::RwLock;

use crate::port::auth::AuthProvider;
use crate::port::error::{ApiError, ApiResult};
use crate::port::types::EndpointInterface;

/// Shared HTTP plumbing for all service adapters.
/// Auth injection is delegated to AuthProvider::authenticate_request().
///
/// Endpoint caching note: cached endpoint is resolved once and reused.
/// Callers should call `invalidate_endpoint()` when token refresh occurs
/// to pick up potential catalog changes. In Phase 2, BaseHttpClient will
/// subscribe to token refresh broadcast to automate this.
pub struct BaseHttpClient {
    client: reqwest::Client,
    auth: Arc<dyn AuthProvider>,
    service_type: String,
    interface: EndpointInterface,
    region: Option<String>,
    endpoint: RwLock<Option<String>>,
}

impl BaseHttpClient {
    pub fn new(
        auth: Arc<dyn AuthProvider>,
        service_type: &str,
        interface: EndpointInterface,
        region: Option<String>,
    ) -> Result<Self, ApiError> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .connect_timeout(Duration::from_secs(10))
                .build()?,
            auth,
            service_type: service_type.to_string(),
            interface,
            region,
            endpoint: RwLock::new(None),
        })
    }

    /// Resolve and cache the endpoint from service catalog.
    #[tracing::instrument(skip(self), fields(service = %self.service_type))]
    async fn resolve_endpoint(&self) -> ApiResult<String> {
        {
            let cached = self.endpoint.read().await;
            if let Some(url) = cached.as_ref() {
                return Ok(url.clone());
            }
        }
        let url = self
            .auth
            .get_endpoint(&self.service_type, self.interface.clone(), self.region.as_deref())
            .await?;
        let mut cached = self.endpoint.write().await;
        *cached = Some(url.clone());
        Ok(url)
    }

    /// Invalidate cached endpoint. Should be called on token refresh
    /// to pick up potential service catalog changes.
    pub async fn invalidate_endpoint(&self) {
        let mut cached = self.endpoint.write().await;
        *cached = None;
    }

    /// Build an authenticated request.
    /// Note (Phase 2): For signed auth methods (HMAC), authenticate_request()
    /// will need actual headers/body. Currently passes empty values since
    /// Phase 1 only uses X-Auth-Token which doesn't depend on request content.
    async fn request(&self, method: Method, path: &str) -> ApiResult<RequestBuilder> {
        let endpoint = self.resolve_endpoint().await?;
        let url = format!("{}{}", endpoint.trim_end_matches('/'), path);
        let method_str = method.as_str();
        let empty_headers = reqwest::header::HeaderMap::new();
        let auth_headers = self
            .auth
            .authenticate_request(method_str, &url, &empty_headers, None)
            .await?;
        let mut builder = self
            .client
            .request(method, &url)
            .header("Content-Type", "application/json");
        for (key, value) in &auth_headers.headers {
            builder = builder.header(key.as_str(), value.as_str());
        }
        Ok(builder)
    }

    pub async fn get(&self, path: &str) -> ApiResult<RequestBuilder> {
        self.request(Method::GET, path).await
    }

    pub async fn post(&self, path: &str) -> ApiResult<RequestBuilder> {
        self.request(Method::POST, path).await
    }

    pub async fn put(&self, path: &str) -> ApiResult<RequestBuilder> {
        self.request(Method::PUT, path).await
    }

    pub async fn patch(&self, path: &str) -> ApiResult<RequestBuilder> {
        self.request(Method::PATCH, path).await
    }

    pub async fn delete(&self, path: &str) -> ApiResult<RequestBuilder> {
        self.request(Method::DELETE, path).await
    }

    /// Send a request and map HTTP errors to ApiError.
    pub async fn send(&self, request: RequestBuilder) -> ApiResult<Response> {
        let resp = request.send().await.map_err(ApiError::Network)?;
        Self::check_status(resp).await
    }

    /// Send + deserialize JSON body.
    pub async fn send_json<T: DeserializeOwned>(
        &self,
        request: RequestBuilder,
    ) -> ApiResult<T> {
        let resp = self.send(request).await?;
        resp.json::<T>()
            .await
            .map_err(|e| ApiError::Parse(format!("JSON deserialization failed: {e}")))
    }

    /// Send and expect 204 No Content (or 202 Accepted).
    pub async fn send_no_content(&self, request: RequestBuilder) -> ApiResult<()> {
        self.send(request).await?;
        Ok(())
    }

    /// Map HTTP status codes to ApiError.
    pub(crate) async fn check_status(resp: Response) -> ApiResult<Response> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let body = resp.text().await.unwrap_or_default();
        match status.as_u16() {
            401 => Err(ApiError::TokenExpired),
            403 => Err(ApiError::Forbidden(body)),
            404 => Err(ApiError::NotFound {
                resource_type: String::new(),
                id: String::new(),
            }),
            409 => Err(ApiError::Conflict(body)),
            400 => Err(ApiError::BadRequest(body)),
            429 => Err(ApiError::RateLimited {
                retry_after_secs: 60,
            }),
            503 => Err(ApiError::ServiceUnavailable {
                service: String::new(),
            }),
            _ => Err(ApiError::Unexpected {
                status: status.as_u16(),
                body,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::http::StatusCode;

    fn mock_response(status: u16, body: &str) -> Response {
        ::http::Response::builder()
            .status(StatusCode::from_u16(status).unwrap())
            .body(body.to_string())
            .unwrap()
            .into()
    }

    #[tokio::test]
    async fn test_check_status_success() {
        let resp = mock_response(200, r#"{"ok": true}"#);
        let result = BaseHttpClient::check_status(resp).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_check_status_401() {
        let resp = mock_response(401, "unauthorized");
        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
        assert!(matches!(err, ApiError::TokenExpired));
    }

    #[tokio::test]
    async fn test_check_status_403() {
        let resp = mock_response(403, "forbidden");
        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
        assert!(matches!(err, ApiError::Forbidden(_)));
    }

    #[tokio::test]
    async fn test_check_status_404() {
        let resp = mock_response(404, "not found");
        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
        assert!(matches!(err, ApiError::NotFound { .. }));
    }

    #[tokio::test]
    async fn test_check_status_409() {
        let resp = mock_response(409, "conflict");
        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
        assert!(matches!(err, ApiError::Conflict(_)));
    }

    #[tokio::test]
    async fn test_check_status_400() {
        let resp = mock_response(400, "bad request");
        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
        assert!(matches!(err, ApiError::BadRequest(_)));
    }

    #[tokio::test]
    async fn test_check_status_429() {
        let resp = mock_response(429, "rate limited");
        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
        assert!(matches!(err, ApiError::RateLimited { .. }));
    }

    #[tokio::test]
    async fn test_check_status_503() {
        let resp = mock_response(503, "unavailable");
        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
        assert!(matches!(err, ApiError::ServiceUnavailable { .. }));
    }

    #[tokio::test]
    async fn test_check_status_500_unexpected() {
        let resp = mock_response(500, "internal error");
        let err = BaseHttpClient::check_status(resp).await.unwrap_err();
        match err {
            ApiError::Unexpected { status, body } => {
                assert_eq!(status, 500);
                assert_eq!(body, "internal error");
            }
            _ => panic!("expected Unexpected"),
        }
    }
}
