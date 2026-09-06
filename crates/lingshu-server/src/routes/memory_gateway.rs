use std::collections::{HashMap, HashSet};
use std::time::Instant;

use axum::{
    extract::{Path, Query, State},
    routing, Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::llm::evolutionary_memory::{
    can_read_memory, estimate_tokens, is_retrieval_eligible, score_memory, RankableMemory,
    RetrievalWeights, ScoreBreakdown,
};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/memory-gateway/agents",
            routing::get(list_agents).post(create_agent),
        )
        .route(
            "/api/v1/memory-gateway/memories",
            routing::get(list_gateway_memories).post(create_gateway_memory),
        )
        .route(
            "/api/v1/memory-gateway/memories/:id/grants",
            routing::post(grant_memory_access),
        )
        .route(
            "/api/v1/memory-gateway/search",
            routing::post(gateway_search),
        )
        .route(
            "/api/v1/memory-gateway/conflicts",
            routing::get(list_conflicts),
        )
        .route(
            "/api/v1/memory-gateway/conflicts/:id/resolve",
            routing::post(resolve_conflict_review),
        )
        .route(
            "/api/v1/memory-gateway/maintenance",
            routing::post(run_maintenance),
        )
        .route("/api/v1/memory-gateway/stats", routing::get(gateway_stats))
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AgentResponse {
    pub agent_key: String,
    pub display_name: String,
    pub role_description: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateAgentRequest {
    pub agent_key: String,
    pub display_name: String,
    #[serde(default)]
    pub role_description: String,
}

#[derive(Debug, Deserialize)]
pub struct GatewayListParams {
    #[serde(default = "default_agent")]
    pub agent_id: String,
    pub layer: Option<String>,
    pub scope: Option<String>,
    pub context_scope: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateGatewayMemoryRequest {
    #[serde(default = "default_context_scope")]
    pub context_scope: String,
    pub request_id: Option<String>,
    #[serde(default = "default_agent")]
    pub agent_id: String,
    #[serde(default = "default_layer")]
    pub memory_layer: String,
    #[serde(default = "default_scope")]
    pub memory_scope: String,
    #[serde(default = "default_memory_type")]
    pub memory_type: String,
    pub content: String,
    #[serde(default = "default_importance")]
    pub importance: f32,
    #[serde(default)]
    pub scene_tags: Vec<String>,
    pub episode_id: Option<Uuid>,
    pub occurred_at: Option<DateTime<Utc>>,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_to: Option<DateTime<Utc>>,
    #[serde(default = "default_source_type")]
    pub source_type: String,
    #[serde(default = "default_source_trust")]
    pub source_trust: f32,
    pub fact_key: Option<String>,
    pub fact_value: Option<String>,
    #[serde(default)]
    pub source_memory_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, ToSchema, Clone)]
pub struct GatewayMemoryResponse {
    pub context_scope: String,
    pub id: Uuid,
    pub memory_type: String,
    pub content: String,
    pub importance: f32,
    pub access_count: i32,
    pub memory_layer: String,
    pub owner_agent_id: String,
    pub memory_scope: String,
    pub scene_tags: Vec<String>,
    pub episode_id: Option<Uuid>,
    pub occurred_at: DateTime<Utc>,
    pub valid_from: DateTime<Utc>,
    pub valid_to: Option<DateTime<Utc>>,
    pub source_type: String,
    pub source_trust: f32,
    pub fact_key: Option<String>,
    pub fact_value: Option<String>,
    pub conflict_status: String,
    pub superseded_by: Option<Uuid>,
    pub retention_status: String,
    pub source_memory_ids: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct GatewayMemoryRow {
    context_scope: String,
    id: Uuid,
    memory_type: String,
    content: String,
    importance: f32,
    access_count: i32,
    memory_layer: String,
    owner_agent_id: String,
    memory_scope: String,
    scene_tags: Vec<String>,
    episode_id: Option<Uuid>,
    occurred_at: DateTime<Utc>,
    valid_from: DateTime<Utc>,
    valid_to: Option<DateTime<Utc>>,
    source_type: String,
    source_trust: f32,
    fact_key: Option<String>,
    fact_value: Option<String>,
    conflict_status: String,
    superseded_by: Option<Uuid>,
    retention_status: String,
    source_memory_ids: Vec<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    has_grant: bool,
}

impl GatewayMemoryRow {
    fn response(&self) -> GatewayMemoryResponse {
        GatewayMemoryResponse {
            context_scope: self.context_scope.clone(),
            id: self.id,
            memory_type: self.memory_type.clone(),
            content: self.content.clone(),
            importance: self.importance,
            access_count: self.access_count,
            memory_layer: self.memory_layer.clone(),
            owner_agent_id: self.owner_agent_id.clone(),
            memory_scope: self.memory_scope.clone(),
            scene_tags: self.scene_tags.clone(),
            episode_id: self.episode_id,
            occurred_at: self.occurred_at,
            valid_from: self.valid_from,
            valid_to: self.valid_to,
            source_type: self.source_type.clone(),
            source_trust: self.source_trust,
            fact_key: self.fact_key.clone(),
            fact_value: self.fact_value.clone(),
            conflict_status: self.conflict_status.clone(),
            superseded_by: self.superseded_by,
            retention_status: self.retention_status.clone(),
            source_memory_ids: self.source_memory_ids.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GrantMemoryRequest {
    pub owner_agent_id: String,
    pub grantee_agent_id: String,
    #[serde(default = "default_true")]
    pub can_read: bool,
    #[serde(default)]
    pub can_write: bool,
    #[serde(default)]
    pub can_share: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GrantMemoryResponse {
    pub memory_id: Uuid,
    pub grantee_agent_id: String,
    pub can_read: bool,
    pub can_write: bool,
    pub can_share: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GatewaySearchRequest {
    pub context_scope: Option<String>,
    pub query: String,
    #[serde(default = "default_agent")]
    pub agent_id: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_k")]
    pub top_k: u32,
    pub layer: Option<String>,
    #[serde(default)]
    pub scene_tags: Vec<String>,
    pub weights: Option<RetrievalWeights>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RankedMemoryResponse {
    #[serde(flatten)]
    pub memory: GatewayMemoryResponse,
    pub rank: usize,
    pub score: f32,
    pub score_breakdown: ScoreBreakdown,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GatewaySearchResponse {
    pub run_id: Uuid,
    pub mode: String,
    pub requested_k: u32,
    pub candidate_count: usize,
    pub filtered_count: usize,
    pub latency_ms: f32,
    pub estimated_tokens: i32,
    pub weights: RetrievalWeights,
    pub results: Vec<RankedMemoryResponse>,
    pub disputed_memories: Vec<GatewayMemoryResponse>,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct ConflictEventResponse {
    pub id: Uuid,
    pub fact_key: String,
    pub incumbent_memory_id: Uuid,
    pub challenger_memory_id: Uuid,
    pub resolution: String,
    pub reason: String,
    pub score_delta: f32,
    pub incumbent_value: Option<String>,
    pub challenger_value: Option<String>,
    pub review_status: String,
    pub reviewer_agent_id: Option<String>,
    pub review_note: Option<String>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub experience_memory_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResolveConflictRequest {
    /// Which candidate becomes active: `incumbent` or `challenger`.
    pub winner: String,
    pub reviewer_agent_id: String,
    pub note: Option<String>,
    #[serde(default = "default_true")]
    pub promote_to_experience: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ResolveConflictResponse {
    pub conflict: ConflictEventResponse,
    pub winner_memory_id: Uuid,
    pub loser_memory_id: Uuid,
    pub experience_memory_id: Option<Uuid>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct ConflictReviewMemoryRow {
    id: Uuid,
    content: String,
    memory_type: String,
    importance: f32,
    owner_agent_id: String,
    memory_scope: String,
    scene_tags: Vec<String>,
    fact_value: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct ConflictReviewEventRow {
    fact_key: String,
    incumbent_memory_id: Uuid,
    challenger_memory_id: Uuid,
    resolution: String,
    review_status: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MaintenanceResponse {
    pub stale_marked: u64,
    pub conflicts_filtered: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GatewayStatsResponse {
    pub total: i64,
    pub episodes: i64,
    pub facts: i64,
    pub experiences: i64,
    pub private_memories: i64,
    pub shared_memories: i64,
    pub global_memories: i64,
    pub unresolved_conflicts: i64,
    pub stale_or_filtered: i64,
    pub retrieval_runs: i64,
}

fn default_agent() -> String {
    "lingshu".into()
}
fn default_layer() -> String {
    "fact".into()
}
fn default_context_scope() -> String {
    "default".into()
}

fn default_scope() -> String {
    "private".into()
}
fn default_memory_type() -> String {
    "fact".into()
}
fn default_source_type() -> String {
    "manual".into()
}
fn default_importance() -> f32 {
    0.7
}
fn default_source_trust() -> f32 {
    0.8
}
fn default_mode() -> String {
    "adaptive".into()
}
fn default_k() -> u32 {
    5
}
fn default_true() -> bool {
    true
}

fn validate_identifier(value: &str, name: &str) -> Result<(), AppError> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(AppError::Validation(format!(
            "{name} 只能包含字母、数字、_、-，且长度为 1-64"
        )));
    }
    Ok(())
}

fn validate_memory_request(req: &CreateGatewayMemoryRequest) -> Result<(), AppError> {
    validate_identifier(&req.agent_id, "agent_id")?;
    if req.context_scope.trim().is_empty() || req.context_scope.len() > 255 {
        return Err(AppError::Validation(
            "context_scope 必须为 1-255 字节".into(),
        ));
    }
    if let Some(key) = &req.request_id {
        if key.trim().is_empty() || key.len() > 255 {
            return Err(AppError::Validation("request_id 必须为 1-255 字节".into()));
        }
    }
    if req
        .valid_to
        .zip(req.valid_from.or(req.occurred_at))
        .is_some_and(|(end, start)| end <= start)
    {
        return Err(AppError::Validation("valid_to 必须晚于 valid_from".into()));
    }
    if !["episode", "fact", "experience"].contains(&req.memory_layer.as_str()) {
        return Err(AppError::Validation(
            "memory_layer 必须是 episode、fact 或 experience".into(),
        ));
    }
    if !["private", "shared", "global"].contains(&req.memory_scope.as_str()) {
        return Err(AppError::Validation(
            "memory_scope 必须是 private、shared 或 global".into(),
        ));
    }
    if req.content.trim().is_empty() {
        return Err(AppError::Validation("content 不能为空".into()));
    }
    if !(0.0..=1.0).contains(&req.importance) || !(0.0..=1.0).contains(&req.source_trust) {
        return Err(AppError::Validation(
            "importance 和 source_trust 必须在 0-1 之间".into(),
        ));
    }
    if req.memory_layer == "fact" && req.fact_key.as_deref().unwrap_or("").trim().is_empty() {
        return Err(AppError::Validation("结构化事实必须提供 fact_key".into()));
    }
    if req.memory_layer == "fact" && req.fact_value.as_deref().unwrap_or("").trim().is_empty() {
        return Err(AppError::Validation("结构化事实必须提供 fact_value".into()));
    }
    Ok(())
}

async fn ensure_agent(db: &PgPool, user_id: Uuid, agent_id: &str) -> Result<(), AppError> {
    validate_identifier(agent_id, "agent_id")?;
    sqlx::query(
        "INSERT INTO memory_agents (user_id, agent_key, display_name, role_description) \
         VALUES ($1, $2, $2, '自动注册的记忆访问主体') ON CONFLICT (user_id, agent_key) DO NOTHING",
    )
    .bind(user_id)
    .bind(agent_id)
    .execute(db)
    .await?;
    Ok(())
}

#[utoipa::path(get, path = "/api/v1/memory-gateway/agents", responses((status = 200, body = Vec<AgentResponse>)))]
pub async fn list_agents(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<Vec<AgentResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;
    ensure_agent(&state.db, user_id, "lingshu").await?;
    let rows = sqlx::query_as(
        "SELECT agent_key, display_name, role_description, is_active, created_at FROM memory_agents \
         WHERE user_id = $1 ORDER BY created_at",
    ).bind(user_id).fetch_all(&state.db).await?;
    Ok(Json(rows))
}

#[utoipa::path(post, path = "/api/v1/memory-gateway/agents", request_body = CreateAgentRequest, responses((status = 201, body = AgentResponse)))]
pub async fn create_agent(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Json(req): Json<CreateAgentRequest>,
) -> Result<(axum::http::StatusCode, Json<AgentResponse>), AppError> {
    let user_id = auth::require_user(auth).await?;
    validate_identifier(&req.agent_key, "agent_key")?;
    if req.display_name.trim().is_empty() {
        return Err(AppError::Validation("display_name 不能为空".into()));
    }
    let row = sqlx::query_as(
        "INSERT INTO memory_agents (user_id, agent_key, display_name, role_description) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (user_id, agent_key) DO UPDATE SET display_name = EXCLUDED.display_name, role_description = EXCLUDED.role_description, updated_at = NOW() \
         RETURNING agent_key, display_name, role_description, is_active, created_at",
    ).bind(user_id).bind(&req.agent_key).bind(req.display_name.trim()).bind(req.role_description.trim()).fetch_one(&state.db).await?;
    Ok((axum::http::StatusCode::CREATED, Json(row)))
}

const MEMORY_SELECT: &str = "SELECT m.context_scope, m.id, m.memory_type, m.content, m.importance, m.access_count, \
    m.memory_layer, m.owner_agent_id, m.memory_scope, m.scene_tags, m.episode_id, m.occurred_at, \
    m.valid_from, m.valid_to, m.source_type, m.source_trust, m.fact_key, m.fact_value, \
    m.conflict_status, m.superseded_by, m.retention_status, m.source_memory_ids, m.created_at, m.updated_at, \
    EXISTS(SELECT 1 FROM memory_access_grants g WHERE g.memory_id = m.id AND g.grantee_agent_id = $2 AND g.can_read) AS has_grant \
    FROM memories m WHERE m.user_id = $1 AND m.deleted_at IS NULL";

async fn accessible_rows(
    db: &PgPool,
    user_id: Uuid,
    agent_id: &str,
    limit: i64,
    context_scope: Option<&str>,
) -> Result<Vec<GatewayMemoryRow>, sqlx::Error> {
    // Apply the ACL inside PostgreSQL before LIMIT. Filtering after truncation
    // could hide an older granted memory merely because newer inaccessible rows
    // occupied the first page.
    let sql = format!(
        "{MEMORY_SELECT} AND (m.owner_agent_id = $2 OR m.memory_scope = 'global' OR \
         (m.memory_scope = 'shared' AND EXISTS(SELECT 1 FROM memory_access_grants acl \
          WHERE acl.memory_id = m.id AND acl.grantee_agent_id = $2 AND acl.can_read))) \
         AND ($4::text IS NULL OR m.context_scope=$4) ORDER BY m.occurred_at DESC LIMIT $3"
    );
    sqlx::query_as(&sql)
        .bind(user_id)
        .bind(agent_id)
        .bind(limit)
        .bind(context_scope)
        .fetch_all(db)
        .await
}

#[utoipa::path(get, path = "/api/v1/memory-gateway/memories", params(("agent_id" = Option<String>, Query), ("layer" = Option<String>, Query), ("scope" = Option<String>, Query), ("context_scope" = Option<String>, Query), ("limit" = Option<i64>, Query)), responses((status = 200, body = Vec<GatewayMemoryResponse>)))]
pub async fn list_gateway_memories(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Query(params): Query<GatewayListParams>,
) -> Result<Json<Vec<GatewayMemoryResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;
    ensure_agent(&state.db, user_id, &params.agent_id).await?;
    let rows = accessible_rows(
        &state.db,
        user_id,
        &params.agent_id,
        params.limit.unwrap_or(100).clamp(1, 500),
        params.context_scope.as_deref().map(str::trim),
    )
    .await?;
    let memories = rows
        .into_iter()
        .filter(|row| {
            can_read_memory(
                &params.agent_id,
                &row.owner_agent_id,
                &row.memory_scope,
                row.has_grant,
            ) && params
                .layer
                .as_ref()
                .map(|value| value == &row.memory_layer)
                .unwrap_or(true)
                && params
                    .scope
                    .as_ref()
                    .map(|value| value == &row.memory_scope)
                    .unwrap_or(true)
        })
        .map(|row| row.response())
        .collect();
    Ok(Json(memories))
}

#[utoipa::path(post, path = "/api/v1/memory-gateway/memories", request_body = CreateGatewayMemoryRequest, responses((status = 201, body = GatewayMemoryResponse)))]
pub async fn create_gateway_memory(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Json(req): Json<CreateGatewayMemoryRequest>,
) -> Result<(axum::http::StatusCode, Json<GatewayMemoryResponse>), AppError> {
    let user_id = auth::require_user(auth).await?;
    validate_memory_request(&req)?;
    ensure_agent(&state.db, user_id, &req.agent_id).await?;
    let memory_id = persist_gateway_memory(&state.db, user_id, &req).await?;

    let sql = format!("{MEMORY_SELECT} AND m.id = $3");
    let row: GatewayMemoryRow = sqlx::query_as(&sql)
        .bind(user_id)
        .bind(&req.agent_id)
        .bind(memory_id)
        .fetch_one(&state.db)
        .await?;

    if let Some(qdrant) = state.vector.clone() {
        let db = state.db.clone();
        let llm = state.llm.clone();
        let model = state.config.llm.embed_model.clone();
        let content = req.content.clone();
        tokio::spawn(async move {
            if let Err(error) = crate::llm::memory::upsert_memory_vector(
                &db, &qdrant, &llm, &model, user_id, memory_id, &content,
            )
            .await
            {
                tracing::warn!(%memory_id, %error, "competition memory vector indexing failed (non-fatal)");
            }
        });
    }

    Ok((axum::http::StatusCode::CREATED, Json(row.response())))
}

// All writers and reviewers acquire this lock before locking memory/event rows.
// Locking an existing row alone cannot serialize the first write to a fact.
async fn lock_key(tx: &mut Transaction<'_, Postgres>, key: &str) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(key)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn lock_fact(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    scope: &str,
    key: &str,
) -> Result<(), sqlx::Error> {
    lock_key(
        tx,
        &serde_json::json!(["fact", user, scope, key]).to_string(),
    )
    .await
}

async fn persist_gateway_memory(
    db: &PgPool,
    user_id: Uuid,
    req: &CreateGatewayMemoryRequest,
) -> Result<Uuid, AppError> {
    validate_memory_request(req)?;
    let now = Utc::now();
    let occurred_at = req.occurred_at.unwrap_or(now);
    let valid_from = req.valid_from.unwrap_or(occurred_at);
    if req.valid_to.is_some_and(|end| end <= valid_from) {
        return Err(AppError::Validation("valid_to 必须晚于 valid_from".into()));
    }
    let mut tx = db.begin().await?;
    if let Some(request_id) = &req.request_id {
        lock_key(
            &mut tx,
            &serde_json::json!(["request", user_id, req.agent_id, request_id]).to_string(),
        )
        .await?;
        let prior: Option<(Uuid, serde_json::Value)> = sqlx::query_as(
            "SELECT memory_id, payload FROM memory_write_requests WHERE user_id=$1 AND agent_id=$2 AND request_id=$3"
        ).bind(user_id).bind(&req.agent_id).bind(request_id).fetch_optional(&mut *tx).await?;
        if let Some((id, payload)) = prior {
            if payload != serde_json::to_value(req).map_err(|e| AppError::Internal(e.into()))? {
                return Err(AppError::Conflict("request_id 已用于不同内容".into()));
            }
            tx.commit().await?;
            return Ok(id);
        }
    }
    if let Some(key) = &req.fact_key {
        lock_fact(&mut tx, user_id, req.context_scope.trim(), key.trim()).await?;
    }
    let memory_id: Uuid = sqlx::query_scalar(
        "INSERT INTO memories (user_id, memory_type, content, importance, source_memory_ids, tier, memory_layer, owner_agent_id, memory_scope, scene_tags, episode_id, occurred_at, valid_from, valid_to, source_type, source_trust, fact_key, fact_value, context_scope) \
         VALUES ($1,$2,$3,$4,$5,CASE WHEN $6 = 'experience' THEN 'derived' ELSE 'raw' END,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18) RETURNING id",
    ).bind(user_id).bind(&req.memory_type).bind(req.content.trim()).bind(req.importance).bind(&req.source_memory_ids)
      .bind(&req.memory_layer).bind(&req.agent_id).bind(&req.memory_scope).bind(&req.scene_tags).bind(req.episode_id)
      .bind(occurred_at).bind(valid_from).bind(req.valid_to).bind(&req.source_type).bind(req.source_trust)
      .bind(req.fact_key.as_deref().map(str::trim)).bind(req.fact_value.as_deref().map(str::trim)).bind(req.context_scope.trim()).fetch_one(&mut *tx).await?;

    if req.memory_layer == "fact" {
        if let Some(fact_key) = req.fact_key.as_deref() {
            resolve_new_fact_conflicts(&mut tx, user_id, memory_id, fact_key.trim()).await?;
        }
    }
    if let Some(request_id) = &req.request_id {
        sqlx::query("INSERT INTO memory_write_requests(user_id,agent_id,request_id,payload,memory_id) VALUES($1,$2,$3,$4,$5)")
            .bind(user_id).bind(&req.agent_id).bind(request_id)
            .bind(serde_json::to_value(req).map_err(|e| AppError::Internal(e.into()))?).bind(memory_id)
            .execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(memory_id)
}

pub(crate) async fn resolve_new_fact_conflicts(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    challenger_id: Uuid,
    fact_key: &str,
) -> Result<(), sqlx::Error> {
    let scope: String =
        sqlx::query_scalar("SELECT context_scope FROM memories WHERE id=$1 AND user_id=$2")
            .bind(challenger_id)
            .bind(user_id)
            .fetch_one(&mut **tx)
            .await?;
    lock_fact(tx, user_id, &scope, fact_key).await?;
    let challenger: GatewayMemoryRow = sqlx::query_as(&format!("{MEMORY_SELECT} AND m.id=$3"))
        .bind(user_id)
        .bind("")
        .bind(challenger_id)
        .fetch_one(&mut **tx)
        .await?;
    if challenger.conflict_status == "superseded" {
        return Ok(());
    }
    // Only equal visibility domains can affect one another. Shared records
    // require the same effective readership, including their owners.
    let candidates: Vec<Uuid> = sqlx::query_scalar(
        "SELECT m.id FROM memories m WHERE m.user_id=$1 AND m.context_scope=$2 AND m.fact_key=$3
         AND m.id<>$4 AND m.deleted_at IS NULL AND m.memory_layer='fact'
         AND m.conflict_status IN ('active','disputed') AND m.retention_status='active'
         AND m.memory_scope=$5
         AND (m.memory_scope='global' OR (m.memory_scope='private' AND m.owner_agent_id=$6)
           OR (m.memory_scope='shared' AND
             ARRAY(SELECT DISTINCT a FROM (SELECT m.owner_agent_id::text AS a UNION SELECT grantee_agent_id FROM memory_access_grants WHERE memory_id=m.id AND can_read) x ORDER BY a)
             = ARRAY(SELECT DISTINCT a FROM (SELECT $6::text AS a UNION SELECT grantee_agent_id FROM memory_access_grants WHERE memory_id=$4 AND can_read) x ORDER BY a)))
         AND m.valid_from < COALESCE($8, 'infinity'::timestamptz)
         AND $7 < COALESCE(m.valid_to, 'infinity'::timestamptz)
         ORDER BY m.id FOR UPDATE"
    ).bind(user_id).bind(&scope).bind(fact_key).bind(challenger_id)
     .bind(&challenger.memory_scope).bind(&challenger.owner_agent_id)
     .bind(challenger.valid_from).bind(challenger.valid_to).fetch_all(&mut **tx).await?;
    for incumbent_id in candidates {
        let incumbent: GatewayMemoryRow = sqlx::query_as(&format!("{MEMORY_SELECT} AND m.id=$3"))
            .bind(user_id)
            .bind("")
            .bind(incumbent_id)
            .fetch_one(&mut **tx)
            .await?;
        if incumbent.fact_value == challenger.fact_value {
            continue;
        }
        // Agent-provided confidence and arrival order are not authority.
        // Preserve both claims until an explicit review resolves the conflict.
        sqlx::query(
            "UPDATE memories SET conflict_status='disputed', updated_at=NOW() WHERE id=ANY($1)",
        )
        .bind(vec![incumbent_id, challenger_id])
        .execute(&mut **tx)
        .await?;
        sqlx::query("INSERT INTO memory_conflict_events(user_id,fact_key,incumbent_memory_id,challenger_memory_id,resolution,reason,score_delta,review_status)
            SELECT $1,$2,$3,$4,'disputed','同一作用域和有效时间内存在不同主张，等待证据或用户裁决',0,'pending'
            WHERE NOT EXISTS(SELECT 1 FROM memory_conflict_events WHERE user_id=$1 AND review_status='pending'
            AND ((incumbent_memory_id=$3 AND challenger_memory_id=$4) OR (incumbent_memory_id=$4 AND challenger_memory_id=$3)))")
            .bind(user_id).bind(fact_key).bind(incumbent_id).bind(challenger_id).execute(&mut **tx).await?;
    }
    Ok(())
}

/// Re-run conflict governance for a memory enriched after the canonical write
/// path (for example an LLM-extracted structured fact).
#[cfg(test)]
pub(crate) async fn reconcile_memory_conflicts(
    db: &PgPool,
    user_id: Uuid,
    memory_id: Uuid,
    fact_key: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    resolve_new_fact_conflicts(&mut tx, user_id, memory_id, fact_key.trim()).await?;
    tx.commit().await?;
    Ok(())
}

#[utoipa::path(post, path = "/api/v1/memory-gateway/memories/{id}/grants", request_body = GrantMemoryRequest, responses((status = 200, body = GrantMemoryResponse)))]
pub async fn grant_memory_access(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<GrantMemoryRequest>,
) -> Result<Json<GrantMemoryResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    validate_identifier(&req.owner_agent_id, "owner_agent_id")?;
    validate_identifier(&req.grantee_agent_id, "grantee_agent_id")?;
    ensure_agent(&state.db, user_id, &req.grantee_agent_id).await?;
    let mut tx = state.db.begin().await?;
    let identity: Option<(String, Option<String>)> = sqlx::query_as("SELECT context_scope,fact_key FROM memories WHERE id=$1 AND user_id=$2 AND owner_agent_id=$3 AND deleted_at IS NULL")
        .bind(id).bind(user_id).bind(&req.owner_agent_id).fetch_optional(&mut *tx).await?;
    let (scope, key) =
        identity.ok_or_else(|| AppError::Forbidden("只有记忆所有者 Agent 可以授权共享".into()))?;
    if let Some(key) = &key {
        lock_fact(&mut tx, user_id, &scope, key).await?;
    }
    let owned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM memories WHERE id=$1 AND user_id=$2 AND owner_agent_id=$3 AND deleted_at IS NULL)")
        .bind(id).bind(user_id).bind(&req.owner_agent_id).fetch_one(&mut *tx).await?;
    if !owned {
        return Err(AppError::Forbidden(
            "只有记忆所有者 Agent 可以授权共享".into(),
        ));
    }
    sqlx::query(
        "INSERT INTO memory_access_grants (memory_id, grantee_agent_id, can_read, can_write, can_share) VALUES ($1,$2,$3,$4,$5) \
         ON CONFLICT (memory_id, grantee_agent_id) DO UPDATE SET can_read=EXCLUDED.can_read, can_write=EXCLUDED.can_write, can_share=EXCLUDED.can_share",
    ).bind(id).bind(&req.grantee_agent_id).bind(req.can_read).bind(req.can_write).bind(req.can_share).execute(&mut *tx).await?;
    sqlx::query("UPDATE memories SET memory_scope='shared', updated_at=NOW() WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if let Some(key) = &key {
        resolve_new_fact_conflicts(&mut tx, user_id, id, key).await?;
    }
    tx.commit().await?;
    Ok(Json(GrantMemoryResponse {
        memory_id: id,
        grantee_agent_id: req.grantee_agent_id,
        can_read: req.can_read,
        can_write: req.can_write,
        can_share: req.can_share,
    }))
}

#[utoipa::path(post, path = "/api/v1/memory-gateway/search", request_body = GatewaySearchRequest, responses((status = 200, body = GatewaySearchResponse)))]
pub async fn gateway_search(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Json(req): Json<GatewaySearchRequest>,
) -> Result<Json<GatewaySearchResponse>, AppError> {
    let started = Instant::now();
    let user_id = auth::require_user(auth).await?;
    if req.query.trim().is_empty() {
        return Err(AppError::Validation("query 不能为空".into()));
    }
    if !["static", "adaptive"].contains(&req.mode.as_str()) {
        return Err(AppError::Validation(
            "mode 必须是 static 或 adaptive".into(),
        ));
    }
    ensure_agent(&state.db, user_id, &req.agent_id).await?;
    let requested_k = req.top_k.clamp(1, 50);
    let rows = accessible_rows(
        &state.db,
        user_id,
        &req.agent_id,
        500,
        req.context_scope.as_deref().map(str::trim),
    )
    .await?;
    let accessible: Vec<GatewayMemoryRow> = rows
        .into_iter()
        .filter(|row| {
            can_read_memory(
                &req.agent_id,
                &row.owner_agent_id,
                &row.memory_scope,
                row.has_grant,
            ) && req
                .layer
                .as_ref()
                .map(|layer| layer == &row.memory_layer)
                .unwrap_or(true)
        })
        .collect();
    let candidate_count = accessible.len();

    let semantic_ids = if let Some(qdrant) = &state.vector {
        crate::llm::semantic::semantic_memory_search(
            qdrant,
            &state.llm,
            &state.config.llm.embed_model,
            user_id,
            &req.query,
            (requested_k * 8).min(200),
        )
        .await
        .unwrap_or_default()
    } else {
        Vec::new()
    };
    let semantic_scores: HashMap<Uuid, f32> = semantic_ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let score = 1.0 - index as f32 / semantic_ids.len().max(1) as f32;
            (*id, score)
        })
        .collect();
    let now = Utc::now();
    let weights = req
        .weights
        .clone()
        .unwrap_or_else(|| RetrievalWeights::for_query(&req.query))
        .normalized();
    let requested_tags: HashSet<&str> = req.scene_tags.iter().map(String::as_str).collect();
    let mut filtered_count = 0usize;
    let mut ranked: Vec<(GatewayMemoryRow, ScoreBreakdown)> = Vec::new();
    let mut disputed_memories = Vec::new();
    for row in accessible {
        let rankable = RankableMemory {
            id: row.id,
            content: row.content.clone(),
            importance: row.importance,
            access_count: row.access_count,
            occurred_at: row.occurred_at,
            valid_to: row.valid_to,
            source_trust: row.source_trust,
            conflict_status: row.conflict_status.clone(),
            retention_status: row.retention_status.clone(),
            semantic_score: semantic_scores.get(&row.id).copied().unwrap_or(0.0),
        };
        if row.conflict_status == "disputed"
            && row.valid_from <= now
            && row.valid_to.map(|end| end > now).unwrap_or(true)
            && (crate::llm::evolutionary_memory::lexical_similarity(&req.query, &row.content) > 0.0
                || rankable.semantic_score > 0.0)
        {
            disputed_memories.push(row.response());
        }
        if req.mode == "adaptive"
            && (row.valid_from > now || !is_retrieval_eligible(&rankable, now))
        {
            filtered_count += 1;
            continue;
        }
        let mut breakdown = score_memory(&req.query, &rankable, &weights, now);
        if req.mode == "static" {
            breakdown.total = rankable.semantic_score * 0.7 + row.importance * 0.3;
        } else if !requested_tags.is_empty()
            && row
                .scene_tags
                .iter()
                .any(|tag| requested_tags.contains(tag.as_str()))
        {
            breakdown.total = (breakdown.total + 0.08).min(1.0);
        }
        ranked.push((row, breakdown));
    }
    ranked.sort_by(|left, right| {
        right
            .1
            .total
            .total_cmp(&left.1.total)
            .then_with(|| right.0.occurred_at.cmp(&left.0.occurred_at))
    });
    ranked.truncate(requested_k as usize);
    disputed_memories.truncate(requested_k as usize);
    let ids: Vec<Uuid> = ranked.iter().map(|(row, _)| row.id).collect();
    if !ids.is_empty() {
        sqlx::query("UPDATE memories SET access_count=access_count+1, last_accessed_at=NOW() WHERE id=ANY($1) AND user_id=$2").bind(&ids).bind(user_id).execute(&state.db).await?;
    }
    let estimated_tokens = estimate_tokens(ranked.iter().map(|(row, _)| row.content.clone()));
    let latency_ms = started.elapsed().as_secs_f32() * 1000.0;
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO memory_retrieval_runs (user_id, agent_id, query, mode, requested_k, returned_count, filtered_count, latency_ms, estimated_tokens, weights, result_ids) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) RETURNING id",
    ).bind(user_id).bind(&req.agent_id).bind(req.query.trim()).bind(&req.mode).bind(requested_k as i32).bind(ranked.len() as i32)
      .bind(filtered_count as i32).bind(latency_ms).bind(estimated_tokens).bind(serde_json::to_value(&weights).unwrap_or_default()).bind(&ids).fetch_one(&state.db).await?;
    let results = ranked
        .into_iter()
        .enumerate()
        .map(|(index, (row, breakdown))| RankedMemoryResponse {
            memory: row.response(),
            rank: index + 1,
            score: breakdown.total,
            score_breakdown: breakdown,
        })
        .collect();
    Ok(Json(GatewaySearchResponse {
        run_id,
        mode: req.mode,
        requested_k,
        candidate_count,
        filtered_count,
        latency_ms,
        estimated_tokens,
        weights,
        results,
        disputed_memories,
    }))
}

const CONFLICT_EVENT_SELECT: &str = "SELECT e.id, e.fact_key, e.incumbent_memory_id, e.challenger_memory_id, \
    e.resolution, e.reason, e.score_delta, incumbent.fact_value AS incumbent_value, \
    challenger.fact_value AS challenger_value, e.review_status, e.reviewer_agent_id, e.review_note, \
    e.reviewed_at, e.experience_memory_id, e.created_at \
    FROM memory_conflict_events e \
    JOIN memories incumbent ON incumbent.id = e.incumbent_memory_id \
    JOIN memories challenger ON challenger.id = e.challenger_memory_id";

async fn conflict_event_by_id(
    db: &PgPool,
    user_id: Uuid,
    conflict_id: Uuid,
) -> Result<ConflictEventResponse, AppError> {
    let sql = format!("{CONFLICT_EVENT_SELECT} WHERE e.user_id = $1 AND e.id = $2");
    sqlx::query_as(&sql)
        .bind(user_id)
        .bind(conflict_id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::NotFound("冲突记录不存在".into()))
}

fn reviewed_experience_content(
    fact_key: &str,
    winner: &ConflictReviewMemoryRow,
    loser: &ConflictReviewMemoryRow,
    note: Option<&str>,
) -> String {
    let winner_value = winner.fact_value.as_deref().unwrap_or(&winner.content);
    let loser_value = loser.fact_value.as_deref().unwrap_or(&loser.content);
    let rationale = note
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("人工复核确认");
    format!(
        "事实「{fact_key}」的冲突已裁决：后续采用「{winner_value}」，不采用「{loser_value}」。依据：{rationale}。"
    )
}

fn conservative_experience_scope(left: &str, right: &str) -> &'static str {
    match (left, right) {
        ("global", "global") => "global",
        ("shared", "shared") => "shared",
        _ => "private",
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/memory-gateway/conflicts/{id}/resolve",
    params(("id" = Uuid, Path)),
    request_body = ResolveConflictRequest,
    responses((status = 200, body = ResolveConflictResponse))
)]
pub async fn resolve_conflict_review(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<ResolveConflictRequest>,
) -> Result<Json<ResolveConflictResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    validate_identifier(&req.reviewer_agent_id, "reviewer_agent_id")?;
    ensure_agent(&state.db, user_id, &req.reviewer_agent_id).await?;
    if !["incumbent", "challenger"].contains(&req.winner.as_str()) {
        return Err(AppError::Validation(
            "winner 必须是 incumbent 或 challenger".into(),
        ));
    }

    let mut tx = state.db.begin().await?;
    let identity: (String, String) = sqlx::query_as("SELECT m.context_scope,e.fact_key FROM memory_conflict_events e JOIN memories m ON m.id=e.challenger_memory_id WHERE e.id=$1 AND e.user_id=$2")
        .bind(id).bind(user_id).fetch_optional(&mut *tx).await?
        .ok_or_else(|| AppError::NotFound("冲突记录不存在".into()))?;
    lock_fact(&mut tx, user_id, &identity.0, &identity.1).await?;
    let event: ConflictReviewEventRow = sqlx::query_as(
        "SELECT fact_key, incumbent_memory_id, challenger_memory_id, resolution, review_status \
         FROM memory_conflict_events WHERE id = $1 AND user_id = $2 FOR UPDATE",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("冲突记录不存在".into()))?;

    if event.resolution != "disputed" || event.review_status != "pending" {
        return Err(AppError::Conflict("该冲突已完成裁决或无需人工裁决".into()));
    }

    let (winner_id, loser_id, resolution) = if req.winner == "incumbent" {
        (
            event.incumbent_memory_id,
            event.challenger_memory_id,
            "incumbent_wins",
        )
    } else {
        (
            event.challenger_memory_id,
            event.incumbent_memory_id,
            "challenger_wins",
        )
    };
    let winner: ConflictReviewMemoryRow = sqlx::query_as(
        "SELECT id, content, memory_type, importance, owner_agent_id, memory_scope, scene_tags, fact_value \
         FROM memories WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(winner_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("获胜候选记忆不存在".into()))?;
    let loser: ConflictReviewMemoryRow = sqlx::query_as(
        "SELECT id, content, memory_type, importance, owner_agent_id, memory_scope, scene_tags, fact_value \
         FROM memories WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(loser_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("落败候选记忆不存在".into()))?;

    let reviewable: bool = sqlx::query_scalar("SELECT a.conflict_status='disputed' AND b.conflict_status='disputed'
        AND a.context_scope=b.context_scope AND a.memory_scope=b.memory_scope AND
        (a.memory_scope='global' OR (a.memory_scope='private' AND a.owner_agent_id=b.owner_agent_id)
        OR (a.memory_scope='shared' AND
        ARRAY(SELECT x FROM (SELECT a.owner_agent_id AS x UNION SELECT grantee_agent_id FROM memory_access_grants WHERE memory_id=a.id AND can_read) g ORDER BY x)
        = ARRAY(SELECT x FROM (SELECT b.owner_agent_id AS x UNION SELECT grantee_agent_id FROM memory_access_grants WHERE memory_id=b.id AND can_read) g ORDER BY x)))
        FROM memories a, memories b WHERE a.id=$1 AND b.id=$2")
        .bind(winner.id).bind(loser.id).fetch_one(&mut *tx).await?;
    if !reviewable {
        return Err(AppError::Conflict(
            "候选状态或共享权限已变化，请刷新冲突记录".into(),
        ));
    }

    sqlx::query(
        "UPDATE memories SET conflict_status='active', retention_status='active', superseded_by=NULL, updated_at=NOW() \
         WHERE id=$1 AND user_id=$2",
    )
    .bind(winner.id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE memories SET conflict_status='superseded', retention_status='filtered', \
         superseded_by=$1, updated_at=NOW() \
         WHERE id=$2 AND user_id=$3",
    )
    .bind(winner.id)
    .bind(loser.id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    // A pairwise review must not hide other unresolved alternatives.
    sqlx::query("UPDATE memories SET conflict_status='disputed' WHERE id=$1 AND EXISTS(
        SELECT 1 FROM memory_conflict_events e JOIN memories a ON a.id=e.incumbent_memory_id JOIN memories b ON b.id=e.challenger_memory_id
        WHERE e.id<>$2 AND e.review_status='pending' AND (a.id=$1 OR b.id=$1)
        AND a.conflict_status<>'superseded' AND b.conflict_status<>'superseded')")
        .bind(winner.id).bind(id).execute(&mut *tx).await?;
    let normalized_note = req
        .note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut experience_to_index: Option<(Uuid, String)> = None;
    let winner_active: bool =
        sqlx::query_scalar("SELECT conflict_status='active' FROM memories WHERE id=$1")
            .bind(winner.id)
            .fetch_one(&mut *tx)
            .await?;
    let experience_memory_id = if req.promote_to_experience && winner_active {
        let content =
            reviewed_experience_content(&event.fact_key, &winner, &loser, normalized_note);
        // A derived record contains values from both sides, so it must never be
        // more visible than either source. Mixed scopes fall back to private;
        // shared sources retain only grants present on both memories.
        let experience_scope =
            conservative_experience_scope(&winner.memory_scope, &loser.memory_scope);
        let mut scene_tags = winner.scene_tags.clone();
        if !scene_tags.iter().any(|tag| tag == "conflict-resolution") {
            scene_tags.push("conflict-resolution".into());
        }
        let source_ids = vec![winner.id, loser.id];
        let experience_id: Uuid = sqlx::query_scalar(
            "INSERT INTO memories (user_id, memory_type, content, importance, source_memory_ids, tier, \
             memory_layer, owner_agent_id, memory_scope, scene_tags, source_type, source_trust, context_scope) \
             VALUES ($1,$2,$3,$4,$5,'derived','experience',$6,$7,$8,'conflict_resolution',1.0,$9) RETURNING id",
        )
        .bind(user_id)
        .bind(&winner.memory_type)
        .bind(&content)
        .bind(winner.importance.clamp(0.8, 1.0))
        .bind(&source_ids)
        .bind(&winner.owner_agent_id)
        .bind(experience_scope)
        .bind(&scene_tags)
        .bind(&identity.0)
        .fetch_one(&mut *tx)
        .await?;
        if experience_scope == "shared" {
            sqlx::query(
                "INSERT INTO memory_access_grants (memory_id, grantee_agent_id, can_read, can_write, can_share) \
                 SELECT $1, winner_grant.grantee_agent_id, \
                        winner_grant.can_read AND loser_grant.can_read, \
                        winner_grant.can_write AND loser_grant.can_write, \
                        winner_grant.can_share AND loser_grant.can_share \
                 FROM memory_access_grants winner_grant \
                 JOIN memory_access_grants loser_grant \
                   ON loser_grant.grantee_agent_id = winner_grant.grantee_agent_id \
                  AND loser_grant.memory_id = $3 \
                 WHERE winner_grant.memory_id = $2 \
                   AND winner_grant.can_read AND loser_grant.can_read \
                 ON CONFLICT (memory_id, grantee_agent_id) DO NOTHING",
            )
            .bind(experience_id)
            .bind(winner.id)
            .bind(loser.id)
            .execute(&mut *tx)
            .await?;
        }
        experience_to_index = Some((experience_id, content));
        Some(experience_id)
    } else {
        None
    };

    let reason = match normalized_note {
        Some(note) => format!("人工裁决：{note}"),
        None => "人工裁决：用户确认有效事实".into(),
    };
    sqlx::query(
        "UPDATE memory_conflict_events SET resolution=$1, reason=$2, review_status='resolved', \
         reviewer_agent_id=$3, review_note=$4, reviewed_at=NOW(), experience_memory_id=$5 WHERE id=$6 AND user_id=$7",
    )
    .bind(resolution)
    .bind(reason)
    .bind(&req.reviewer_agent_id)
    .bind(normalized_note)
    .bind(experience_memory_id)
    .bind(id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE memory_conflict_events SET review_status='resolved', reason='候选已在另一项人工裁决中被替代', reviewed_at=NOW()
        WHERE user_id=$1 AND id<>$2 AND review_status='pending' AND (incumbent_memory_id=$3 OR challenger_memory_id=$3)")
        .bind(user_id).bind(id).bind(loser.id).execute(&mut *tx).await?;
    // Equal-value peers may have disputed only the now-retired alternative.
    sqlx::query("UPDATE memories m SET conflict_status='active', updated_at=NOW()
        WHERE m.user_id=$1 AND m.context_scope=$2 AND m.fact_key=$3 AND m.conflict_status='disputed'
        AND m.deleted_at IS NULL AND EXISTS(SELECT 1 FROM memory_conflict_events e WHERE e.review_status='resolved'
            AND ((e.incumbent_memory_id=m.id AND e.challenger_memory_id=$4) OR (e.challenger_memory_id=m.id AND e.incumbent_memory_id=$4)))
        AND NOT EXISTS(SELECT 1 FROM memory_conflict_events e WHERE e.review_status='pending'
            AND (e.incumbent_memory_id=m.id OR e.challenger_memory_id=m.id))")
        .bind(user_id).bind(&identity.0).bind(&identity.1).bind(loser.id).execute(&mut *tx).await?;
    tx.commit().await?;

    crate::telemetry::record(
        &state.db,
        user_id,
        crate::telemetry::SignalEventType::MemoryConflictResolved,
        Some("memory_conflict"),
        Some(id),
        serde_json::json!({
            "winner_memory_id": winner.id,
            "loser_memory_id": loser.id,
            "reviewer_agent_id": req.reviewer_agent_id,
            "experience_memory_id": experience_memory_id,
        }),
    )
    .await;

    if let (Some(qdrant), Some((memory_id, content))) = (state.vector.clone(), experience_to_index)
    {
        let db = state.db.clone();
        let llm = state.llm.clone();
        let model = state.config.llm.embed_model.clone();
        tokio::spawn(async move {
            if let Err(error) = crate::llm::memory::upsert_memory_vector(
                &db, &qdrant, &llm, &model, user_id, memory_id, &content,
            )
            .await
            {
                tracing::warn!(%memory_id, %error, "reviewed conflict experience indexing failed (non-fatal)");
            }
        });
    }

    let conflict = conflict_event_by_id(&state.db, user_id, id).await?;
    Ok(Json(ResolveConflictResponse {
        conflict,
        winner_memory_id: winner.id,
        loser_memory_id: loser.id,
        experience_memory_id,
    }))
}

#[utoipa::path(get, path = "/api/v1/memory-gateway/conflicts", responses((status = 200, body = Vec<ConflictEventResponse>)))]
pub async fn list_conflicts(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<Vec<ConflictEventResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let sql =
        format!("{CONFLICT_EVENT_SELECT} WHERE e.user_id=$1 ORDER BY e.created_at DESC LIMIT 200");
    let rows = sqlx::query_as(&sql)
        .bind(user_id)
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows))
}

#[utoipa::path(post, path = "/api/v1/memory-gateway/maintenance", responses((status = 200, body = MaintenanceResponse)))]
pub async fn run_maintenance(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<MaintenanceResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let stale_marked = sqlx::query(
        "UPDATE memories SET retention_status='stale', updated_at=NOW() WHERE user_id=$1 AND deleted_at IS NULL AND retention_status='active' AND valid_to < NOW()",
    ).bind(user_id).execute(&state.db).await?.rows_affected();
    let conflicts_filtered = sqlx::query(
        "UPDATE memories SET retention_status='filtered', updated_at=NOW() WHERE user_id=$1 AND deleted_at IS NULL AND conflict_status='superseded' AND retention_status <> 'filtered'",
    ).bind(user_id).execute(&state.db).await?.rows_affected();
    Ok(Json(MaintenanceResponse {
        stale_marked,
        conflicts_filtered,
    }))
}

#[utoipa::path(get, path = "/api/v1/memory-gateway/stats", responses((status = 200, body = GatewayStatsResponse)))]
pub async fn gateway_stats(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<GatewayStatsResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let row: (i64,i64,i64,i64,i64,i64,i64,i64,i64) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(*) FILTER (WHERE memory_layer='episode'), COUNT(*) FILTER (WHERE memory_layer='fact'), \
         COUNT(*) FILTER (WHERE memory_layer='experience'), COUNT(*) FILTER (WHERE memory_scope='private'), \
         COUNT(*) FILTER (WHERE memory_scope='shared'), COUNT(*) FILTER (WHERE memory_scope='global'), \
         COUNT(*) FILTER (WHERE conflict_status='disputed'), COUNT(*) FILTER (WHERE retention_status IN ('stale','filtered')) \
         FROM memories WHERE user_id=$1 AND deleted_at IS NULL",
    ).bind(user_id).fetch_one(&state.db).await?;
    let retrieval_runs: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM memory_retrieval_runs WHERE user_id=$1")
            .bind(user_id)
            .fetch_one(&state.db)
            .await?;
    Ok(Json(GatewayStatsResponse {
        total: row.0,
        episodes: row.1,
        facts: row.2,
        experiences: row.3,
        private_memories: row.4,
        shared_memories: row.5,
        global_memories: row.6,
        unresolved_conflicts: row.7,
        stale_or_filtered: row.8,
        retrieval_runs,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_defaults_to_adaptive_lingshu_fact() {
        let request: GatewaySearchRequest =
            serde_json::from_value(serde_json::json!({"query":"当前技术栈"})).unwrap();
        assert_eq!(request.agent_id, "lingshu");
        assert_eq!(request.mode, "adaptive");
        assert_eq!(request.top_k, 5);
    }

    #[test]
    fn fact_write_requires_fact_key() {
        let request: CreateGatewayMemoryRequest =
            serde_json::from_value(serde_json::json!({"content":"使用 Rust"})).unwrap();
        assert!(validate_memory_request(&request).is_err());
    }

    #[test]
    fn identifiers_reject_sql_and_whitespace() {
        assert!(validate_identifier("writer_agent", "agent_id").is_ok());
        assert!(validate_identifier("writer agent", "agent_id").is_err());
        assert!(validate_identifier("x';drop", "agent_id").is_err());
    }

    #[test]
    fn conflict_review_defaults_to_promoting_experience() {
        let request: ResolveConflictRequest = serde_json::from_value(serde_json::json!({
            "winner": "challenger",
            "reviewer_agent_id": "lingshu"
        }))
        .unwrap();
        assert!(request.promote_to_experience);
    }

    #[test]
    fn reviewed_experience_keeps_both_sides_and_reason() {
        let winner = ConflictReviewMemoryRow {
            id: Uuid::new_v4(),
            content: "用户目前在深圳".into(),
            memory_type: "fact".into(),
            importance: 0.8,
            owner_agent_id: "lingshu".into(),
            memory_scope: "private".into(),
            scene_tags: vec!["profile".into()],
            fact_value: Some("深圳".into()),
        };
        let loser = ConflictReviewMemoryRow {
            id: Uuid::new_v4(),
            content: "用户目前在广州".into(),
            fact_value: Some("广州".into()),
            ..winner.clone()
        };
        let content =
            reviewed_experience_content("profile.city", &winner, &loser, Some("用户明确确认"));
        assert!(content.contains("采用「深圳」"));
        assert!(content.contains("不采用「广州」"));
        assert!(content.contains("用户明确确认"));
    }

    #[test]
    fn conflict_experience_never_broadens_source_scope() {
        assert_eq!(conservative_experience_scope("global", "global"), "global");
        assert_eq!(conservative_experience_scope("shared", "shared"), "shared");
        assert_eq!(conservative_experience_scope("global", "shared"), "private");
        assert_eq!(
            conservative_experience_scope("shared", "private"),
            "private"
        );
        assert_eq!(
            conservative_experience_scope("private", "private"),
            "private"
        );
    }
}

#[cfg(test)]
#[path = "memory_gateway_concurrency_tests.rs"]
mod concurrency_tests;
