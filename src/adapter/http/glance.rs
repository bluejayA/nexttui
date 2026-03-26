use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{append_pagination_parts, encode_param};
use crate::adapter::http::base::BaseHttpClient;
use crate::models::glance::Image;
use crate::port::auth::AuthProvider;
use crate::port::error::{ApiError, ApiResult};
use crate::port::glance::GlancePort;
use crate::port::types::*;

pub struct GlanceHttpAdapter {
    base: BaseHttpClient,
}

impl GlanceHttpAdapter {
    pub fn new(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Result<Self, ApiError> {
        Ok(Self {
            base: BaseHttpClient::new(auth, "image", EndpointInterface::Public, region)?,
        })
    }
}

// --- JSON wrapper structs ---

#[derive(Deserialize)]
struct GlanceImagesResponse {
    images: Vec<Image>,
    #[serde(default)]
    next: Option<String>,
}

// Glance uses `next` URL field instead of `*_links` pattern.
// We extract marker from the next URL if present.
fn extract_glance_marker(next: Option<&str>) -> Option<String> {
    next.and_then(|url| {
        url.split('?')
            .nth(1)
            .and_then(|query| {
                query
                    .split('&')
                    .find(|p| p.starts_with("marker="))
                    .map(|p| p.trim_start_matches("marker=").to_string())
            })
    })
}

// --- Serialize structs ---

#[derive(Serialize)]
struct ImageCreateBody {
    name: String,
    disk_format: String,
    container_format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    visibility: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_disk: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_ram: Option<u32>,
}

// Glance v2 uses JSON Patch (RFC 6902) for updates, but also accepts
// a simpler JSON merge for common fields. We use the simpler approach.
#[derive(Serialize)]
struct ImageUpdateBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    visibility: Option<String>,
}

// --- Query builders ---

fn build_image_query(filter: &ImageListFilter, pagination: &PaginationParams) -> String {
    let mut parts = Vec::new();
    if let Some(ref name) = filter.name {
        parts.push(format!("name={}", encode_param(name)));
    }
    if let Some(ref status) = filter.status {
        parts.push(format!("status={}", encode_param(status)));
    }
    if let Some(ref vis) = filter.visibility {
        parts.push(format!("visibility={}", encode_param(vis)));
    }
    append_pagination_parts(&mut parts, pagination);
    parts.join("&")
}

// --- GlancePort implementation ---

#[async_trait]
impl GlancePort for GlanceHttpAdapter {
    async fn list_images(
        &self,
        filter: &ImageListFilter,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Image>> {
        let query = build_image_query(filter, pagination);
        let path = if query.is_empty() {
            "/v2/images".to_string()
        } else {
            format!("/v2/images?{query}")
        };
        let req = self.base.get(&path).await?;
        let resp: GlanceImagesResponse = self.base.send_json(req).await?;
        let next_marker = extract_glance_marker(resp.next.as_deref());
        let has_more = next_marker.is_some();
        Ok(PaginatedResponse {
            items: resp.images,
            next_marker,
            has_more,
        })
    }

    async fn get_image(&self, image_id: &str) -> ApiResult<Image> {
        let req = self
            .base
            .get(&format!("/v2/images/{}", encode_param(image_id)))
            .await?;
        let image: Image = self.base.send_json(req).await?;
        Ok(image)
    }

    async fn create_image(&self, params: &ImageCreateParams) -> ApiResult<Image> {
        let body = ImageCreateBody {
            name: params.name.clone(),
            disk_format: params.disk_format.clone(),
            container_format: params.container_format.clone(),
            visibility: params.visibility.clone(),
            min_disk: params.min_disk,
            min_ram: params.min_ram,
        };
        let req = self.base.post("/v2/images").await?.json(&body);
        let image: Image = self.base.send_json(req).await?;
        Ok(image)
    }

    async fn update_image(
        &self,
        image_id: &str,
        params: &ImageUpdateParams,
    ) -> ApiResult<Image> {
        let body = ImageUpdateBody {
            name: params.name.clone(),
            visibility: params.visibility.clone(),
        };
        let req = self
            .base
            .put(&format!("/v2/images/{}", encode_param(image_id)))
            .await?
            .json(&body);
        let image: Image = self.base.send_json(req).await?;
        Ok(image)
    }

    async fn delete_image(&self, image_id: &str) -> ApiResult<()> {
        let req = self
            .base
            .delete(&format!("/v2/images/{}", encode_param(image_id)))
            .await?;
        self.base.send_no_content(req).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glance_images_response_deserialize() {
        let json = r#"{
            "images": [
                {
                    "id": "img-1",
                    "name": "Ubuntu",
                    "status": "active",
                    "disk_format": "qcow2",
                    "container_format": "bare",
                    "size": 2147483648,
                    "visibility": "public",
                    "min_disk": 10,
                    "min_ram": 512,
                    "created_at": "2026-01-01T00:00:00Z"
                }
            ],
            "next": "/v2/images?marker=img-1&limit=50"
        }"#;
        let resp: GlanceImagesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.images.len(), 1);
        assert_eq!(resp.images[0].name, "Ubuntu");
        let marker = extract_glance_marker(resp.next.as_deref());
        assert_eq!(marker, Some("img-1".to_string()));
    }

    #[test]
    fn test_glance_images_no_next() {
        let json = r#"{ "images": [] }"#;
        let resp: GlanceImagesResponse = serde_json::from_str(json).unwrap();
        assert!(resp.images.is_empty());
        assert!(resp.next.is_none());
    }

    #[test]
    fn test_image_create_body_serialize() {
        let body = ImageCreateBody {
            name: "test-img".into(),
            disk_format: "qcow2".into(),
            container_format: "bare".into(),
            visibility: Some("private".into()),
            min_disk: Some(10),
            min_ram: None,
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["name"], "test-img");
        assert_eq!(json["disk_format"], "qcow2");
        assert_eq!(json["visibility"], "private");
        assert_eq!(json["min_disk"], 10);
        assert!(json.get("min_ram").is_none());
    }

    #[test]
    fn test_build_image_query_full() {
        let filter = ImageListFilter {
            name: Some("Ubuntu".into()),
            status: Some("active".into()),
            visibility: Some("public".into()),
        };
        let pagination = PaginationParams {
            marker: Some("img-last".into()),
            limit: Some(50),
            sort_key: None,
            sort_dir: None,
        };
        let query = build_image_query(&filter, &pagination);
        assert!(query.contains("name=Ubuntu"));
        assert!(query.contains("status=active"));
        assert!(query.contains("visibility=public"));
        assert!(query.contains("marker=img-last"));
    }

    #[test]
    fn test_build_image_query_empty() {
        let filter = ImageListFilter::default();
        let pagination = PaginationParams::default();
        assert!(build_image_query(&filter, &pagination).is_empty());
    }

    #[test]
    fn test_extract_glance_marker() {
        assert_eq!(
            extract_glance_marker(Some("/v2/images?marker=abc&limit=50")),
            Some("abc".to_string())
        );
        assert_eq!(extract_glance_marker(None), None);
        assert_eq!(extract_glance_marker(Some("/v2/images")), None);
    }
}
