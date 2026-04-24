//! Organization persistence — CRUD for orgs, members, invitations

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrgRow {
    pub org_id: Uuid,
    pub name: String,
    pub slug: String,
    pub owner_account_id: String,
    pub plan_id: String,
    pub limits: serde_json::Value,
    pub is_trial: bool,
    pub trial_ends_at: Option<DateTime<Utc>>,
    pub grace_ends_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrgMemberRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub account_id: String,
    pub role: String,
    pub invited_by: Option<String>,
    pub joined_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrgInvitationRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub email: Option<String>,
    pub role: String,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub accepted_by: Option<String>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub struct OrgRepo;

impl OrgRepo {
    // === Organizations ===

    pub async fn create(pool: &PgPool, name: &str, slug: &str, owner_account_id: &str, plan_id: &str) -> Result<OrgRow> {
        let row = sqlx::query_as::<_, OrgRow>(
            "INSERT INTO organizations (name, slug, owner_account_id, plan_id) VALUES ($1, $2, $3, $4) RETURNING *"
        )
        .bind(name).bind(slug).bind(owner_account_id).bind(plan_id)
        .fetch_one(pool).await?;
        Ok(row)
    }

    pub async fn get(pool: &PgPool, org_id: Uuid) -> Result<Option<OrgRow>> {
        let row = sqlx::query_as::<_, OrgRow>("SELECT * FROM organizations WHERE org_id = $1")
            .bind(org_id).fetch_optional(pool).await?;
        Ok(row)
    }

    pub async fn get_by_slug(pool: &PgPool, slug: &str) -> Result<Option<OrgRow>> {
        let row = sqlx::query_as::<_, OrgRow>("SELECT * FROM organizations WHERE slug = $1")
            .bind(slug).fetch_optional(pool).await?;
        Ok(row)
    }

    pub async fn list_by_account(pool: &PgPool, account_id: &str) -> Result<Vec<OrgRow>> {
        let rows = sqlx::query_as::<_, OrgRow>(
            "SELECT o.* FROM organizations o
             JOIN org_members m ON m.org_id = o.org_id
             WHERE m.account_id = $1
             ORDER BY o.created_at DESC"
        )
        .bind(account_id).fetch_all(pool).await?;
        Ok(rows)
    }

    pub async fn update(pool: &PgPool, org_id: Uuid, name: Option<&str>, slug: Option<&str>) -> Result<Option<OrgRow>> {
        let row = sqlx::query_as::<_, OrgRow>(
            "UPDATE organizations SET name = COALESCE($2, name), slug = COALESCE($3, slug), updated_at = NOW()
             WHERE org_id = $1 RETURNING *"
        )
        .bind(org_id).bind(name).bind(slug)
        .fetch_optional(pool).await?;
        Ok(row)
    }

    pub async fn delete(pool: &PgPool, org_id: Uuid) -> Result<bool> {
        let result = sqlx::query("DELETE FROM organizations WHERE org_id = $1")
            .bind(org_id).execute(pool).await?;
        Ok(result.rows_affected() > 0)
    }

    // === Members ===

    pub async fn add_member(pool: &PgPool, org_id: Uuid, account_id: &str, role: &str, invited_by: Option<&str>) -> Result<OrgMemberRow> {
        let row = sqlx::query_as::<_, OrgMemberRow>(
            "INSERT INTO org_members (org_id, account_id, role, invited_by) VALUES ($1, $2, $3, $4)
             ON CONFLICT (org_id, account_id) DO UPDATE SET role = EXCLUDED.role
             RETURNING *"
        )
        .bind(org_id).bind(account_id).bind(role).bind(invited_by)
        .fetch_one(pool).await?;
        Ok(row)
    }

    pub async fn list_members(pool: &PgPool, org_id: Uuid) -> Result<Vec<OrgMemberRow>> {
        let rows = sqlx::query_as::<_, OrgMemberRow>(
            "SELECT om.id, om.org_id, om.account_id, om.role, om.invited_by, om.joined_at, om.created_at, a.email
             FROM org_members om
             LEFT JOIN accounts a ON a.account_id = om.account_id
             WHERE om.org_id = $1 ORDER BY om.joined_at"
        )
        .bind(org_id).fetch_all(pool).await?;
        Ok(rows)
    }

    pub async fn get_member(pool: &PgPool, org_id: Uuid, account_id: &str) -> Result<Option<OrgMemberRow>> {
        let row = sqlx::query_as::<_, OrgMemberRow>(
            "SELECT * FROM org_members WHERE org_id = $1 AND account_id = $2"
        )
        .bind(org_id).bind(account_id).fetch_optional(pool).await?;
        Ok(row)
    }

    pub async fn remove_member(pool: &PgPool, org_id: Uuid, account_id: &str) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM org_members WHERE org_id = $1 AND account_id = $2 AND role != 'owner'"
        )
        .bind(org_id).bind(account_id).execute(pool).await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn change_role(pool: &PgPool, org_id: Uuid, account_id: &str, new_role: &str) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE org_members SET role = $3 WHERE org_id = $1 AND account_id = $2 AND role != 'owner'"
        )
        .bind(org_id).bind(account_id).bind(new_role).execute(pool).await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn transfer_ownership(pool: &PgPool, org_id: Uuid, from_account_id: &str, to_account_id: &str) -> Result<bool> {
        let mut tx = pool.begin().await?;
        sqlx::query("UPDATE org_members SET role = 'member' WHERE org_id = $1 AND account_id = $2 AND role = 'owner'")
            .bind(org_id).bind(from_account_id).execute(&mut *tx).await?;
        sqlx::query("UPDATE org_members SET role = 'owner' WHERE org_id = $1 AND account_id = $2")
            .bind(org_id).bind(to_account_id).execute(&mut *tx).await?;
        sqlx::query("UPDATE organizations SET owner_account_id = $2 WHERE org_id = $1")
            .bind(org_id).bind(to_account_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }

    // === Invitations ===

    pub async fn create_invitation(pool: &PgPool, org_id: Uuid, email: Option<&str>, role: &str, token: &str, expires_at: DateTime<Utc>) -> Result<OrgInvitationRow> {
        let row = sqlx::query_as::<_, OrgInvitationRow>(
            "INSERT INTO org_invitations (org_id, email, role, token, expires_at) VALUES ($1, $2, $3, $4, $5) RETURNING *"
        )
        .bind(org_id).bind(email).bind(role).bind(token).bind(expires_at)
        .fetch_one(pool).await?;
        Ok(row)
    }

    pub async fn list_invitations(pool: &PgPool, org_id: Uuid) -> Result<Vec<OrgInvitationRow>> {
        let rows = sqlx::query_as::<_, OrgInvitationRow>(
            "SELECT * FROM org_invitations WHERE org_id = $1 AND accepted_at IS NULL AND expires_at > NOW() ORDER BY created_at DESC"
        )
        .bind(org_id).fetch_all(pool).await?;
        Ok(rows)
    }

    pub async fn accept_invitation(pool: &PgPool, token: &str, account_id: &str) -> Result<Option<(Uuid, String)>> {
        let row = sqlx::query_as::<_, OrgInvitationRow>(
            "SELECT * FROM org_invitations WHERE token = $1 AND accepted_at IS NULL AND expires_at > NOW()"
        )
        .bind(token).fetch_optional(pool).await?;

        match row {
            Some(inv) => {
                sqlx::query("UPDATE org_invitations SET accepted_by = $2, accepted_at = NOW() WHERE id = $1")
                    .bind(inv.id).bind(account_id).execute(pool).await?;
                // Add member
                OrgRepo::add_member(pool, inv.org_id, account_id, &inv.role, None).await?;
                Ok(Some((inv.org_id, inv.role)))
            }
            None => Ok(None),
        }
    }

    pub async fn delete_invitation(pool: &PgPool, invitation_id: Uuid) -> Result<bool> {
        let result = sqlx::query("DELETE FROM org_invitations WHERE id = $1")
            .bind(invitation_id).execute(pool).await?;
        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn now() -> DateTime<Utc> { Utc::now() }
    fn test_uuid() -> Uuid { uuid::Uuid::new_v4() }

    fn make_org_row() -> OrgRow {
        OrgRow {
            org_id: test_uuid(),
            name: "Test Org".to_string(),
            slug: "test-org".to_string(),
            owner_account_id: "owner_001".to_string(),
            plan_id: "plan_pro".to_string(),
            limits: serde_json::json!({"max_members": 10, "max_agents": 5}),
            is_trial: true,
            trial_ends_at: Some(Utc::now() + chrono::Duration::days(14)),
            grace_ends_at: None,
            created_at: now(),
            updated_at: now(),
        }
    }

    fn make_member_row() -> OrgMemberRow {
        OrgMemberRow {
            id: test_uuid(),
            org_id: test_uuid(),
            account_id: "member_001".to_string(),
            role: "admin".to_string(),
            invited_by: Some("owner_001".to_string()),
            joined_at: now(),
            created_at: now(),
            email: Some("user@example.com".to_string()),
        }
    }

    fn make_invitation_row() -> OrgInvitationRow {
        OrgInvitationRow {
            id: test_uuid(),
            org_id: test_uuid(),
            email: Some("invite@example.com".to_string()),
            role: "member".to_string(),
            token: "token_abc123".to_string(),
            expires_at: Utc::now() + chrono::Duration::days(7),
            accepted_by: None,
            accepted_at: None,
            created_at: now(),
        }
    }

    // ── OrgRow ─────────────────────────────────────────────────────

    #[test]
    fn org_row_construction() {
        let o = make_org_row();
        assert_eq!(o.name, "Test Org");
        assert_eq!(o.slug, "test-org");
        assert!(o.is_trial);
    }

    #[test]
    fn org_row_none_trial_dates() {
        let mut o = make_org_row();
        o.trial_ends_at = None;
        o.grace_ends_at = None;
        assert!(o.trial_ends_at.is_none());
        assert!(o.grace_ends_at.is_none());
    }

    #[test]
    fn org_row_some_trial_dates() {
        let o = make_org_row();
        assert!(o.trial_ends_at.is_some());
    }

    #[test]
    fn org_row_clone() {
        let o = make_org_row();
        let cloned = o.clone();
        assert_eq!(cloned.org_id, o.org_id);
        assert_eq!(cloned.name, o.name);
    }

    #[test]
    fn org_row_debug() {
        let o = make_org_row();
        let debug = format!("{:?}", o);
        assert!(debug.contains("Test Org"));
    }

    #[test]

    #[test]
    fn org_row_empty_name_slug() {
        let mut o = make_org_row();
        o.name = String::new();
        o.slug = String::new();
        assert!(o.name.is_empty());
        assert!(o.slug.is_empty());
    }

    #[test]
    fn org_row_limits_field() {
        let o = make_org_row();
        assert_eq!(o.limits["max_members"], 10);
        assert_eq!(o.limits["max_agents"], 5);
    }

    #[test]
    fn org_row_not_trial() {
        let mut o = make_org_row();
        o.is_trial = false;
        assert!(!o.is_trial);
    }

    // ── OrgMemberRow ───────────────────────────────────────────────

    #[test]
    fn member_row_construction() {
        let m = make_member_row();
        assert_eq!(m.account_id, "member_001");
        assert_eq!(m.role, "admin");
    }

    #[test]
    fn member_row_none_invited_by() {
        let mut m = make_member_row();
        m.invited_by = None;
        assert!(m.invited_by.is_none());
    }

    #[test]
    fn member_row_none_email() {
        let mut m = make_member_row();
        m.email = None;
        assert!(m.email.is_none());
    }

    #[test]
    fn member_row_clone() {
        let m = make_member_row();
        let cloned = m.clone();
        assert_eq!(cloned.id, m.id);
        assert_eq!(cloned.account_id, m.account_id);
    }

    #[test]

    #[test]
    fn member_row_different_roles() {
        for role in &["owner", "admin", "member", "viewer"] {
            let mut m = make_member_row();
            m.role = role.to_string();
            assert_eq!(m.role, *role);
        }
    }

    // ── OrgInvitationRow ───────────────────────────────────────────

    #[test]
    fn invitation_row_construction() {
        let i = make_invitation_row();
        assert_eq!(i.token, "token_abc123");
        assert_eq!(i.role, "member");
        assert!(i.accepted_by.is_none());
        assert!(i.accepted_at.is_none());
    }

    #[test]
    fn invitation_row_none_accepted() {
        let i = make_invitation_row();
        assert!(i.accepted_by.is_none());
        assert!(i.accepted_at.is_none());
    }

    #[test]
    fn invitation_row_some_accepted() {
        let mut i = make_invitation_row();
        i.accepted_by = Some("acct_001".to_string());
        i.accepted_at = Some(now());
        assert_eq!(i.accepted_by.as_deref(), Some("acct_001"));
        assert!(i.accepted_at.is_some());
    }

    #[test]
    fn invitation_row_clone() {
        let i = make_invitation_row();
        let cloned = i.clone();
        assert_eq!(cloned.id, i.id);
        assert_eq!(cloned.token, i.token);
    }

    #[test]

    #[test]
    fn invitation_row_none_email() {
        let mut i = make_invitation_row();
        i.email = None;
        assert!(i.email.is_none());
    }

    // ── OrgRepo ────────────────────────────────────────────────────

    #[test]
    fn org_repo_exists() {
        let _repo = OrgRepo;
    }
}
