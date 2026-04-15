use super::*;

// Permission utilities
pub struct PermissionUtils;

impl PermissionUtils {
    pub fn get_resource_permissions(resource: &str) -> Vec<String> {
        match resource {
            RESOURCE_DASHBOARD => vec![
                ACTION_READ,
                ACTION_UPDATE,
            ],
            RESOURCE_AGENTS => vec![
                ACTION_READ,
                ACTION_EXECUTE,
                ACTION_UPDATE,
                ACTION_DELETE,
            ],
            RESOURCE_WEBHOOKS => vec![
                ACTION_READ,
                ACTION_CREATE,
                ACTION_UPDATE,
                ACTION_DELETE,
            ],
            RESOURCE_USERS => vec![
                ACTION_READ,
                ACTION_CREATE,
                ACTION_UPDATE,
                ACTION_DELETE,
            ],
            RESOURCE_BILLING => vec![
                ACTION_READ,
                ACTION_UPDATE,
                ACTION_DELETE,
            ],
            RESOURCE_SETTINGS => vec![
                ACTION_READ,
                ACTION_UPDATE,
            ],
            RESOURCE_LOGS => vec![
                ACTION_READ,
                ACTION_UPDATE,
                ACTION_DELETE,
            ],
            RESOURCE_REPORTS => vec![
                ACTION_READ,
                ACTION_UPDATE,
                ACTION_DELETE,
            ],
            _ => vec![],
        }
    }

    pub fn get_all_resources() -> Vec<String> {
        vec![
            RESOURCE_DASHBOARD,
            RESOURCE_AGENTS,
            RESOURCE_WEBHOOKS,
            RESOURCE_USERS,
            RESOURCE_BILLING,
            RESOURCE_SETTINGS,
            RESOURCE_LOGS,
            RESOURCE_REPORTS,
        ]
    }

    pub fn get_all_actions() -> Vec<String> {
        vec![
            ACTION_READ,
            ACTION_WRITE,
            ACTION_UPDATE,
            ACTION_DELETE,
            ACTION_CREATE,
            ACTION_APPROVE,
            ACTION_EXECUTE,
            ACTION_MANAGE,
        ]
    }

    pub fn get_all_roles() -> Vec<Role> {
        RoleManager::create_standard_roles()
    }

    pub fn get_all_policies() -> Vec<Policy> {
        RoleManager::create_standard_policies()
    }
}

// Permission levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionLevel {
    None,
    Read,
    Write,
    Admin,
}

impl PermissionLevel {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "read" => PermissionLevel::Read,
            "write" => PermissionLevel::Write,
            "admin" => PermissionLevel::Admin,
            _ => PermissionLevel::None,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            PermissionLevel::None => "none".to_string(),
            PermissionLevel::Read => "read".to_string(),
            PermissionLevel::Write => "write".to_string(),
            PermissionLevel::Admin => "admin".to_string(),
        }
    }

    pub fn to_grant_level(&self) -> GrantLevel {
        match self {
            PermissionLevel::None => GrantLevel::NoAccess,
            PermissionLevel::Read => GrantLevel::ReadOnly,
            PermissionLevel::Write => GrantLevel::ReadWrite,
            PermissionLevel::Admin => GrantLevel::FullAccess,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantLevel {
    NoAccess,
    ReadOnly,
    ReadWrite,
    FullAccess,
}