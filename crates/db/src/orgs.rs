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
            "SELECT * FROM org_members WHERE org_id = $1 ORDER BY joined_at"
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
