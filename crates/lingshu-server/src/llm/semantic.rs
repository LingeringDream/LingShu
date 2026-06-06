//! Shared semantic memory search — embeds a query, searches Qdrant with
//! a user-scoped filter, and returns ranked memory IDs. Used by both the
//! chat context pipeline and the `/api/v1/memories/search` endpoint.
//!
//! All functions return `Option` on any failure so callers fall back
//! gracefully to SQL-based retrieval.

use uuid::Uuid;

use crate::llm::client::LlmClient;
use crate::llm::memory;
use lingshu_vector::search::QdrantClient;

/// Run the semantic pipeline: embed → Qdrant search → extract ranked IDs.
/// Returns `None` when Qdrant is unavailable, the embedding model fails,
/// or no results are found — the caller should fall back to SQL retrieval.
pub async fn semantic_memory_search(
    qdrant: &QdrantClient,
    llm: &LlmClient,
    embed_model: &str,
    user_id: Uuid,
    query: &str,
    limit: u32,
) -> Option<Vec<Uuid>> {
    if query.trim().is_empty() {
        return None;
    }

    let embedding = llm
        .embed(embed_model, query)
        .await
        .map_err(|e| {
            tracing::warn!(%user_id, %e, "Embedding failed for semantic memory search, falling back to SQL");
        })
        .ok()?;

    let filter = memory::build_user_filter(user_id);
    let results = qdrant
        .search("memories", embedding, limit, Some(filter))
        .await
        .map_err(|e| {
            tracing::warn!(%user_id, %e, "Qdrant search failed, falling back to SQL");
        })
        .ok()?;

    let ids = memory::extract_memory_ids(&results);
    if ids.is_empty() {
        return None;
    }
    Some(ids)
}
