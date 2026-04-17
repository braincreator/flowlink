//! Organization API endpoints

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::middleware::AccountIdExtractor;
use crate::server::AppState;
use flowlink_db::orgs::{OrgInvitationRow, OrgMemberRow, OrgRepo, OrgRow};

// ═══════════════════════════════════════════════
// Request types
// ═══════════════════════════════════════════════

#[derive(Deserialize)]
pub struct CreateOrgRequest {
    pub name: String,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default = "default_plan")]
    pub plan_id: String,
}

#[derive(Deserialize)]
pub struct UpdateOrgRequest {
    pub name: Option<String>,
    pub slug: Option<String>,
}

#[derive(Deserialize)]
pub struct SwitchOrgRequest {
    pub org_id: String,
}

#[derive(Deserialize)]
pub struct InviteMemberRequest {
    pub email: Option<String>,
    pub role: String,
}

#[derive(Deserialize)]
pub struct AcceptInviteRequest {
    pub token: String,
}

#[derive(Deserialize)]
pub struct ChangeRoleRequest {
    pub role: String,
}

#[derive(Deserialize)]
pub struct OnboardRequest {
    pub org_name: String,
    pub slug: Option<String>,
}

fn default_plan() -> String { "trial".to_string() }

// ═══════════════════════════════════════════════
// Helper
// ═══════════════════════════════════════════════

fn get_db(state: &AppState) -> Result<&sqlx::PgPool, (StatusCode, Json<Value>)> {
    state.db.as_ref()
        .map(|db| db.pool())
        .ok_or_else(|| (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "database not configured"}))))
}

async fn require_org_role(
    pool: &sqlx::PgPool,
    org_id: Uuid,
    account_id: &str,
    required_roles: &[&str],
) -> Result<OrgMemberRow, (StatusCode, Json<Value>)> {
    let member = OrgRepo::get_member(pool, org_id, account_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    match member {
        Some(m) if required_roles.contains(&m.role.as_str()) => Ok(m),
        Some(_) => Err((StatusCode::FORBIDDEN, Json(json!({"error": "insufficient permissions"})))),
        None => Err((StatusCode::NOT_FOUND, Json(json!({"error": "not a member of this organization"})))),
    }
}

fn json_row(row: &OrgRow) -> Value {
    json!({
        "org_id": row.org_id,
        "name": row.name,
        "slug": row.slug,
        "owner_account_id": row.owner_account_id,
        "plan_id": row.plan_id,
        "limits": row.limits,
        "is_trial": row.is_trial,
        "trial_ends_at": row.trial_ends_at,
        "grace_ends_at": row.grace_ends_at,
        "created_at": row.created_at,
        "updated_at": row.updated_at,
    })
}

fn json_member(row: &OrgMemberRow) -> Value {
    json!({
        "id": row.id,
        "org_id": row.org_id,
        "account_id": row.account_id,
        "email": row.email,
        "role": row.role,
        "invited_by": row.invited_by,
        "joined_at": row.joined_at,
    })
}

fn json_invitation(row: &OrgInvitationRow) -> Value {
    json!({
        "id": row.id,
        "org_id": row.org_id,
        "email": row.email,
        "role": row.role,
        "expires_at": row.expires_at,
        "created_at": row.created_at,
    })
}

fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else if c.is_whitespace() { '-' } else { '\0' })
        .filter(|c| *c != '\0')
        .collect::<String>()
        .split(|c: char| c == '-' && { true })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// ═══════════════════════════════════════════════
// Handlers
// ═══════════════════════════════════════════════

/// GET /api/orgs — list orgs where user is member
pub async fn list_my_orgs(State(state): State<AppState>, AccountIdExtractor(account_id): AccountIdExtractor) -> impl IntoResponse {
    let pool = match get_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };

    match OrgRepo::list_by_account(pool, &account_id).await {
        Ok(orgs) => Json(json!({ "orgs": orgs.iter().map(json_row).collect::<Vec<_>>() })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// POST /api/orgs — create org, auto-add user as owner
pub async fn create_org(State(state): State<AppState>, AccountIdExtractor(account_id): AccountIdExtractor, Json(body): Json<CreateOrgRequest>) -> impl IntoResponse {
    let pool = match get_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };

    // Generate slug if not provided
    let slug = if let Some(s) = &body.slug {
        s.clone()
    } else {
        let base = slugify(&body.name);
        // Check for collision
        let final_slug = match OrgRepo::get_by_slug(pool, &base).await {
            Ok(None) => base,
            _ => {
                let suffix: String = uuid::Uuid::new_v4().to_string()[..4].to_string();
                format!("{}-{}", base, suffix)
            }
        };
        final_slug
    };

    // Check slug uniqueness
    if let Ok(Some(_)) = OrgRepo::get_by_slug(pool, &slug).await {
        return (StatusCode::CONFLICT, Json(json!({"error": "slug already exists"}))).into_response();
    }

    match OrgRepo::create(pool, &body.name, &slug, &account_id, &body.plan_id).await {
        Ok(org) => {
            // Auto-add creator as owner member
            let _ = OrgRepo::add_member(pool, org.org_id, &account_id, "owner", None).await;
            (StatusCode::CREATED, Json(json_row(&org))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// GET /api/orgs/{org_id} — get org details
pub async fn get_org(State(state): State<AppState>, AccountIdExtractor(account_id): AccountIdExtractor, Path(org_id): Path<Uuid>) -> impl IntoResponse {
    let pool = match get_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };

    // Must be member
    if let Err(e) = require_org_role(pool, org_id, &account_id, &["owner", "admin", "member", "viewer"]).await {
        return e.into_response();
    }

    match OrgRepo::get(pool, org_id).await {
        Ok(Some(org)) => {
            let members = OrgRepo::list_members(pool, org_id).await.unwrap_or_default();
            let mut data = json_row(&org);
            data["member_count"] = json!(members.len());
            Json(data).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "organization not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// PUT /api/orgs/{org_id} — update name/slug (owner/admin only)
pub async fn update_org(State(state): State<AppState>, AccountIdExtractor(account_id): AccountIdExtractor, Path(org_id): Path<Uuid>, Json(body): Json<UpdateOrgRequest>) -> impl IntoResponse {
    let pool = match get_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };

    if let Err(e) = require_org_role(pool, org_id, &account_id, &["owner", "admin"]).await {
        return e.into_response();
    }

    // Check slug uniqueness if changing
    if let Some(ref slug) = body.slug {
        if let Ok(Some(existing)) = OrgRepo::get_by_slug(pool, slug).await {
            if existing.org_id != org_id {
                return (StatusCode::CONFLICT, Json(json!({"error": "slug already exists"}))).into_response();
            }
        }
    }

    match OrgRepo::update(pool, org_id, body.name.as_deref(), body.slug.as_deref()).await {
        Ok(Some(org)) => Json(json_row(&org)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "organization not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// DELETE /api/orgs/{org_id} — delete org (owner only)
pub async fn delete_org(State(state): State<AppState>, AccountIdExtractor(account_id): AccountIdExtractor, Path(org_id): Path<Uuid>) -> impl IntoResponse {
    let pool = match get_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };

    if let Err(e) = require_org_role(pool, org_id, &account_id, &["owner"]).await {
        return e.into_response();
    }

    match OrgRepo::delete(pool, org_id).await {
        Ok(true) => Json(json!({"ok": true, "message": "organization deleted"})).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(json!({"error": "organization not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// POST /api/orgs/switch — switch active org (returns new JWT with org_id)
pub async fn switch_org(State(state): State<AppState>, AccountIdExtractor(account_id): AccountIdExtractor, Json(body): Json<SwitchOrgRequest>) -> impl IntoResponse {
    let pool = match get_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };

    let engine = match &state.auth_engine {
        Some(e) => e,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "auth not configured"}))).into_response(),
    };

    let org_id = match Uuid::parse_str(&body.org_id) {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid org_id"}))).into_response(),
    };

    // Must be member
    let member = match require_org_role(pool, org_id, &account_id, &["owner", "admin", "member", "viewer"]).await {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    // Get user info from claims (via extension or DB)
    // We'll create a minimal token — need user_id from account
    let user_id = &member.account_id; // fallback

    match engine.create_org_tokens(user_id, &account_id, None, None, false, &org_id.to_string(), &member.role) {
        Ok(tokens) => Json(json!({
            "access_token": tokens.access_token,
            "refresh_token": tokens.refresh_token,
            "expires_in": tokens.expires_in,
            "token_type": "Bearer",
            "org_id": org_id,
        })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// GET /api/orgs/{org_id}/members — list members
pub async fn list_members(State(state): State<AppState>, AccountIdExtractor(account_id): AccountIdExtractor, Path(org_id): Path<Uuid>) -> impl IntoResponse {
    let pool = match get_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };

    if let Err(e) = require_org_role(pool, org_id, &account_id, &["owner", "admin", "member", "viewer"]).await {
        return e.into_response();
    }

    match OrgRepo::list_members(pool, org_id).await {
        Ok(members) => Json(json!({ "members": members.iter().map(json_member).collect::<Vec<_>>() })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// POST /api/orgs/{org_id}/invites — create invitation (owner/admin)
pub async fn invite_member(State(state): State<AppState>, AccountIdExtractor(account_id): AccountIdExtractor, Path(org_id): Path<Uuid>, Json(body): Json<InviteMemberRequest>) -> impl IntoResponse {
    let pool = match get_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };

    if let Err(e) = require_org_role(pool, org_id, &account_id, &["owner", "admin"]).await {
        return e.into_response();
    }

    // Validate role
    if !["owner", "admin", "member", "viewer"].contains(&body.role.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid role"}))).into_response();
    }

    let token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + chrono::Duration::days(7);

    match OrgRepo::create_invitation(pool, org_id, body.email.as_deref(), &body.role, &token, expires_at).await {
        Ok(inv) => Json(json_invitation(&inv)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// POST /api/orgs/invites/accept — accept by token
pub async fn accept_invite(State(state): State<AppState>, AccountIdExtractor(account_id): AccountIdExtractor, Json(body): Json<AcceptInviteRequest>) -> impl IntoResponse {
    let pool = match get_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };

    match OrgRepo::accept_invitation(pool, &body.token, &account_id).await {
        Ok(Some((org_id, role))) => {
            let org = OrgRepo::get(pool, org_id).await.unwrap_or(None);
            Json(json!({
                "ok": true,
                "org_id": org_id,
                "role": role,
                "org": org.map(|r| json_row(&r)),
            })).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "invitation not found or expired"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// GET /api/orgs/{org_id}/invites — list pending invitations
pub async fn list_invites(State(state): State<AppState>, AccountIdExtractor(account_id): AccountIdExtractor, Path(org_id): Path<Uuid>) -> impl IntoResponse {
    let pool = match get_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };

    if let Err(e) = require_org_role(pool, org_id, &account_id, &["owner", "admin"]).await {
        return e.into_response();
    }

    match OrgRepo::list_invitations(pool, org_id).await {
        Ok(invites) => Json(json!({ "invitations": invites.iter().map(json_invitation).collect::<Vec<_>>() })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// DELETE /api/orgs/{org_id}/members/{account_id} — remove member (owner/admin)
pub async fn remove_member(State(state): State<AppState>, AccountIdExtractor(account_id): AccountIdExtractor, Path((org_id, target_id)): Path<(Uuid, String)>) -> impl IntoResponse {
    let pool = match get_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };

    if let Err(e) = require_org_role(pool, org_id, &account_id, &["owner", "admin"]).await {
        return e.into_response();
    }

    match OrgRepo::remove_member(pool, org_id, &target_id).await {
        Ok(true) => Json(json!({"ok": true, "message": "member removed"})).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(json!({"error": "member not found or is owner"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// POST /api/orgs/onboard — create first org with JWT tokens
pub async fn onboard(State(state): State<AppState>, AccountIdExtractor(account_id): AccountIdExtractor, Json(body): Json<OnboardRequest>) -> impl IntoResponse {
    let pool = match get_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };

    let engine = match &state.auth_engine {
        Some(e) => e,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "auth not configured"}))).into_response(),
    };

    // Generate slug
    let slug = if let Some(s) = &body.slug {
        s.clone()
    } else {
        let base = slugify(&body.org_name);
        match OrgRepo::get_by_slug(pool, &base).await {
            Ok(None) => base,
            _ => {
                let suffix: String = Uuid::new_v4().to_string()[..4].to_string();
                format!("{}-{}", base, suffix)
            }
        }
    };

    if let Ok(Some(_)) = OrgRepo::get_by_slug(pool, &slug).await {
        return (StatusCode::CONFLICT, Json(json!({"error": "slug already exists"}))).into_response();
    }

    let org = match OrgRepo::create(pool, &body.org_name, &slug, &account_id, "trial").await {
        Ok(o) => o,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    };

    // Enable trial: 7 days
    let trial_ends = Utc::now() + chrono::Duration::days(7);
    if let Err(e) = sqlx::query("UPDATE organizations SET is_trial = true, trial_ends_at = $2 WHERE org_id = $1")
        .bind(org.org_id)
        .bind(trial_ends)
        .execute(pool)
        .await
    {
        log::warn!("Failed to set trial for org {}: {e}", org.org_id);
    }

    // Refetch to include trial fields
    let org = OrgRepo::get(pool, org.org_id).await.unwrap_or(Some(org)).unwrap();

    let _ = OrgRepo::add_member(pool, org.org_id, &account_id, "owner", None).await;

    match engine.create_org_tokens(&account_id, &account_id, None, None, false, &org.org_id.to_string(), "owner") {
        Ok(tokens) => Json(json!({
            "org": json_row(&org),
            "access_token": tokens.access_token,
            "refresh_token": tokens.refresh_token,
            "expires_in": tokens.expires_in,
        })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// PATCH /api/orgs/{org_id}/members/{account_id} — change role (owner/admin)
pub async fn change_member_role(State(state): State<AppState>, AccountIdExtractor(account_id): AccountIdExtractor, Path((org_id, target_id)): Path<(Uuid, String)>, Json(body): Json<ChangeRoleRequest>) -> impl IntoResponse {
    let pool = match get_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };

    if let Err(e) = require_org_role(pool, org_id, &account_id, &["owner", "admin"]).await {
        return e.into_response();
    }

    if !["owner", "admin", "member", "viewer"].contains(&body.role.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid role"}))).into_response();
    }

    match OrgRepo::change_role(pool, org_id, &target_id, &body.role).await {
        Ok(true) => Json(json!({"ok": true, "account_id": target_id, "new_role": body.role})).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(json!({"error": "member not found or is owner"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}
