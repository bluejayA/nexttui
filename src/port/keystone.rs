use async_trait::async_trait;

use super::error::ApiResult;
use super::types::*;
use crate::models::keystone::{Project, Role, RoleAssignment, User};

#[async_trait]
pub trait KeystonePort: Send + Sync {
    // Projects
    async fn list_projects(
        &self,
        pagination: &PaginationParams,
    ) -> ApiResult<PaginatedResponse<Project>>;
    async fn get_project(&self, project_id: &str) -> ApiResult<Project>;
    async fn create_project(&self, params: &ProjectCreateParams) -> ApiResult<Project>;
    async fn update_project(
        &self,
        project_id: &str,
        params: &ProjectUpdateParams,
    ) -> ApiResult<Project>;
    async fn delete_project(&self, project_id: &str) -> ApiResult<()>;

    // Users
    async fn list_users(&self, pagination: &PaginationParams)
    -> ApiResult<PaginatedResponse<User>>;
    async fn get_user(&self, user_id: &str) -> ApiResult<User>;
    async fn create_user(&self, params: &UserCreateParams) -> ApiResult<User>;
    async fn update_user(&self, user_id: &str, params: &UserUpdateParams) -> ApiResult<User>;
    async fn delete_user(&self, user_id: &str) -> ApiResult<()>;

    // Roles
    async fn list_roles(&self) -> ApiResult<Vec<Role>>;
    async fn assign_role(&self, params: &RoleAssignmentParams) -> ApiResult<()>;
    async fn revoke_role(&self, params: &RoleAssignmentParams) -> ApiResult<()>;
    async fn list_role_assignments(
        &self,
        filter: &RoleAssignmentFilter,
    ) -> ApiResult<Vec<RoleAssignment>>;

    // Domains
    async fn list_domains(&self) -> ApiResult<Vec<Domain>>;
}
