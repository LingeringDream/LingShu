//! Shared production recall for chat, HTTP and MCP. PostgreSQL is the final
//! authority for visibility; vector hits never grant access.
#[cfg(test)]
#[path = "retrieval_baseline_tests.rs"]
mod baseline_tests;
#[cfg(test)]
#[path = "retrieval_quality_tests.rs"]
mod quality_tests;
#[cfg(test)]
#[path = "retrieval_tests.rs"]
mod tests;
use super::evolutionary_memory::{
    estimate_tokens, is_retrieval_eligible, score_memory, RankableMemory, RetrievalWeights,
    ScoreBreakdown,
};
use crate::error::AppError;
use crate::routes::memory_gateway::{
    ensure_agent, GatewayMemoryRow, GatewaySearchRequest, GatewaySearchResponse,
    RankedMemoryResponse, MEMORY_SELECT,
};
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use uuid::Uuid;

// PostgreSQL POSIX regex, passed as a bound value. Escaping makes punctuation
// literal; ASCII boundaries prevent matching an identifier's longer prefix.
fn exact_phrase_pattern(query: &str) -> Option<String> {
    let query = query.trim().to_lowercase();
    if query.chars().filter(|c| c.is_alphanumeric()).count() < 3 {
        return None;
    }
    let mut pattern = String::new();
    if query.starts_with(|c: char| c.is_ascii_alphanumeric()) {
        pattern.push_str("(^|[^a-z0-9_-])");
    }
    for c in query.chars() {
        if ".^$*+?()[]{}\\|".contains(c) {
            pattern.push('\\');
        }
        pattern.push(c);
    }
    if query.ends_with(|c: char| c.is_ascii_alphanumeric()) {
        pattern.push_str("($|[^a-z0-9_.-]|[.]($|[^a-z0-9]))");
    }
    Some(pattern)
}

/// Retrieve relevant records across the entire authorized scope. Limits apply
/// to ranked matches, never to a recency window. Lexical and semantic channels
/// have independent budgets, so either can contribute older evidence.
async fn candidate_rows(
    db: &sqlx::PgPool,
    user_id: Uuid,
    req: &GatewaySearchRequest,
    semantic_ids: &[Uuid],
) -> Result<Vec<GatewayMemoryRow>, sqlx::Error> {
    // One/two-character substrings may share no padded trigrams with a word.
    // Retain the terms index for those queries, without per-row token counting.
    let lexical_selection = if req.query.trim().chars().count() <= 2 {
        "WHERE memory_search_terms(e.content) && memory_search_terms($5)
         ORDER BY e.occurred_at DESC, e.id"
    } else {
        "ORDER BY lower(e.content) <-> lower($5)"
    };
    let sql = format!(
        "WITH eligible AS NOT MATERIALIZED (
          {MEMORY_SELECT}
          AND NOT m.derivation_stale
          AND (m.owner_agent_id=$2 OR m.memory_scope='global'
            OR (m.memory_scope='shared' AND EXISTS (
              SELECT 1 FROM memory_access_grants g WHERE g.memory_id=m.id
              AND g.grantee_agent_id=$2 AND g.can_read)))
          AND ($3::text IS NULL OR m.context_scope=$3)
          AND ($4::text IS NULL OR m.memory_layer=$4)
          AND ($7='static' OR (m.valid_from <= COALESCE($8::timestamptz,NOW())
            AND (m.valid_to IS NULL OR m.valid_to > COALESCE($8::timestamptz,NOW()))
            AND ((m.retention_status='active' AND m.conflict_status IN ('active','disputed'))
              OR ($8 IS NOT NULL AND (m.retention_status='stale'
                OR m.conflict_status='superseded')))))
        ),
        nearest AS MATERIALIZED (
          SELECT e.* FROM eligible e
          {lexical_selection}
          LIMIT 200
        ),
        lexical AS (
          SELECT * FROM nearest
          WHERE memory_search_terms(content) && memory_search_terms($5)
        ),
        exact AS MATERIALIZED (
          SELECT e.* FROM eligible e
          WHERE $9::text IS NOT NULL AND lower(e.content) ~ $9
          ORDER BY lower(e.content) <-> lower($5)
          LIMIT 50
        ),
        semantic AS (
          SELECT e.* FROM eligible e WHERE e.id=ANY($6::uuid[])
        )
        SELECT * FROM lexical UNION SELECT * FROM exact UNION SELECT * FROM semantic"
    );
    // Keep tokenization behind the materialized candidate budget. Eligibility
    // stays before LIMIT so inaccessible records cannot crowd out visible ones.
    // A local timeout also bounds expensive plans for very selective scopes;
    // transaction rollback prevents the setting leaking into pooled sessions.
    let mut tx = db.begin().await?;
    sqlx::query("SET LOCAL statement_timeout = '2s'")
        .execute(&mut *tx)
        .await?;
    let rows = sqlx::query_as(&sql)
        .bind(user_id)
        .bind(&req.agent_id)
        .bind(req.context_scope.as_deref().map(str::trim))
        .bind(&req.layer)
        .bind(&req.query)
        .bind(semantic_ids)
        .bind(&req.mode)
        .bind(req.as_of)
        .bind(exact_phrase_pattern(&req.query))
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(rows)
}

pub async fn search(
    db: &sqlx::PgPool,
    vector: &Option<lingshu_vector::search::QdrantClient>,
    llm: &super::client::LlmClient,
    embed_model: &str,
    user_id: Uuid,
    req: GatewaySearchRequest,
) -> Result<GatewaySearchResponse, AppError> {
    let started = Instant::now();
    if req.query.trim().is_empty() {
        return Err(AppError::Validation("query 不能为空".into()));
    }
    if !["static", "adaptive"].contains(&req.mode.as_str()) {
        return Err(AppError::Validation(
            "mode 必须是 static 或 adaptive".into(),
        ));
    }
    ensure_agent(db, user_id, &req.agent_id).await?;
    if req.as_of.is_some() && req.mode != "adaptive" {
        return Err(AppError::Validation("as_of 仅适用于 adaptive 检索".into()));
    }
    let requested_k = req.top_k.clamp(1, 50);
    let semantic_matches = if let Some(qdrant) = vector {
        // A stalled embedding service must not prevent lexical recall.
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            super::semantic::semantic_memory_search_scored(
                qdrant,
                llm,
                embed_model,
                user_id,
                &req.query,
                200,
            ),
        )
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
    } else {
        Vec::new()
    };
    let semantic_scores: HashMap<Uuid, f32> = semantic_matches.into_iter().collect();
    let semantic_ids: Vec<Uuid> = semantic_scores.keys().copied().collect();
    let accessible = candidate_rows(db, user_id, &req, &semantic_ids).await?;
    let candidate_count = accessible.len();
    let now = req.as_of.unwrap_or_else(Utc::now);
    let weights = req
        .weights
        .clone()
        .unwrap_or_else(|| RetrievalWeights::for_query(&req.query))
        .normalized();
    let requested_tags: HashSet<&str> = req.scene_tags.iter().map(String::as_str).collect();
    let mut filtered_count = 0usize;
    let mut ranked: Vec<(GatewayMemoryRow, ScoreBreakdown)> = Vec::new();
    let mut disputed_memories = Vec::new();
    let mut historical_memories = Vec::new();
    for row in accessible {
        let mut rankable = RankableMemory {
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
        if req.as_of.is_some() && row.retention_status == "stale" {
            // Expired now may still be valid at the explicitly requested time.
            rankable.retention_status = "active".into();
        }
        if req.as_of.is_some() && row.conflict_status == "superseded" {
            historical_memories.push((
                row.response(),
                score_memory(&req.query, &rankable, &weights, now).total,
            ));
        }
        if row.conflict_status == "disputed"
            && row.valid_from <= now
            && row.valid_to.map(|end| end > now).unwrap_or(true)
            && (crate::llm::evolutionary_memory::lexical_similarity(&req.query, &row.content) > 0.0
                || rankable.semantic_score > 0.0)
        {
            disputed_memories.push((
                row.response(),
                score_memory(&req.query, &rankable, &weights, now).total,
            ));
        }
        if req.mode == "adaptive"
            && (row.valid_from > now || !is_retrieval_eligible(&rankable, now))
        {
            filtered_count += 1;
            continue;
        }
        // Metadata alone must not promote an unrelated memory.
        if rankable.semantic_score <= 0.0
            && super::evolutionary_memory::lexical_similarity(&req.query, &row.content) == 0.0
        {
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
            .then_with(|| left.0.id.cmp(&right.0.id))
    });
    ranked.truncate(requested_k as usize);
    disputed_memories.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.id.cmp(&b.0.id)));
    disputed_memories.truncate(requested_k as usize);
    let disputed_memories: Vec<crate::routes::memory_gateway::GatewayMemoryResponse> =
        disputed_memories
            .into_iter()
            .map(|(memory, _)| memory)
            .collect();
    historical_memories.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.id.cmp(&b.0.id)));
    historical_memories.truncate(requested_k as usize);
    let historical_memories: Vec<crate::routes::memory_gateway::GatewayMemoryResponse> =
        historical_memories
            .into_iter()
            .map(|(memory, _)| memory)
            .collect();
    let ids: Vec<Uuid> = ranked.iter().map(|(row, _)| row.id).collect();
    if !ids.is_empty() {
        sqlx::query("UPDATE memories SET access_count=access_count+1, last_accessed_at=NOW() WHERE id=ANY($1) AND user_id=$2").bind(&ids).bind(user_id).execute(db).await?;
        // Preserve retrieval telemetry without one database round-trip per hit.
        if let Err(error) = sqlx::query(
            "INSERT INTO signal_events(user_id,event_type,entity_type,entity_id,metadata)
            SELECT $1,'memory_retrieval_hit','memory',id,$3 FROM unnest($2::uuid[]) id",
        )
        .bind(user_id)
        .bind(&ids)
        .bind(serde_json::json!({"agent_id": req.agent_id}))
        .execute(db)
        .await
        {
            tracing::warn!(%error, "Failed to record retrieval hits");
        }
    }
    let estimated_tokens = estimate_tokens(
        ranked
            .iter()
            .map(|(row, _)| row.content.clone())
            .chain(disputed_memories.iter().map(|m| m.content.clone()))
            .chain(historical_memories.iter().map(|m| m.content.clone())),
    );
    let historical_ids: Vec<Uuid> = historical_memories.iter().map(|m| m.id).collect();
    let latency_ms = started.elapsed().as_secs_f32() * 1000.0;
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO memory_retrieval_runs (user_id, agent_id, query, mode, requested_k, returned_count, filtered_count, latency_ms, estimated_tokens, weights, result_ids, as_of, historical_result_ids) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13) RETURNING id",
    ).bind(user_id).bind(&req.agent_id).bind(req.query.trim()).bind(&req.mode).bind(requested_k as i32).bind(ranked.len() as i32)
      .bind(filtered_count as i32).bind(latency_ms).bind(estimated_tokens).bind(serde_json::to_value(&weights).unwrap_or_default()).bind(&ids).bind(req.as_of).bind(&historical_ids).fetch_one(db).await?;
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
    Ok(GatewaySearchResponse {
        as_of: req.as_of,
        historical_memories,
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
    })
}
