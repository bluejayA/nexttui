use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{build_pagination_query, encode_param, extract_marker_from_url, paginated_list};
use crate::adapter::http::base::BaseHttpClient;
use crate::models::keystone::{Project, Role, RoleAssignment, User};
use crate::port::auth::AuthProvider;
use crate::port::error::{ApiError, ApiResult};
use crate::port::keystone::KeystonePort;
use crate::port::types::*;

pub struct KeystoneHttpAdapter {
    base: Arc<BaseHttpClient>,
}

impl KeystoneHttpAdapter {
    pub fn new(auth: Arc<dyn AuthProvider>, region: Option<String>) -> Result<Self, ApiError> {
        Ok(Self {
            base: Arc::new(BaseHttpClient::new(
                auth,
                "identity",
                EndpointInterface::Public,
                region,
            )?),
        })
    }

    pub fn from_base(base: Arc<BaseHttpClient>) -> Self {
        Self { base }
    }
}

// --- JSON wrapper structs ---

#[derive(Deserialize)]
struct KeystoneProjectsResponse {
    projects: Vec<Project>,
    links: Option<KeystoneLinks>,
}

#[derive(Deserialize)]
struct KeystoneProjectWrapper {
    project: Project,
}

#[derive(Deserialize)]
struct KeystoneUsersResponse {
    users: Vec<User>,
    links: Option<KeystoneLinks>,
}

#[derive(Deserialize)]
struct KeystoneUserWrapper {
    user: User,
}

#[derive(Deserialize)]
struct KeystoneRolesResponse {
    roles: Vec<Role>,
}

#[derive(Deserialize)]
struct KeystoneRoleAssignmentsResponse {
    role_assignments: Vec<RoleAssignment>,
}

#[derive(Deserialize)]
struct KeystoneDomainsResponse {
    domains: Vec<Domain>,
}

// Keystone uses {"links": {"next": "url", "previous": "url"}} pattern
#[derive(Deserialize)]
struct KeystoneLinks {
    next: Option<String>,
}

fn extract_keystone_marker(links: &Option<KeystoneLinks>) -> Option<String> {
    links
        .as_ref()
        .and_then(|l| l.next.as_deref().and_then(extract_marker_from_url))
}

// --- Serialize structs ---

#[derive(Serialize)]
struct ProjectCreateBody {
    project: ProjectCreateInner,
}

#[derive(Serialize)]
struct ProjectCreateInner {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    domain_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
}

#[derive(Serialize)]
struct ProjectUpdateBody {
    project: ProjectUpdateInner,
}

#[derive(Serialize)]
struct ProjectUpdateInner {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
}

#[derive(Serialize)]
struct UserCreateBody {
    user: UserCreateInner,
}

#[derive(Serialize)]
struct UserCreateInner {
    name: String,
    password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_project_id: Option<String>,
    domain_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
}

#[derive(Serialize)]
struct UserUpdateBody {
    user: UserUpdateInner,
}

#[derive(Serialize)]
struct UserUpdateInner {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
}

// --- KeystonePort implementation ---

#[async_trait]
impl KeystonePort for KeystoneHttpAdapter {
    // -- Projects --

    async fn list_projects(
        &self,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Project>> {
        let query = build_pagination_query(pagination);
        paginated_list(
            &self.base,
            "/v3/projects",
            &query,
            |resp: KeystoneProjectsResponse| {
                let next = extract_keystone_marker(&resp.links);
                (resp.projects, next)
            },
        )
        .await
    }

    async fn get_project(&self, project_id: &str) -> ApiResult<Project> {
        let req = self
            .base
            .get(&format!("/v3/projects/{}", encode_param(project_id)))
            .await?;
        let resp: KeystoneProjectWrapper = self.base.send_json(req).await?;
        Ok(resp.project)
    }

    async fn create_project(&self, params: &ProjectCreateParams) -> ApiResult<Project> {
        let body = ProjectCreateBody {
            project: ProjectCreateInner {
                name: params.name.clone(),
                description: params.description.clone(),
                domain_id: params.domain_id.clone(),
                enabled: params.enabled,
            },
        };
        let req = self.base.post("/v3/projects").await?.json(&body);
        let resp: KeystoneProjectWrapper = self.base.send_json(req).await?;
        Ok(resp.project)
    }

    async fn update_project(
        &self,
        project_id: &str,
        params: &ProjectUpdateParams,
    ) -> ApiResult<Project> {
        let body = ProjectUpdateBody {
            project: ProjectUpdateInner {
                name: params.name.clone(),
                description: params.description.clone(),
                enabled: params.enabled,
            },
        };
        let req = self
            .base
            .put(&format!("/v3/projects/{}", encode_param(project_id)))
            .await?
            .json(&body);
        let resp: KeystoneProjectWrapper = self.base.send_json(req).await?;
        Ok(resp.project)
    }

    async fn delete_project(&self, project_id: &str) -> ApiResult<()> {
        let req = self
            .base
            .delete(&format!("/v3/projects/{}", encode_param(project_id)))
            .await?;
        self.base.send_no_content(req).await
    }

    // -- Users --

    async fn list_users(
        &self,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<User>> {
        let query = build_pagination_query(pagination);
        paginated_list(
            &self.base,
            "/v3/users",
            &query,
            |resp: KeystoneUsersResponse| {
                let next = extract_keystone_marker(&resp.links);
                (resp.users, next)
            },
        )
        .await
    }

    async fn get_user(&self, user_id: &str) -> ApiResult<User> {
        let req = self
            .base
            .get(&format!("/v3/users/{}", encode_param(user_id)))
            .await?;
        let resp: KeystoneUserWrapper = self.base.send_json(req).await?;
        Ok(resp.user)
    }

    async fn create_user(&self, params: &UserCreateParams) -> ApiResult<User> {
        let body = UserCreateBody {
            user: UserCreateInner {
                name: params.name.clone(),
                password: params.password.clone(),
                email: params.email.clone(),
                default_project_id: params.default_project_id.clone(),
                domain_id: params.domain_id.clone(),
                enabled: params.enabled,
            },
        };
        let req = self.base.post("/v3/users").await?.json(&body);
        let resp: KeystoneUserWrapper = self.base.send_json(req).await?;
        Ok(resp.user)
    }

    async fn update_user(&self, user_id: &str, params: &UserUpdateParams) -> ApiResult<User> {
        let body = UserUpdateBody {
            user: UserUpdateInner {
                name: params.name.clone(),
                password: params.password.clone(),
                email: params.email.clone(),
                enabled: params.enabled,
            },
        };
        let req = self
            .base
            .put(&format!("/v3/users/{}", encode_param(user_id)))
            .await?
            .json(&body);
        let resp: KeystoneUserWrapper = self.base.send_json(req).await?;
        Ok(resp.user)
    }

    async fn delete_user(&self, user_id: &str) -> ApiResult<()> {
        let req = self
            .base
            .delete(&format!("/v3/users/{}", encode_param(user_id)))
            .await?;
        self.base.send_no_content(req).await
    }

    // -- Roles --

    async fn list_roles(&self) -> ApiResult<Vec<Role>> {
        let req = self.base.get("/v3/roles").await?;
        let resp: KeystoneRolesResponse = self.base.send_json(req).await?;
        Ok(resp.roles)
    }

    async fn assign_role(&self, params: &RoleAssignmentParams) -> ApiResult<()> {
        let path = format!(
            "/v3/projects/{}/users/{}/roles/{}",
            encode_param(&params.project_id),
            encode_param(&params.user_id),
            encode_param(&params.role_id),
        );
        let req = self.base.put(&path).await?;
        self.base.send_no_content(req).await
    }

    async fn revoke_role(&self, params: &RoleAssignmentParams) -> ApiResult<()> {
        let path = format!(
            "/v3/projects/{}/users/{}/roles/{}",
            encode_param(&params.project_id),
            encode_param(&params.user_id),
            encode_param(&params.role_id),
        );
        let req = self.base.delete(&path).await?;
        self.base.send_no_content(req).await
    }

    async fn list_role_assignments(
        &self,
        filter: &RoleAssignmentFilter,
    ) -> ApiResult<Vec<RoleAssignment>> {
        let mut parts = Vec::new();
        if let Some(ref uid) = filter.user_id {
            parts.push(format!("user.id={}", encode_param(uid)));
        }
        if let Some(ref pid) = filter.project_id {
            parts.push(format!("scope.project.id={}", encode_param(pid)));
        }
        if let Some(ref rid) = filter.role_id {
            parts.push(format!("role.id={}", encode_param(rid)));
        }
        let query = parts.join("&");
        let path = if query.is_empty() {
            "/v3/role_assignments".to_string()
        } else {
            format!("/v3/role_assignments?{query}")
        };
        let req = self.base.get(&path).await?;
        let resp: KeystoneRoleAssignmentsResponse = self.base.send_json(req).await?;
        Ok(resp.role_assignments)
    }

    // -- Domains --

    async fn list_domains(&self) -> ApiResult<Vec<Domain>> {
        let req = self.base.get("/v3/domains").await?;
        let resp: KeystoneDomainsResponse = self.base.send_json(req).await?;
        Ok(resp.domains)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keystone_projects_response_deserialize() {
        let json = r#"{
            "projects": [
                {"id": "proj-1", "name": "admin", "enabled": true}
            ],
            "links": {"next": "http://keystone/v3/projects?marker=proj-1&limit=50", "previous": null}
        }"#;
        let resp: KeystoneProjectsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.projects.len(), 1);
        let marker = extract_keystone_marker(&resp.links);
        assert_eq!(marker, Some("proj-1".to_string()));
    }

    #[test]
    fn test_keystone_users_response_deserialize() {
        let json = r#"{
            "users": [
                {"id": "user-1", "name": "admin", "enabled": true}
            ],
            "links": {"next": null}
        }"#;
        let resp: KeystoneUsersResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.users.len(), 1);
        let marker = extract_keystone_marker(&resp.links);
        assert!(marker.is_none());
    }

    #[test]
    fn test_project_create_body_serialize() {
        let body = ProjectCreateBody {
            project: ProjectCreateInner {
                name: "test-proj".into(),
                description: Some("Test".into()),
                domain_id: "default".into(),
                enabled: None,
            },
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["project"]["name"], "test-proj");
        assert_eq!(json["project"]["domain_id"], "default");
        assert!(json["project"].get("enabled").is_none());
    }

    #[test]
    fn test_user_create_body_serialize() {
        let body = UserCreateBody {
            user: UserCreateInner {
                name: "testuser".into(),
                password: "secret123".into(),
                email: Some("test@example.com".into()),
                default_project_id: None,
                domain_id: "default".into(),
                enabled: Some(true),
            },
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["user"]["name"], "testuser");
        assert_eq!(json["user"]["password"], "secret123");
        assert_eq!(json["user"]["email"], "test@example.com");
        assert!(json["user"].get("default_project_id").is_none());
    }

    #[test]
    fn test_keystone_roles_response_deserialize() {
        let json =
            r#"{"roles": [{"id": "r-1", "name": "admin"}, {"id": "r-2", "name": "member"}]}"#;
        let resp: KeystoneRolesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.roles.len(), 2);
    }

    #[test]
    fn test_extract_keystone_marker_none() {
        let links = Some(KeystoneLinks { next: None });
        assert!(extract_keystone_marker(&links).is_none());
        assert!(extract_keystone_marker(&None).is_none());
    }
}
