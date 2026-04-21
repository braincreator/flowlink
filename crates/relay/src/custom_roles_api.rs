use sqlx::Row;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::middleware::AccountIdExtractor;
use crate::server::AppState;

fn get_pool(state: &AppState) -> Result<&sqlx::PgPool, (StatusCode, String)> {
    state.db.as_ref()
        .map(|db| db.pool())
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Database not configured".to_string()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRole {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub description: String,
    pub permissions: Vec<String>,
    pub base_role: String,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateRoleRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub permissions: Vec<String>,
    #[serde(default = "default_base_role")]
    pub base_role: String,
}

fn default_base_role() -> String { "viewer".to_string() }

#[derive(Debug, Deserialize)]
pub struct UpdateRoleRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub permissions: Option<Vec<String>>,
    pub base_role: Option<String>,
}

fn valid_permissions() -> Vec<&'static str> {
    vec![
        "agent:register", "agent:list", "agent:remove",
        "command:execute", "command:execute_destructive", "command:approve",
        "file:read", "file:write", "file:delete",
        "shield:view", "shield:approve", "shield:reject", "shield:configure",
        "metrics:view", "audit:view",
        "user:manage", "policy:manage",
        "backup:create", "backup:restore", "backup:delete",
        "api_key:create", "api_key:list", "api_key:revoke", "api_key:delete",
        "webhook:manage", "role:manage",
    ]
}

fn validate_permissions(perms: &[String]) -> Result<(), String> {
    let valid = valid_permissions();
    for p in perms {
        if !valid.contains(&p.as_str()) {
            return Err(format!("Unknown permission: {}", p));
        }
    }
    Ok(())
}

fn row_to_role(row: &sqlx::postgres::PgRow) -> CustomRole {
    CustomRole {
        id: row.get("id"),
        org_id: row.get("org_id"),
        name: row.get("name"),
        description: row.get::<Option<String>, _>("description").unwrap_or_default(),
        permissions: row.get::<Vec<String>, _>("permissions"),
        base_role: row.get("base_role"),
        created_by: row.get("created_by"),
        created_at: row.get::<String, _>("created_at"),
        updated_at: row.get::<String, _>("updated_at"),
    }
}

async fn require_org_admin(pool: &sqlx::PgPool, org_id: &Uuid, account_id: &str) -> Result<(), (StatusCode, String)> {
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM org_members WHERE org_id = $1 AND account_id = $2"
    )
    .bind(org_id).bind(account_id)
    .fetch_optional(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .flatten();

    match role.as_deref() {
        Some("owner") | Some("admin") => Ok(()),
        Some(r) => Err((StatusCode::FORBIDDEN, format!("Role '{}' cannot manage custom roles", r))),
        None => Err((StatusCode::FORBIDDEN, "Not a member of this organization".into())),
    }
}

pub async fn list_roles(
    State(state): State<AppState>,
    AccountIdExtractor(account_id): AccountIdExtractor,
    Path(org_id): Path<Uuid>,
) -> Result<(StatusCode, Json<Vec<CustomRole>>), (StatusCode, String)> {
    let pool = get_pool(&state)?;
    require_org_admin(pool, &org_id, &account_id).await?;

    let rows = sqlx::query(
        "SELECT id, org_id, name, description, permissions, base_role, created_by, created_at::text, updated_at::text FROM custom_roles WHERE org_id = $1 ORDER BY created_at"
    )
    .bind(org_id)
    .fetch_all(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::OK, Json(rows.iter().map(|r| row_to_role(r)).collect())))
}

pub async fn create_role(
    State(state): State<AppState>,
    AccountIdExtractor(account_id): AccountIdExtractor,
    Path(org_id): Path<Uuid>,
    Json(body): Json<CreateRoleRequest>,
) -> Result<(StatusCode, Json<CustomRole>), (StatusCode, String)> {
    let pool = get_pool(&state)?;
    require_org_admin(pool, &org_id, &account_id).await?;

    if body.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Role name is required".into()));
    }
    if !["admin", "operator", "viewer", "agent"].contains(&body.base_role.as_str()) {
        return Err((StatusCode::BAD_REQUEST, format!("Invalid base_role: {}", body.base_role)));
    }
    validate_permissions(&body.permissions).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM custom_roles WHERE org_id = $1 AND name = $2)"
    ).bind(org_id).bind(&body.name).fetch_one(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if exists {
        return Err((StatusCode::CONFLICT, format!("Role '{}' already exists", body.name)));
    }

    let row = sqlx::query(
        "INSERT INTO custom_roles (org_id, name, description, permissions, base_role, created_by) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id, org_id, name, description, permissions, base_role, created_by, created_at::text, updated_at::text"
    )
    .bind(org_id).bind(body.name.trim()).bind(&body.description).bind(&body.permissions).bind(&body.base_role).bind(&account_id)
    .fetch_one(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(row_to_role(&row))))
}

pub async fn update_role(
    State(state): State<AppState>,
    AccountIdExtractor(account_id): AccountIdExtractor,
    Path((org_id, role_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateRoleRequest>,
) -> Result<(StatusCode, Json<CustomRole>), (StatusCode, String)> {
    let pool = get_pool(&state)?;
    require_org_admin(pool, &org_id, &account_id).await?;

    if let Some(ref perms) = body.permissions { validate_permissions(perms).map_err(|e| (StatusCode::BAD_REQUEST, e))?; }
    if let Some(ref br) = body.base_role {
        if !["admin", "operator", "viewer", "agent"].contains(&br.as_str()) {
            return Err((StatusCode::BAD_REQUEST, format!("Invalid base_role: {}", br)));
        }
    }

    let row = sqlx::query(
        "UPDATE custom_roles SET name = COALESCE($3, name), description = COALESCE($4, description), permissions = COALESCE($5, permissions), base_role = COALESCE($6, base_role), updated_at = NOW() WHERE id = $1 AND org_id = $2 RETURNING id, org_id, name, description, permissions, base_role, created_by, created_at::text, updated_at::text"
    )
    .bind(role_id).bind(org_id).bind(&body.name).bind(&body.description).bind(body.permissions.as_deref()).bind(body.base_role.as_deref())
    .fetch_optional(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match row {
        Some(r) => Ok((StatusCode::OK, Json(row_to_role(&r)))),
        None => Err((StatusCode::NOT_FOUND, "Role not found".into())),
    }
}

pub async fn delete_role(
    State(state): State<AppState>,
    AccountIdExtractor(account_id): AccountIdExtractor,
    Path((org_id, role_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let pool = get_pool(&state)?;
    require_org_admin(pool, &org_id, &account_id).await?;

    sqlx::query("UPDATE org_members SET custom_role_id = NULL WHERE custom_role_id = $1 AND org_id = $2")
        .bind(role_id).bind(org_id).execute(pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query("UPDATE api_keys SET custom_role_id = NULL WHERE custom_role_id = $1")
        .bind(role_id).execute(pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let result = sqlx::query("DELETE FROM custom_roles WHERE id = $1 AND org_id = $2")
        .bind(role_id).bind(org_id).execute(pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 { return Err((StatusCode::NOT_FOUND, "Role not found".into())); }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct AssignRoleRequest { pub role_id: Uuid }

pub async fn assign_role(
    State(state): State<AppState>,
    AccountIdExtractor(account_id): AccountIdExtractor,
    Path((org_id, target_account_id)): Path<(Uuid, String)>,
    Json(body): Json<AssignRoleRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let pool = get_pool(&state)?;
    require_org_admin(pool, &org_id, &account_id).await?;

    let role_org: Option<Uuid> = sqlx::query_scalar(
        "SELECT org_id FROM custom_roles WHERE id = $1"
    ).bind(body.role_id).fetch_optional(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?.flatten();

    match role_org {
        Some(ro) if ro == org_id => {},
        Some(_) => return Err((StatusCode::FORBIDDEN, "Role does not belong to this org".into())),
        None => return Err((StatusCode::NOT_FOUND, "Custom role not found".into())),
    }

    sqlx::query("UPDATE org_members SET custom_role_id = $1 WHERE org_id = $2 AND account_id = $3")
        .bind(body.role_id).bind(org_id).bind(&target_account_id)
        .execute(pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::OK)
}
