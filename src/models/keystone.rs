use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub domain_id: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub default_project_id: Option<String>,
    pub domain_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Role {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoleAssignment {
    pub role: Role,
    pub user: Option<UserRef>,
    pub scope: Option<Scope>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserRef {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Scope {
    pub project: Option<ProjectRef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectRef {
    pub id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_deserialize() {
        let json = r#"{
            "id": "proj-001",
            "name": "my-project",
            "description": "Test project",
            "enabled": true,
            "domain_id": "default"
        }"#;
        let proj: Project = serde_json::from_str(json).unwrap();
        assert_eq!(proj.name, "my-project");
        assert!(proj.enabled);
    }

    #[test]
    fn test_user_deserialize() {
        let json = r#"{
            "id": "user-001",
            "name": "admin",
            "email": "admin@example.com",
            "enabled": true,
            "default_project_id": "proj-001",
            "domain_id": "default"
        }"#;
        let user: User = serde_json::from_str(json).unwrap();
        assert_eq!(user.name, "admin");
        assert_eq!(user.email.as_deref(), Some("admin@example.com"));
    }
}
