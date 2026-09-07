use axum::{
    extract::{Path, Query},
    routing, Json, Router,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::llm::memory::save_memory;
use crate::models::memory::Memory;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/memories",
            routing::get(list_memories).post(create_memory),
        )
        .route(
            "/api/v1/memories/:id",
            routing::get(get_memory)
                .patch(update_memory)
                .delete(delete_memory),
        )
        .route(
            "/api/v1/memories-sync",
            routing::get(export_memories_for_sync).post(import_memories_from_sync),
        )
        .route("/api/v1/memories/search", routing::get(search_memories))
}

// ── Types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub memory_type: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MemoryResponse {
    pub id: Uuid,
    pub memory_type: String,
    pub content: String,
    pub importance: f32,
    pub metadata: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Portable record used by the Pro encrypted-sync client. Tombstones are kept
/// so a deletion made offline is not resurrected by another device.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SyncMemoryRecord {
    pub id: Uuid,
    pub memory_type: String,
    pub content: String,
    pub importance: f32,
    pub metadata: serde_json::Value,
    pub source_memory_ids: Vec<Uuid>,
    pub tier: String,
    pub memory_layer: String,
    pub owner_agent_id: String,
    pub memory_scope: String,
    pub scene_tags: Vec<String>,
    pub episode_id: Option<Uuid>,
    pub occurred_at: chrono::DateTime<chrono::Utc>,
    pub valid_from: chrono::DateTime<chrono::Utc>,
    pub valid_to: Option<chrono::DateTime<chrono::Utc>>,
    pub source_type: String,
    pub source_trust: f32,
    pub fact_key: Option<String>,
    pub fact_value: Option<String>,
    pub conflict_status: String,
    pub superseded_by: Option<Uuid>,
    pub retention_status: String,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportSyncMemoriesRequest {
    pub memories: Vec<SyncMemoryRecord>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ImportSyncMemoriesResponse {
    pub accepted: usize,
    pub changed: u64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMemoryRequest {
    pub memory_type: String,
    pub content: String,
    #[serde(default = "default_importance")]
    pub importance: f32,
}

fn default_importance() -> f32 {
    0.5
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMemoryRequest {
    pub content: Option<String>,
    pub memory_type: Option<String>,
    pub importance: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub q: String,
    pub limit: Option<i64>,
    /// When `true`, use semantic vector search via Qdrant instead of ILIKE
    /// keyword matching. Falls back to ILIKE when Qdrant is unavailable.
    #[serde(default)]
    pub semantic: bool,
}

// ── Handler: list ──────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/memories",
    responses((status = 200, body = Vec<MemoryResponse>))
)]
async fn list_memories(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Query(params): Query<ListParams>,
) -> Result<Json<Vec<MemoryResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);

    let rows: Vec<MemoryRow> = if let Some(mt) = &params.memory_type {
        sqlx::query_as(
            "SELECT id, memory_type, content, importance, metadata, \
             created_at, updated_at \
             FROM memories WHERE user_id = $1 AND memory_type = $2 \
             AND deleted_at IS NULL \
             ORDER BY importance DESC, created_at DESC LIMIT $3 OFFSET $4",
        )
        .bind(user_id)
        .bind(mt)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as(
            "SELECT id, memory_type, content, importance, metadata, \
             created_at, updated_at \
             FROM memories WHERE user_id = $1 AND deleted_at IS NULL \
             ORDER BY importance DESC, created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?
    };

    Ok(Json(
        rows.into_iter().map(MemoryRow::into_response).collect(),
    ))
}

// ── Handler: get by id ─────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/memories/{id}",
    responses((status = 200, body = MemoryResponse))
)]
async fn get_memory(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<MemoryResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;

    let row: MemoryRow = sqlx::query_as(
        "SELECT id, memory_type, content, importance, metadata, created_at, updated_at \
         FROM memories WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Memory not found".into()))?;

    Ok(Json(row.into_response()))
}

// ── Handler: create ────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/api/v1/memories",
    request_body = CreateMemoryRequest,
    responses(
        (status = 201, description = "Memory created", body = MemoryResponse),
        (status = 200, description = "Duplicate memory updated", body = MemoryResponse)
    )
)]
async fn create_memory(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Json(req): Json<CreateMemoryRequest>,
) -> Result<(axum::http::StatusCode, Json<MemoryResponse>), AppError> {
    let user_id = auth::require_user(auth).await?;

    let outcome = save_memory(
        &state.db,
        &state.vector,
        &state.llm,
        &state.config.llm.embed_model,
        user_id,
        &req.memory_type,
        &req.content,
        req.importance,
    )
    .await?;

    let status = if outcome.created {
        axum::http::StatusCode::CREATED
    } else {
        axum::http::StatusCode::OK
    };

    Ok((status, Json(memory_to_response(outcome.memory))))
}

// ── Handler: update ────────────────────────────────────────────────

#[utoipa::path(
    patch,
    path = "/api/v1/memories/{id}",
    request_body = UpdateMemoryRequest,
    responses((status = 200, body = MemoryResponse))
)]
async fn update_memory(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateMemoryRequest>,
) -> Result<Json<MemoryResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;

    // Fetch current values for fields that weren't provided
    let current: MemoryRow = sqlx::query_as(
        "SELECT id, memory_type, content, importance, metadata, created_at, updated_at \
         FROM memories WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Memory not found".into()))?;

    let content = req.content.unwrap_or(current.content);
    let memory_type = req.memory_type.unwrap_or(current.memory_type);
    let importance = req.importance.unwrap_or(current.importance);

    let row: MemoryRow = sqlx::query_as(
        "UPDATE memories SET content = $1, memory_type = $2, importance = $3, \
         updated_at = NOW() \
         WHERE id = $4 AND user_id = $5 AND deleted_at IS NULL \
         RETURNING id, memory_type, content, importance, metadata, created_at, updated_at",
    )
    .bind(&content)
    .bind(&memory_type)
    .bind(importance)
    .bind(id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row.into_response()))
}

// ── Handler: delete (soft) ─────────────────────────────────────────

#[utoipa::path(
    delete,
    path = "/api/v1/memories/{id}",
    responses((status = 204))
)]
async fn delete_memory(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, AppError> {
    let user_id = auth::require_user(auth).await?;

    let rows = sqlx::query(
        "UPDATE memories SET deleted_at = NOW() WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .execute(&state.db)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::NotFound("Memory not found".into()));
    }

    // Best-effort: remove the corresponding vector point from Qdrant.
    // Failure is non-fatal — the PG row is already soft-deleted.
    if let Some(qdrant) = &state.vector {
        if let Err(e) = qdrant.delete_points("memories", &[id]).await {
            tracing::warn!(
                %user_id, memory_id = %id, %e,
                "Failed to delete Qdrant point for manually deleted memory (non-fatal)"
            );
        }
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/v1/memories-sync",
    responses((status = 200, body = Vec<SyncMemoryRecord>))
)]
async fn export_memories_for_sync(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<Vec<SyncMemoryRecord>>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let rows = sqlx::query_as::<_, SyncMemoryRow>(
        "SELECT id, memory_type, content, importance, metadata, source_memory_ids, tier, \
         memory_layer, owner_agent_id, memory_scope, scene_tags, episode_id, occurred_at, valid_from, \
         valid_to, source_type, source_trust, fact_key, fact_value, conflict_status, superseded_by, \
         retention_status, deleted_at, created_at, updated_at \
         FROM memories WHERE user_id = $1 ORDER BY updated_at, id LIMIT 10000",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter().map(SyncMemoryRow::into_record).collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/memories-sync",
    request_body = ImportSyncMemoriesRequest,
    responses((status = 200, body = ImportSyncMemoriesResponse))
)]
async fn import_memories_from_sync(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Json(req): Json<ImportSyncMemoriesRequest>,
) -> Result<Json<ImportSyncMemoriesResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    if req.memories.len() > 10_000 {
        return Err(AppError::Validation("同步记录不能超过 10000 条".into()));
    }
    let mut changed = 0;
    let mut changed_memories = Vec::new();
    let mut transaction = state.db.begin().await?;
    for memory in &req.memories {
        if memory.memory_type.trim().is_empty()
            || memory.memory_type.len() > 30
            || memory.content.len() > 100_000
            || !memory.importance.is_finite()
            || !(0.0..=1.0).contains(&memory.importance)
            || !memory.source_trust.is_finite()
            || !(0.0..=1.0).contains(&memory.source_trust)
            || !memory.metadata.is_object()
            || memory.owner_agent_id.trim().is_empty()
            || memory.owner_agent_id.len() > 64
            || memory.source_type.trim().is_empty()
            || memory.source_type.len() > 32
            || memory
                .fact_key
                .as_ref()
                .is_some_and(|value| value.len() > 255)
            || memory.scene_tags.len() > 100
            || memory.scene_tags.iter().any(|value| value.len() > 100)
            || !matches!(memory.tier.as_str(), "raw" | "derived")
            || !matches!(
                memory.memory_layer.as_str(),
                "episode" | "fact" | "experience"
            )
            || !matches!(
                memory.memory_scope.as_str(),
                "private" | "shared" | "global"
            )
            || !matches!(
                memory.conflict_status.as_str(),
                "active" | "superseded" | "disputed"
            )
            || !matches!(
                memory.retention_status.as_str(),
                "active" | "stale" | "filtered"
            )
        {
            return Err(AppError::Validation("同步记录包含无效字段".into()));
        }
        let changed_row = sqlx::query(
            r#"INSERT INTO memories
               (id, user_id, memory_type, content, importance, metadata, source_memory_ids, tier,
                memory_layer, owner_agent_id, memory_scope, scene_tags, episode_id, occurred_at,
                valid_from, valid_to, source_type, source_trust, fact_key, fact_value,
                conflict_status, superseded_by, retention_status, deleted_at, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
                       $15, $16, $17, $18, $19, $20, $21, NULL, $22, $23, $24, $25)
               ON CONFLICT (id) DO UPDATE SET
                 memory_type = EXCLUDED.memory_type, content = EXCLUDED.content,
                 importance = EXCLUDED.importance, metadata = EXCLUDED.metadata,
                 source_memory_ids = EXCLUDED.source_memory_ids, tier = EXCLUDED.tier,
                 memory_layer = EXCLUDED.memory_layer, owner_agent_id = EXCLUDED.owner_agent_id,
                 memory_scope = EXCLUDED.memory_scope, scene_tags = EXCLUDED.scene_tags,
                 episode_id = EXCLUDED.episode_id, occurred_at = EXCLUDED.occurred_at,
                 valid_from = EXCLUDED.valid_from, valid_to = EXCLUDED.valid_to,
                 source_type = EXCLUDED.source_type, source_trust = EXCLUDED.source_trust,
                 fact_key = EXCLUDED.fact_key, fact_value = EXCLUDED.fact_value,
                 conflict_status = EXCLUDED.conflict_status, superseded_by = NULL,
                 retention_status = EXCLUDED.retention_status,
                 deleted_at = EXCLUDED.deleted_at, updated_at = EXCLUDED.updated_at
               WHERE memories.user_id = EXCLUDED.user_id
                 AND memories.updated_at < EXCLUDED.updated_at"#,
        )
        .bind(memory.id)
        .bind(user_id)
        .bind(memory.memory_type.trim())
        .bind(&memory.content)
        .bind(memory.importance)
        .bind(&memory.metadata)
        .bind(&memory.source_memory_ids)
        .bind(&memory.tier)
        .bind(&memory.memory_layer)
        .bind(&memory.owner_agent_id)
        .bind(&memory.memory_scope)
        .bind(&memory.scene_tags)
        .bind(memory.episode_id)
        .bind(memory.occurred_at)
        .bind(memory.valid_from)
        .bind(memory.valid_to)
        .bind(&memory.source_type)
        .bind(memory.source_trust)
        .bind(&memory.fact_key)
        .bind(&memory.fact_value)
        .bind(&memory.conflict_status)
        .bind(&memory.retention_status)
        .bind(memory.deleted_at)
        .bind(memory.created_at)
        .bind(memory.updated_at)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        changed += changed_row;
        if changed_row > 0 {
            changed_memories.push((
                memory.id,
                memory.content.clone(),
                memory.deleted_at.is_some(),
            ));
        }
    }
    // References are restored only after every record exists, avoiding FK
    // failures when the superseding memory appears later in the bundle.
    for memory in &req.memories {
        if let Some(superseded_by) = memory.superseded_by {
            sqlx::query(
                "UPDATE memories SET superseded_by = $1 WHERE id = $2 AND user_id = $3 \
                 AND EXISTS (SELECT 1 FROM memories target WHERE target.id = $1 AND target.user_id = $3)",
            )
            .bind(superseded_by)
            .bind(memory.id)
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
        }
    }
    transaction.commit().await?;
    // Rebuild vector state after the SQL transaction. Sync remains successful
    // when an embedding provider is offline; keyword recall still works and a
    // later sync/maintenance pass can retry the best-effort index operation.
    if let Some(qdrant) = state.vector.clone() {
        let db = state.db.clone();
        let llm = state.llm.clone();
        let embed_model = state.config.llm.embed_model.clone();
        tokio::spawn(async move {
            let deleted: Vec<Uuid> = changed_memories
                .iter()
                .filter_map(|(id, _, is_deleted)| is_deleted.then_some(*id))
                .collect();
            if !deleted.is_empty() {
                if let Err(error) = qdrant.delete_points("memories", &deleted).await {
                    tracing::warn!(%error, "Failed to remove synced memory vectors");
                }
            }
            for (id, content, is_deleted) in changed_memories {
                if is_deleted {
                    continue;
                }
                if let Err(error) = crate::llm::memory::upsert_memory_vector(
                    &db,
                    &qdrant,
                    &llm,
                    &embed_model,
                    user_id,
                    id,
                    &content,
                )
                .await
                {
                    tracing::warn!(memory_id = %id, %error, "Failed to index synced memory");
                }
            }
        });
    }
    Ok(Json(ImportSyncMemoriesResponse {
        accepted: req.memories.len(),
        changed,
    }))
}

// ── Handler: search ────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/memories/search",
    params(
        ("q" = String, Query),
        ("limit" = Option<i64>, Query),
        ("semantic" = Option<bool>, Query)
    ),
    responses((status = 200, body = Vec<MemoryResponse>))
)]
async fn search_memories(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Vec<MemoryResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let limit = params.limit.unwrap_or(20).min(100);

    // ── Semantic path ──────────────────────────────────────────
    let rows = if params.semantic {
        try_semantic_search_memories(&state, user_id, &params.q, limit).await
    } else {
        None
    };

    // ── Fallback / default: ILIKE ──────────────────────────────
    let rows = match rows {
        Some(rows) if !rows.is_empty() => rows,
        _ => {
            let pattern = format!("%{}%", params.q);
            sqlx::query_as(
                "SELECT id, memory_type, content, importance, metadata, created_at, updated_at \
                 FROM memories WHERE user_id = $1 AND content ILIKE $2 AND deleted_at IS NULL \
                 ORDER BY importance DESC LIMIT $3",
            )
            .bind(user_id)
            .bind(&pattern)
            .bind(limit)
            .fetch_all(&state.db)
            .await?
        }
    };

    Ok(Json(
        rows.into_iter().map(MemoryRow::into_response).collect(),
    ))
}

/// Try semantic search via the shared [`crate::llm::semantic::semantic_memory_search`].
/// Returns `None` on any failure or when Qdrant is unavailable, so the caller
/// falls back to ILIKE.
async fn try_semantic_search_memories(
    state: &AppState,
    user_id: Uuid,
    query: &str,
    limit: i64,
) -> Option<Vec<MemoryRow>> {
    let qdrant = state.vector.as_ref()?;
    let ids = crate::llm::semantic::semantic_memory_search(
        qdrant,
        &state.llm,
        &state.config.llm.embed_model,
        user_id,
        query,
        limit as u32,
    )
    .await?;

    // Load from PG and preserve Qdrant rank order
    let rows: Vec<MemoryRow> = sqlx::query_as(
        "SELECT id, memory_type, content, importance, metadata, created_at, updated_at \
         FROM memories WHERE id = ANY($1) AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(&ids)
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!(%user_id, %e, "Failed to load semantic search results from PG");
    })
    .ok()?;

    // Reorder to match Qdrant rank
    let mut row_map: std::collections::HashMap<Uuid, MemoryRow> =
        rows.into_iter().map(|r: MemoryRow| (r.id, r)).collect();
    let ordered: Vec<MemoryRow> = ids.iter().filter_map(|id| row_map.remove(id)).collect();

    Some(ordered)
}

// ── FromRow helper ─────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct MemoryRow {
    id: Uuid,
    memory_type: String,
    content: String,
    importance: f32,
    metadata: serde_json::Value,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct SyncMemoryRow {
    id: Uuid,
    memory_type: String,
    content: String,
    importance: f32,
    metadata: serde_json::Value,
    source_memory_ids: Vec<Uuid>,
    tier: String,
    memory_layer: String,
    owner_agent_id: String,
    memory_scope: String,
    scene_tags: Vec<String>,
    episode_id: Option<Uuid>,
    occurred_at: chrono::DateTime<chrono::Utc>,
    valid_from: chrono::DateTime<chrono::Utc>,
    valid_to: Option<chrono::DateTime<chrono::Utc>>,
    source_type: String,
    source_trust: f32,
    fact_key: Option<String>,
    fact_value: Option<String>,
    conflict_status: String,
    superseded_by: Option<Uuid>,
    retention_status: String,
    deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl SyncMemoryRow {
    fn into_record(self) -> SyncMemoryRecord {
        SyncMemoryRecord {
            id: self.id,
            memory_type: self.memory_type,
            content: self.content,
            importance: self.importance,
            metadata: self.metadata,
            source_memory_ids: self.source_memory_ids,
            tier: self.tier,
            memory_layer: self.memory_layer,
            owner_agent_id: self.owner_agent_id,
            memory_scope: self.memory_scope,
            scene_tags: self.scene_tags,
            episode_id: self.episode_id,
            occurred_at: self.occurred_at,
            valid_from: self.valid_from,
            valid_to: self.valid_to,
            source_type: self.source_type,
            source_trust: self.source_trust,
            fact_key: self.fact_key,
            fact_value: self.fact_value,
            conflict_status: self.conflict_status,
            superseded_by: self.superseded_by,
            retention_status: self.retention_status,
            deleted_at: self.deleted_at,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl MemoryRow {
    fn into_response(self) -> MemoryResponse {
        MemoryResponse {
            id: self.id,
            memory_type: self.memory_type,
            content: self.content,
            importance: self.importance,
            metadata: self.metadata,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// Convert a model-layer [`Memory`] to the route-layer [`MemoryResponse`].
fn memory_to_response(m: Memory) -> MemoryResponse {
    MemoryResponse {
        id: m.id,
        memory_type: m.memory_type,
        content: m.content,
        importance: m.importance,
        metadata: m.metadata,
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn default_importance_is_0_5() {
        assert!((default_importance() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn create_memory_request_defaults() {
        let req = CreateMemoryRequest {
            memory_type: "fact".into(),
            content: "Paris is the capital of France".into(),
            importance: 0.0, // explicit, should override default
        };
        assert_eq!(req.memory_type, "fact");
        assert_eq!(req.content, "Paris is the capital of France");
        assert!((req.importance - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn create_memory_request_uses_default_importance() {
        let json = serde_json::json!({
            "memory_type": "preference",
            "content": "I prefer dark mode"
        });
        let req: CreateMemoryRequest = serde_json::from_value(json).unwrap();
        assert!((req.importance - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn memory_to_response_maps_fields() {
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let now = Utc::now();
        let m = Memory {
            id,
            user_id: Uuid::new_v4(),
            project_id: None,
            memory_type: "fact".into(),
            content: "test content".into(),
            importance: 0.8,
            access_count: 3,
            last_accessed_at: Some(now),
            vector_id: None,
            source_memory_ids: vec![],
            tier: "raw".into(),
            memory_layer: "fact".into(),
            owner_agent_id: "lingshu".into(),
            memory_scope: "private".into(),
            scene_tags: vec![],
            episode_id: None,
            occurred_at: now,
            valid_from: now,
            valid_to: None,
            source_type: "manual".into(),
            source_trust: 0.8,
            fact_key: None,
            fact_value: None,
            conflict_status: "active".into(),
            superseded_by: None,
            retention_status: "active".into(),
            metadata: serde_json::json!({"source": "chat"}),
            deleted_at: None,
            created_at: now,
            updated_at: now,
        };
        let resp = memory_to_response(m);
        assert_eq!(resp.id, id);
        assert_eq!(resp.memory_type, "fact");
        assert_eq!(resp.content, "test content");
        assert!((resp.importance - 0.8).abs() < f32::EPSILON);
    }
}
