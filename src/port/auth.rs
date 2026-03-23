use async_trait::async_trait;
use tokio::sync::broadcast;

use super::error::ApiResult;
use super::types::*;

#[async_trait]
pub trait AuthProvider: Send + Sync {
    async fn authenticate(&self, credential: &AuthCredential) -> ApiResult<Token>;
    async fn refresh_token(&self) -> ApiResult<Token>;
    async fn get_token(&self) -> ApiResult<String>;
    async fn get_token_info(&self) -> ApiResult<Token>;
    async fn authenticate_request(
        &self,
        method: &str,
        url: &str,
        headers: &reqwest::header::HeaderMap,
        body: Option<&[u8]>,
    ) -> ApiResult<AuthHeaders>;
    async fn get_endpoint(
        &self,
        service_type: &str,
        interface: EndpointInterface,
        region: Option<&str>,
    ) -> ApiResult<String>;
    fn subscribe_token_refresh(&self) -> broadcast::Receiver<Token>;
    async fn has_role(&self, role_name: &str) -> ApiResult<bool>;
    async fn get_catalog(&self) -> ApiResult<Vec<CatalogEntry>>;
    async fn get_capabilities(&self) -> ApiResult<Vec<Capability>>;
}
