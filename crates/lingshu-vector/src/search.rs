use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Qdrant vector search client
pub struct QdrantClient {
    base_url: String,
    http: reqwest::Client,
}

impl QdrantClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    /// Create a collection with HNSW index
    pub async fn create_collection(&self, name: &str, vector_size: u64) -> anyhow::Result<()> {
        let url = format!("{}/collections/{}", self.base_url, name);
        let body = serde_json::json!({
            "vectors": {
                "size": vector_size,
                "distance": "Cosine"
            },
            "optimizers_config": {
                "indexing_threshold": 20000
            }
        });

        self.http
            .put(&url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Insert a vector point
    pub async fn upsert_point(
        &self,
        collection: &str,
        id: Uuid,
        vector: Vec<f32>,
        payload: serde_json::Value,
    ) -> anyhow::Result<()> {
        let url = format!("{}/collections/{}/points", self.base_url, collection);
        let body = serde_json::json!({
            "points": [{
                "id": id.to_string(),
                "vector": vector,
                "payload": payload
            }]
        });

        self.http
            .put(&url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Search for similar vectors
    pub async fn search(
        &self,
        collection: &str,
        vector: Vec<f32>,
        limit: u32,
        filter: Option<serde_json::Value>,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let url = format!("{}/collections/{}/points/search", self.base_url, collection);
        let mut body = serde_json::json!({
            "vector": vector,
            "limit": limit,
            "with_payload": true
        });

        if let Some(f) = filter {
            body["filter"] = f;
        }

        let resp: SearchResponse = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(resp.result)
    }
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    result: Vec<SearchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub payload: Option<serde_json::Value>,
}
