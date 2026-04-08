// RBAC Manager — token issuance, validation, permission enforcement

use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use dashmap::DashMap;
use rand::Rng;

use flowlink_core::rbac::{Permission, RbacToken, RbacUser};

#[derive(Debug, Clone, thiserror::Error)]
pub enum RbacError {
    #[error("user not found: {0}")]
    UserNotFound(String),
    #[error("token invalid or expired")]
    TokenInvalid,
    #[error("permission denied: {0:?}")]
    PermissionDenied(Permission),
    #[error("path access denied: {0}")]
    PathDenied(String),
    #[error("command denied: {0}")]
    CommandDenied(String),
}

pub type Result<T> = std::result::Result<T, RbacError>;

pub struct RbacManager {
    users: DashMap<String, RbacUser>,
    tokens: DashMap<String, RbacToken>, // token_hash → RbacToken
}

impl RbacManager {
    pub fn new() -> Self {
        Self {
            users: DashMap::new(),
            tokens: DashMap::new(),
        }
    }

    pub fn add_user(&self, user: RbacUser) -> Result<()> {
        self.users.insert(user.id.clone(), user);
        Ok(())
    }

    pub fn remove_user(&self, user_id: &str) -> Result<()> {
        if self.users.remove(user_id).is_none() {
            return Err(RbacError::UserNotFound(user_id.to_string()));
        }
        // Remove associated tokens
        self.tokens.retain(|_, tok| tok.user_id != user_id);
        Ok(())
    }

    pub fn issue_token(&self, user_id: &str, ttl: Duration) -> Result<String> {
        let user = self.users.get(user_id)
            .ok_or_else(|| RbacError::UserNotFound(user_id.to_string()))?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let raw_token: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        let token_hash = hex::encode(raw_token.as_bytes());

        let token = RbacToken {
            user_id: user_id.to_string(),
            roles: user.roles.clone(),
            issued_at: now,
            expires_at: now + ttl.as_secs(),
            issuer: "rbac".to_string(),
        };

        self.tokens.insert(token_hash, token);
        Ok(raw_token)
    }

    pub fn validate_token(&self, token: &str) -> Option<RbacToken> {
        let hash = hex::encode(token.as_bytes());
        let tok = self.tokens.get(&hash)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if tok.expires_at < now {
            drop(tok);
            self.tokens.remove(&hash);
            return None;
        }
        Some(tok.clone())
    }

    pub fn check_permission(&self, token: &str, permission: &Permission) -> Result<()> {
        let tok = self.validate_token(token)
            .ok_or(RbacError::TokenInvalid)?;

        let perms: HashSet<Permission> = tok.roles.iter().flat_map(|r| r.permissions()).collect();
        if perms.contains(permission) {
            Ok(())
        } else {
            Err(RbacError::PermissionDenied(permission.clone()))
        }
    }

    pub fn check_path_access(&self, token: &str, path: &str) -> Result<()> {
        let tok = self.validate_token(token)
            .ok_or(RbacError::TokenInvalid)?;

        let user = self.users.get(&tok.user_id)
            .ok_or(RbacError::TokenInvalid)?;

        if let Some(ref allowed) = user.allowed_paths {
            if !allowed.iter().any(|p| path.starts_with(p)) {
                return Err(RbacError::PathDenied(path.to_string()));
            }
        }
        Ok(())
    }

    pub fn check_command(&self, token: &str, command: &str) -> Result<()> {
        let tok = self.validate_token(token)
            .ok_or(RbacError::TokenInvalid)?;

        let user = self.users.get(&tok.user_id)
            .ok_or(RbacError::TokenInvalid)?;

        if let Some(ref denied) = user.denied_commands {
            for pattern in denied {
                if pattern_matches(pattern, command) {
                    return Err(RbacError::CommandDenied(command.to_string()));
                }
            }
        }
        Ok(())
    }

    pub fn list_users(&self) -> Vec<RbacUser> {
        self.users.iter().map(|u| u.value().clone()).collect()
    }

    /// Load users from a config structure (caller deserializes YAML/JSON).
    pub fn load_users(&self, users: Vec<RbacUser>) {
        for user in users {
            self.users.insert(user.id.clone(), user);
        }
    }
}

impl Default for RbacManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple glob matching: `sudo *` matches `sudo rm -rf /`, `rm -rf *` matches `rm -rf /foo`.
fn pattern_matches(pattern: &str, command: &str) -> bool {
    let pattern_parts: Vec<&str> = pattern.split_whitespace().collect();
    let cmd_parts: Vec<&str> = command.split_whitespace().collect();

    if pattern_parts.is_empty() {
        return false;
    }

    let stars = pattern_parts.iter().filter(|p| **p == "*").count();
    if stars == pattern_parts.len() {
        return true; // just "*"
    }

    // First word must match literally (unless it's *)
    if pattern_parts[0] != "*" && cmd_parts.first() != Some(&pattern_parts[0]) {
        return false;
    }

    // If pattern is "sudo *" and command starts with "sudo"
    if pattern_parts.len() == 2 && pattern_parts[1] == "*" {
        return cmd_parts.first() == Some(&pattern_parts[0]);
    }

    // "rm -rf *" pattern
    if pattern_parts.len() == 3 && pattern_parts[2] == "*" {
        return cmd_parts.len() >= 2
            && cmd_parts[0] == pattern_parts[0]
            && cmd_parts[1] == pattern_parts[1];
    }

    // Fallback: prefix match
    let prefix: String = pattern_parts.iter()
        .take_while(|p| **p != "*")
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    command.starts_with(&prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowlink_core::rbac::RbacUser;
    use std::collections::HashMap;

    fn make_user(id: &str, username: &str, roles: Vec<Role>) -> RbacUser {
        RbacUser {
            id: id.to_string(),
            username: username.to_string(),
            roles,
            allowed_paths: None,
            denied_commands: None,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_add_and_list_users() {
        let mgr = RbacManager::new();
        mgr.add_user(make_user("u1", "admin", vec![Role::Admin])).unwrap();
        mgr.add_user(make_user("u2", "viewer", vec![Role::Viewer])).unwrap();
        assert_eq!(mgr.list_users().len(), 2);
    }

    #[test]
    fn test_remove_user() {
        let mgr = RbacManager::new();
        mgr.add_user(make_user("u1", "admin", vec![Role::Admin])).unwrap();
        mgr.remove_user("u1").unwrap();
        assert_eq!(mgr.list_users().len(), 0);
    }

    #[test]
    fn test_remove_nonexistent_user() {
        let mgr = RbacManager::new();
        assert!(mgr.remove_user("nope").is_err());
    }

    #[test]
    fn test_token_issuance_and_validation() {
        let mgr = RbacManager::new();
        mgr.add_user(make_user("u1", "admin", vec![Role::Admin])).unwrap();
        let token = mgr.issue_token("u1", Duration::from_secs(3600)).unwrap();
        let tok = mgr.validate_token(&token).unwrap();
        assert_eq!(tok.user_id, "u1");
        assert_eq!(tok.roles, vec![Role::Admin]);
    }

    #[test]
    fn test_token_invalid_for_nonexistent_user() {
        let mgr = RbacManager::new();
        assert!(mgr.issue_token("nope", Duration::from_secs(3600)).is_err());
    }

    #[test]
    fn test_token_expiry() {
        let mgr = RbacManager::new();
        mgr.add_user(make_user("u1", "admin", vec![Role::Admin])).unwrap();
        let token = mgr.issue_token("u1", Duration::from_secs(0)).unwrap();
        // Give a tiny bit of time
        std::thread::sleep(std::time::Duration::from_secs(1));
        assert!(mgr.validate_token(&token).is_none());
    }

    #[test]
    fn test_permission_check_admin() {
        let mgr = RbacManager::new();
        mgr.add_user(make_user("u1", "admin", vec![Role::Admin])).unwrap();
        let token = mgr.issue_token("u1", Duration::from_secs(3600)).unwrap();
        assert!(mgr.check_permission(&token, &Permission::UserManage).is_ok());
        assert!(mgr.check_permission(&token, &Permission::ShieldApprove).is_ok());
    }

    #[test]
    fn test_permission_check_viewer_denied() {
        let mgr = RbacManager::new();
        mgr.add_user(make_user("u1", "viewer", vec![Role::Viewer])).unwrap();
        let token = mgr.issue_token("u1", Duration::from_secs(3600)).unwrap();
        assert!(mgr.check_permission(&token, &Permission::MetricsView).is_ok());
        assert!(matches!(mgr.check_permission(&token, &Permission::CommandExecute), Err(RbacError::PermissionDenied(_))));
    }

    #[test]
    fn test_path_restriction() {
        let mut user = make_user("u1", "op", vec![Role::Operator]);
        user.allowed_paths = Some(vec!["/home/app".to_string()]);
        let mgr = RbacManager::new();
        mgr.add_user(user).unwrap();
        let token = mgr.issue_token("u1", Duration::from_secs(3600)).unwrap();
        assert!(mgr.check_path_access(&token, "/home/app/file.txt").is_ok());
        assert!(matches!(mgr.check_path_access(&token, "/etc/passwd"), Err(RbacError::PathDenied(_))));
    }

    #[test]
    fn test_command_deny_list() {
        let mut user = make_user("u1", "ag", vec![Role::Agent]);
        user.denied_commands = Some(vec!["sudo *".to_string(), "rm -rf *".to_string()]);
        let mgr = RbacManager::new();
        mgr.add_user(user).unwrap();
        let token = mgr.issue_token("u1", Duration::from_secs(3600)).unwrap();
        assert!(mgr.check_command(&token, "ls -la").is_ok());
        assert!(matches!(mgr.check_command(&token, "sudo rm -rf /"), Err(RbacError::CommandDenied(_))));
        assert!(matches!(mgr.check_command(&token, "rm -rf /tmp/foo"), Err(RbacError::CommandDenied(_))));
    }

    #[test]
    fn test_multi_role_union_permissions() {
        let mgr = RbacManager::new();
        mgr.add_user(make_user("u1", "multi", vec![Role::Viewer, Role::Agent])).unwrap();
        let token = mgr.issue_token("u1", Duration::from_secs(3600)).unwrap();
        // Viewer can view metrics
        assert!(mgr.check_permission(&token, &Permission::MetricsView).is_ok());
        // Agent can execute commands
        assert!(mgr.check_permission(&token, &Permission::CommandExecute).is_ok());
        // Neither can manage users
        assert!(matches!(mgr.check_permission(&token, &Permission::UserManage), Err(RbacError::PermissionDenied(_))));
    }

    #[test]
    fn test_invalid_token() {
        let mgr = RbacManager::new();
        assert!(mgr.validate_token("bogus").is_none());
        assert!(matches!(mgr.check_permission("bogus", &Permission::AgentList), Err(RbacError::TokenInvalid)));
    }

    #[test]
    fn test_pattern_matches() {
        assert!(pattern_matches("sudo *", "sudo rm -rf /"));
        assert!(pattern_matches("sudo *", "sudo apt install foo"));
        assert!(!pattern_matches("sudo *", "ls -la"));
        assert!(pattern_matches("rm -rf *", "rm -rf /tmp/foo"));
        assert!(!pattern_matches("rm -rf *", "rm -r /tmp/foo"));
        assert!(pattern_matches("*", "anything goes here"));
    }

    #[test]
    fn test_remove_user_revokes_tokens() {
        let mgr = RbacManager::new();
        mgr.add_user(make_user("u1", "admin", vec![Role::Admin])).unwrap();
        let token = mgr.issue_token("u1", Duration::from_secs(3600)).unwrap();
        assert!(mgr.validate_token(&token).is_some());
        mgr.remove_user("u1").unwrap();
        assert!(mgr.validate_token(&token).is_none());
    }

    #[test]
    fn test_load_users_batch() {
        let mgr = RbacManager::new();
        mgr.load_users(vec![
            make_user("u1", "a", vec![Role::Admin]),
            make_user("u2", "v", vec![Role::Viewer]),
        ]);
        assert_eq!(mgr.list_users().len(), 2);
    }
}
