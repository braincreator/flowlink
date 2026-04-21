use sqlx::Row;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::middleware::AccountIdExtractor;
use crate::server::AppState;

fn gp(state: &AppState) -> Result<&sqlx::PgPool, (StatusCode, String)> {
    state.db.as_ref().map(|db| db.pool()).ok_or((StatusCode::SERVICE_UNAVAILABLE, "Database not configured".to_string()))
}

#[derive(Debug, Deserialize)]
pub struct TagRequest { pub tags: Vec<String> }

#[derive(Debug, Serialize)]
pub struct TagsResponse { pub agent_id: String, pub tags: Vec<String> }

#[derive(Debug, Deserialize)]
pub struct TagFilterQuery {
    pub tag: Option<String>,
    pub tags: Option<String>,
}

pub async fn set_tags(
    State(state): State<AppState>,
    AccountIdExtractor(_account_id): AccountIdExtractor,
    Path(agent_id): Path<String>,
    Json(body): Json<TagRequest>,
) -> Result<(StatusCode, Json<TagsResponse>), (StatusCode, String)> {
    if agent_id.len() > 128 || agent_id.contains('\0') {
        return Err((StatusCode::BAD_REQUEST, "Invalid agent_id".into()));
    }
    if body.tags.len() > 50 {
        return Err((StatusCode::BAD_REQUEST, "Too many tags (max 50)".into()));
    }
    sqlx::query("DELETE FROM agent_tags WHERE agent_id = $1")
        .bind(&agent_id).execute(gp(&state)?).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut clean = Vec::new();
    for tag in &body.tags {
        let t = tag.trim().to_lowercase();
        if t.is_empty() { continue; }
        if t.len() > 64 {
            return Err((StatusCode::BAD_REQUEST, format!("Tag too long (max 64 chars): {}", &t[..32.min(t.len())])));
        }
        if !t.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == ':' || c == '/') {
            return Err((StatusCode::BAD_REQUEST, format!("Invalid tag characters: {}", &t)));
        }
        sqlx::query("INSERT INTO agent_tags (agent_id, tag) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(&agent_id).bind(&t).execute(gp(&state)?).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        clean.push(t);
    }

    Ok((StatusCode::OK, Json(TagsResponse { agent_id, tags: clean })))
}

pub async fn get_tags(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<(StatusCode, Json<TagsResponse>), (StatusCode, String)> {
    let rows = sqlx::query("SELECT tag FROM agent_tags WHERE agent_id = $1 ORDER BY tag")
        .bind(&agent_id).fetch_all(gp(&state)?).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let tags: Vec<String> = rows.iter().map(|r| r.get("tag")).collect();
    Ok((StatusCode::OK, Json(TagsResponse { agent_id, tags })))
}

pub async fn delete_tags(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    sqlx::query("DELETE FROM agent_tags WHERE agent_id = $1")
        .bind(&agent_id).execute(gp(&state)?).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize)]
pub struct AgentWithTag { pub agent_id: String, pub tags: Vec<String> }

pub async fn list_by_tag(
    State(state): State<AppState>,
    Query(query): Query<TagFilterQuery>,
) -> Result<(StatusCode, Json<Vec<AgentWithTag>>), (StatusCode, String)> {
    let tags: Vec<String> = query.tag.into_iter()
        .chain(query.tags.into_iter().flat_map(|s| s.split(',').map(|t| t.trim().to_lowercase()).collect::<Vec<_>>()))
        .filter(|t| !t.is_empty()).collect();

    if tags.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Provide 'tag' or 'tags' query parameter".into()));
    }

    let rows = sqlx::query(
        "SELECT agent_id FROM agent_tags WHERE tag = ANY($1) GROUP BY agent_id HAVING COUNT(DISTINCT tag) = $2"
    ).bind(&tags).bind(tags.len() as i64).fetch_all(gp(&state)?).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut result = Vec::new();
    for row in rows {
        let aid: String = row.get("agent_id");
        let trows = sqlx::query("SELECT tag FROM agent_tags WHERE agent_id = $1 ORDER BY tag")
            .bind(&aid).fetch_all(gp(&state)?).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let atags: Vec<String> = trows.iter().map(|r| r.get("tag")).collect();
        result.push(AgentWithTag { agent_id: aid, tags: atags });
    }

    Ok((StatusCode::OK, Json(result)))
}

pub async fn list_all_tags(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<Vec<String>>), (StatusCode, String)> {
    let rows = sqlx::query("SELECT DISTINCT tag FROM agent_tags ORDER BY tag")
        .fetch_all(gp(&state)?).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let tags: Vec<String> = rows.iter().map(|r| r.get("tag")).collect();
    Ok((StatusCode::OK, Json(tags)))
}
