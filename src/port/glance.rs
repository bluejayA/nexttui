use async_trait::async_trait;

use super::error::ApiResult;
use super::types::*;
use crate::models::glance::Image;

#[async_trait]
pub trait GlancePort: Send + Sync {
    async fn list_images(
        &self,
        filter: &ImageListFilter,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Image>>;
    async fn get_image(&self, image_id: &str) -> ApiResult<Image>;
    async fn create_image(&self, params: &ImageCreateParams) -> ApiResult<Image>;
    async fn update_image(&self, image_id: &str, params: &ImageUpdateParams) -> ApiResult<Image>;
    async fn delete_image(&self, image_id: &str) -> ApiResult<()>;
}
