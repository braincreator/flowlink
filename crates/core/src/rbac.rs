// RBAC — Role-Based Access Control types
// Multi-tenant access control for FlowLink

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash, Eq)]
pub enum Role {
    Admin,
    Operator,
    Viewer,
    Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash, Eq)]
pub enum Permission {
    // Agent management
    AgentRegister,
    AgentList,
    AgentRemove,
    // Command execution
    CommandExecute,
    CommandExecuteDestructive,
    CommandApprove,
    // File operations
    FileRead,
    FileWrite,
    FileDelete,
    // Shield
    ShieldView,
    ShieldApprove,
    ShieldReject,
    ShieldConfigure,
    // System
    MetricsView,
    AuditLogView,
    UserManage,
    PolicyManage,
    // Backup
    BackupCreate,
    BackupRestore,
    BackupDelete,
}

impl Permission {
    pub fn all() -> HashSet<Permission> {
        use Permission::*;
        [
            AgentRegister, AgentList, AgentRemove,
            CommandExecute, CommandExecuteDestructive, CommandApprove,
            FileRead, FileWrite, FileDelete,
            ShieldView, ShieldApprove, ShieldReject, ShieldConfigure,
            MetricsView, AuditLogView, UserManage, PolicyManage,
            BackupCreate, BackupRestore, BackupDelete,
        ].into_iter().collect()
    }
}

impl Role {
    pub fn permissions(&self) -> HashSet<Permission> {
        use Permission::*;
        match self {
            Role::Admin => Permission::all(),
            Role::Operator => [
                CommandExecute, FileRead, FileWrite,
                ShieldView, MetricsView, AuditLogView,
                AgentList, BackupCreate, BackupRestore,
            ].into_iter().collect(),
            Role::Viewer => [
                AgentList, ShieldView, MetricsView, AuditLogView,
            ].into_iter().collect(),
            Role::Agent => [
                AgentRegister, CommandExecute, FileRead, FileWrite, BackupCreate,
            ].into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacUser {
    pub id: String,
    pub username: String,
    pub roles: Vec<Role>,
    #[serde(default)]
    pub allowed_paths: Option<Vec<String>>,
    #[serde(default)]
    pub denied_commands: Option<Vec<String>>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl RbacUser {
    pub fn permissions(&self) -> HashSet<Permission> {
        self.roles.iter().flat_map(|r| r.permissions()).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacToken {
    pub user_id: String,
    pub roles: Vec<Role>,
    pub issued_at: u64,
    pub expires_at: u64,
    pub issuer: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_has_all_permissions() {
        let perms = Role::Admin.permissions();
        assert_eq!(perms.len(), Permission::all().len());
        for p in Permission::all() {
            assert!(perms.contains(&p));
        }
    }

    #[test]
    fn test_viewer_limited() {
        let perms = Role::Viewer.permissions();
        assert!(perms.contains(&Permission::AgentList));
        assert!(perms.contains(&Permission::MetricsView));
        assert!(!perms.contains(&Permission::CommandExecute));
        assert!(!perms.contains(&Permission::ShieldApprove));
    }

    #[test]
    fn test_agent_permissions() {
        let perms = Role::Agent.permissions();
        assert!(perms.contains(&Permission::AgentRegister));
        assert!(perms.contains(&Permission::CommandExecute));
        assert!(perms.contains(&Permission::FileRead));
        assert!(!perms.contains(&Permission::ShieldApprove));
        assert!(!perms.contains(&Permission::UserManage));
    }

    #[test]
    fn test_operator_permissions() {
        let perms = Role::Operator.permissions();
        assert!(perms.contains(&Permission::CommandExecute));
        assert!(perms.contains(&Permission::FileRead));
        assert!(perms.contains(&Permission::FileWrite));
        assert!(perms.contains(&Permission::BackupCreate));
        assert!(!perms.contains(&Permission::UserManage));
        assert!(!perms.contains(&Permission::FileDelete));
    }

    #[test]
    fn test_multi_role_union() {
        let user = RbacUser {
            id: "u1".into(),
            username: "test".into(),
            roles: vec![Role::Viewer, Role::Agent],
            allowed_paths: None,
            denied_commands: None,
            metadata: HashMap::new(),
        };
        let perms = user.permissions();
        assert!(perms.contains(&Permission::AgentList)); // Viewer
        assert!(perms.contains(&Permission::CommandExecute)); // Agent
        assert!(perms.contains(&Permission::MetricsView)); // Viewer
    }

    #[test]
    fn test_user_serialization() {
        let user = RbacUser {
            id: "u1".into(),
            username: "admin".into(),
            roles: vec![Role::Admin],
            allowed_paths: None,
            denied_commands: None,
            metadata: HashMap::new(),
        };
        let json = serde_json::to_string(&user).unwrap();
        let back: RbacUser = serde_json::from_str(&json).unwrap();
        assert_eq!(back.username, "admin");
        assert_eq!(back.roles, vec![Role::Admin]);
    }

    #[test]
    fn test_load_from_yaml() {
        let yaml = r#"
users:
  - username: admin
    roles: [Admin]
    metadata:
      email: admin@example.com
  - username: op1
    roles: [Operator]
    allowed_paths: ["/home/app"]
  - username: ag1
    roles: [Agent]
    denied_commands: ["sudo *"]
"#;
        #[derive(Deserialize)]
        struct RbacConfig {
            users: Vec<RbacUserPartial>,
        }
        #[derive(Deserialize)]
        struct RbacUserPartial {
            username: String,
            roles: Vec<Role>,
            allowed_paths: Option<Vec<String>>,
            denied_commands: Option<Vec<String>>,
            metadata: Option<HashMap<String, String>>,
        }
        let cfg: RbacConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.users.len(), 3);
        assert_eq!(cfg.users[0].username, "admin");
        assert_eq!(cfg.users[0].roles, vec![Role::Admin]);
        let paths = cfg.users[1].allowed_paths.as_ref().unwrap();
        assert_eq!(paths, &["/home/app".to_string()]);
    }


    #[test]
    fn test_role_serialization_roundtrip() {
        for role in [Role::Admin, Role::Operator, Role::Viewer, Role::Agent] {
            let json = serde_json::to_string(&role).unwrap();
            let back: Role = serde_json::from_str(&json).unwrap();
            assert_eq!(role, back);
        }
    }

    #[test]
    fn test_permission_serialization_roundtrip() {
        for perm in [Permission::CommandExecute, Permission::FileRead, Permission::ShieldApprove, Permission::UserManage] {
            let json = serde_json::to_string(&perm).unwrap();
            let back: Permission = serde_json::from_str(&json).unwrap();
            assert_eq!(perm, back);
        }
    }

    #[test]
    fn test_rbac_token_serialization() {
        let token = RbacToken {
            user_id: "u1".into(),
            roles: vec![Role::Admin],
            issued_at: 1000,
            expires_at: 2000,
            issuer: "flowlink".into(),
        };
        let json = serde_json::to_string(&token).unwrap();
        let back: RbacToken = serde_json::from_str(&json).unwrap();
        assert_eq!(back.expires_at, 2000);
        assert_eq!(back.issuer, "flowlink");
    }

    #[test]
    fn test_token_expiry_validation() {
        let now = 2000u64;
        let expired = RbacToken { user_id: "u".into(), roles: vec![], issued_at: 1000, expires_at: 1500, issuer: "x".into() };
        let valid = RbacToken { user_id: "u".into(), roles: vec![], issued_at: 1000, expires_at: 3000, issuer: "x".into() };
        assert!(expired.expires_at < now, "expired token");
        assert!(valid.expires_at > now, "valid token");
    }

    #[test]
    fn test_invalid_yaml_fails() {
        let yaml = "not valid yaml: [";
        #[derive(Deserialize)]
        struct Cfg { users: Vec<RbacUser> }
        let result: Result<Cfg, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_user_with_allowed_paths() {
        let user = RbacUser {
            id: "u1".into(), username: "op".into(),
            roles: vec![Role::Operator],
            allowed_paths: Some(vec!["/home/app".into(), "/var/log".into()]),
            denied_commands: None,
            metadata: HashMap::new(),
        };
        let json = serde_json::to_string(&user).unwrap();
        let back: RbacUser = serde_json::from_str(&json).unwrap();
        assert_eq!(back.allowed_paths.unwrap().len(), 2);
    }

    #[test]
    fn test_user_with_denied_commands() {
        let user = RbacUser {
            id: "u1".into(), username: "ag".into(),
            roles: vec![Role::Agent],
            allowed_paths: None,
            denied_commands: Some(vec!["sudo *".into(), "rm -rf *".into()]),
            metadata: HashMap::new(),
        };
        let json = serde_json::to_string(&user).unwrap();
        let back: RbacUser = serde_json::from_str(&json).unwrap();
        let denied = back.denied_commands.unwrap();
        assert!(denied.contains(&"sudo *".to_string()));
    }

    #[test]
    fn test_complex_user_all_fields() {
        let mut meta = HashMap::new();
        meta.insert("email".into(), "a@b.com".into());
        meta.insert("team".into(), "infra".into());
        let user = RbacUser {
            id: "u1".into(), username: "admin".into(),
            roles: vec![Role::Admin, Role::Operator],
            allowed_paths: Some(vec!["/".into()]),
            denied_commands: Some(vec![]),
            metadata: meta,
        };
        let json = serde_json::to_string(&user).unwrap();
        let back: RbacUser = serde_json::from_str(&json).unwrap();
        assert_eq!(back.metadata["team"], "infra");
        assert_eq!(back.roles.len(), 2);
        assert!(back.permissions().contains(&Permission::UserManage));
    }

    #[test]
    fn test_permission_all_count() {
        let all = Permission::all();
        assert_eq!(all.len(), 20);
    }

    #[test]
    fn test_viewer_has_minimal_permissions() {
        let perms = Role::Viewer.permissions();
        assert_eq!(perms.len(), 4);
    }
}