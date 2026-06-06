//! Lightweight Redis cache helpers.
//!
//! All operations are best-effort: Redis errors and JSON parse failures
//! are logged via [`tracing::warn!`] and never propagate to callers.
//! When Redis is unavailable the application falls back to PostgreSQL
//! or in-memory state transparently.

use fred::interfaces::KeysInterface;
use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

use crate::state::OptionalRedis;

// ── Key builders ──────────────────────────────────────────────────

pub fn llm_settings_cache_key(user_id: Uuid) -> String {
    format!("lingshu:user:{user_id}:llm_settings:v1")
}

pub fn chat_sessions_cache_key(user_id: Uuid) -> String {
    format!("lingshu:user:{user_id}:chat_sessions:v1")
}

// ── Generic helpers ───────────────────────────────────────────────

/// Read a JSON value from Redis. Returns `None` on any failure
/// (key missing, connection error, deserialization error).
pub async fn get_json<T: DeserializeOwned>(redis: &OptionalRedis, key: &str) -> Option<T> {
    let client = redis.as_ref()?;
    let raw: Option<String> = match client.get(key).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(%key, %e, "Redis GET failed");
            return None;
        }
    };
    let raw = raw?;
    match serde_json::from_str(&raw) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!(%key, %e, "Redis JSON deserialize failed, cache miss");
            None
        }
    }
}

/// Write a JSON value to Redis with an optional TTL.
pub async fn set_json<T: Serialize>(
    redis: &OptionalRedis,
    key: &str,
    value: &T,
    ttl_seconds: Option<u64>,
) {
    let client = match redis.as_ref() {
        Some(c) => c,
        None => return,
    };
    let json = match serde_json::to_string(value) {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!(%key, %e, "Redis JSON serialize failed, skipping SET");
            return;
        }
    };
    let expiration = ttl_seconds.map(|s| fred::types::Expiration::EX(s as i64));
    if let Err(e) = client
        .set::<String, _, _>(key, json.as_str(), expiration, None, false)
        .await
    {
        tracing::warn!(%key, %e, "Redis SET failed");
    }
}

/// Delete a key from Redis. Best-effort.
pub async fn del(redis: &OptionalRedis, key: &str) {
    let client = match redis.as_ref() {
        Some(c) => c,
        None => return,
    };
    if let Err(e) = client.del::<i64, _>(key).await {
        tracing::warn!(%key, %e, "Redis DEL failed");
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llm_settings_key_contains_user_id() {
        let uid = Uuid::parse_str("01999d99-9999-7999-9999-999999999999").unwrap();
        let key = llm_settings_cache_key(uid);
        assert!(key.contains("llm_settings"));
        assert!(key.contains("v1"));
        assert!(key.contains("01999d99"));
    }

    #[test]
    fn chat_sessions_key_contains_user_id() {
        let uid = Uuid::parse_str("aabbccdd-1234-5678-9abc-def012345678").unwrap();
        let key = chat_sessions_cache_key(uid);
        assert!(key.contains("chat_sessions"));
        assert!(key.contains("v1"));
        assert!(key.contains("aabbccdd"));
    }

    #[test]
    fn cache_keys_differ_per_user() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        assert_ne!(llm_settings_cache_key(a), llm_settings_cache_key(b));
        assert_ne!(chat_sessions_cache_key(a), chat_sessions_cache_key(b));
    }
}
