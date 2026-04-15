pub mod base;
pub mod cinder;
pub mod endpoint_invalidator;
pub mod glance;
pub mod keystone;
pub mod neutron;
pub mod nova;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::adapter::http::base::BaseHttpClient;
use crate::port::error::ApiResult;
use crate::port::types::{PaginatedResponse, PaginationParams, SortDirection};

// --- Shared HTTP helpers (used by nova, neutron, cinder, etc.) ---

#[derive(Deserialize)]
pub(crate) struct Link {
    pub rel: String,
    pub href: String,
}

/// Percent-encode a query parameter value (RFC 3986).
pub(crate) fn encode_param(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

pub(crate) fn append_pagination_parts(parts: &mut Vec<String>, pagination: &PaginationParams) {
    if let Some(ref marker) = pagination.marker {
        parts.push(format!("marker={}", encode_param(marker)));
    }
    if let Some(limit) = pagination.limit {
        parts.push(format!("limit={limit}"));
    }
    if let Some(ref key) = pagination.sort_key {
        parts.push(format!("sort_key={}", encode_param(key)));
    }
    if let Some(ref dir) = pagination.sort_dir {
        let dir_str = match dir {
            SortDirection::Asc => "asc",
            SortDirection::Desc => "desc",
        };
        parts.push(format!("sort_dir={dir_str}"));
    }
}

pub(crate) fn build_pagination_query(pagination: &PaginationParams) -> String {
    let mut parts = Vec::new();
    append_pagination_parts(&mut parts, pagination);
    parts.join("&")
}

pub(crate) fn extract_next_marker(links: &[Link]) -> Option<String> {
    links
        .iter()
        .find(|l| l.rel == "next")
        .and_then(|l| extract_marker_from_url(&l.href))
}

/// Extract `marker=` value from a URL query string.
/// Shared by all marker extraction variants (Link array, Glance next URL, Keystone links).
pub(crate) fn extract_marker_from_url(url: &str) -> Option<String> {
    url.split('?').nth(1).and_then(|query| {
        query
            .split('&')
            .find(|p| p.starts_with("marker="))
            .map(|p| p.trim_start_matches("marker=").to_string())
    })
}

/// Generic paginated list combinator.
///
/// Handles the common pattern: build path + query → GET → deserialize → extract items + marker.
/// The `extract` closure receives the deserialized response and returns (items, next_marker).
pub(crate) async fn paginated_list<T, R, F>(
    base: &BaseHttpClient,
    path: &str,
    query: &str,
    extract: F,
) -> ApiResult<PaginatedResponse<T>>
where
    R: DeserializeOwned,
    F: FnOnce(R) -> (Vec<T>, Option<String>),
{
    let full_path = if query.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{query}")
    };
    let req = base.get(&full_path).await?;
    let resp: R = base.send_json(req).await?;
    let (items, next_marker) = extract(resp);
    let has_more = next_marker.is_some();
    Ok(PaginatedResponse {
        items,
        next_marker,
        has_more,
    })
}
