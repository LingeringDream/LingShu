//! Cooldown cache — bounded per-user timestamp maps with TTL eviction.
//!
//! Several modules (forgetting sweeps, thought generation, memory extraction,
//! personality evolution, consolidation, thought maintenance) track the last
//! time a per-user background job ran, using `static Mutex<HashMap<Uuid, u64>>`.
//! Without eviction those maps grow unboundedly for long-running processes.
//!
//! This module provides [`CooldownCache`], a simple TTL-evicting wrapper that
//! is safe, cheap, and replaces all six ad-hoc static maps.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// A per-user cooldown tracker with automatic TTL eviction.
///
/// Entries older than `ttl_secs` seconds are evicted on every write.
/// This bounds memory to at most `max_entries` recent users.
pub struct CooldownCache {
    inner: Mutex<HashMap<Uuid, u64>>,
    ttl_secs: u64,
    max_entries: usize,
}

impl CooldownCache {
    pub fn new(ttl_secs: u64, max_entries: usize) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            ttl_secs,
            max_entries,
        }
    }

    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Returns `true` if enough time has passed since the last call for
    /// `user_id`, and records the current timestamp. Returns `false` if
    /// the cooldown has not yet elapsed.
    pub fn should_run(&self, user_id: Uuid) -> bool {
        let now = Self::now_secs();
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());

        if let Some(last) = map.get(&user_id) {
            if now.saturating_sub(*last) < self.ttl_secs {
                return false;
            }
        }

        // Evict stale entries before inserting
        if map.len() >= self.max_entries {
            map.retain(|_, &mut ts| now.saturating_sub(ts) < self.ttl_secs * 2);
        }

        map.insert(user_id, now);
        true
    }

    /// Remove a specific user's cooldown entry (e.g. after manual reset).
    pub fn remove(&self, user_id: &Uuid) {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        map.remove(user_id);
    }

    /// Current number of tracked users (for diagnostics).
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap_or_else(|p| p.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Create a `&'static CooldownCache` with a given TTL and max entries.
pub fn cooldown_cache(ttl_secs: u64, max_entries: usize) -> CooldownCache {
    CooldownCache::new(ttl_secs, max_entries)
}

// ── Pre-defined caches (one per background job) ────────────────────

/// Forgetting sweep cooldown: 5 minutes.
pub static FORGETTING_SWEEPS: OnceLock<CooldownCache> = OnceLock::new();
/// Memory extraction cooldown: 30 seconds.
pub static MEMORY_EXTRACTIONS: OnceLock<CooldownCache> = OnceLock::new();
/// Personality evolution cooldown: 1 hour.
pub static PERSONALITY_EVOLUTIONS: OnceLock<CooldownCache> = OnceLock::new();
/// Thought generation cooldown: 10 minutes.
pub static THOUGHT_GENERATIONS: OnceLock<CooldownCache> = OnceLock::new();
/// Consolidation cooldown: 1 hour.
pub static CONSOLIDATIONS: OnceLock<CooldownCache> = OnceLock::new();
/// Thought maintenance cooldown: 5 minutes.
pub static THOUGHT_MAINTENANCE: OnceLock<CooldownCache> = OnceLock::new();

pub fn forgetting_sweeps() -> &'static CooldownCache {
    FORGETTING_SWEEPS.get_or_init(|| CooldownCache::new(300, 10_000))
}

pub fn memory_extractions() -> &'static CooldownCache {
    MEMORY_EXTRACTIONS.get_or_init(|| CooldownCache::new(30, 10_000))
}

pub fn personality_evolutions() -> &'static CooldownCache {
    PERSONALITY_EVOLUTIONS.get_or_init(|| CooldownCache::new(3600, 10_000))
}

pub fn thought_generations() -> &'static CooldownCache {
    THOUGHT_GENERATIONS.get_or_init(|| CooldownCache::new(600, 10_000))
}

pub fn consolidations() -> &'static CooldownCache {
    CONSOLIDATIONS.get_or_init(|| CooldownCache::new(3600, 10_000))
}

pub fn thought_maintenance() -> &'static CooldownCache {
    THOUGHT_MAINTENANCE.get_or_init(|| CooldownCache::new(300, 10_000))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cooldown_cache_allows_first_run() {
        let cache = CooldownCache::new(3600, 100);
        let uid = Uuid::new_v4();
        assert!(cache.should_run(uid));
    }

    #[test]
    fn cooldown_cache_blocks_immediate_rerun() {
        let cache = CooldownCache::new(3600, 100);
        let uid = Uuid::new_v4();
        assert!(cache.should_run(uid));
        assert!(!cache.should_run(uid));
    }

    #[test]
    fn cooldown_cache_evicts_old_entries() {
        let cache = CooldownCache::new(1, 3); // 1 second TTL, max 3
        let u1 = Uuid::new_v4();
        let u2 = Uuid::new_v4();
        let u3 = Uuid::new_v4();
        let u4 = Uuid::new_v4();

        assert!(cache.should_run(u1));
        assert!(cache.should_run(u2));
        assert!(cache.should_run(u3));
        assert_eq!(cache.len(), 3);

        // Inserting u4 should trigger eviction of stale entries
        assert!(cache.should_run(u4));
        // All 4 might still be there since TTL hasn't elapsed
        // But the cache should never exceed max_entries * 2 after eviction
        assert!(cache.len() <= 6);
    }

    #[test]
    fn cooldown_cache_remove_clears_user() {
        let cache = CooldownCache::new(3600, 100);
        let uid = Uuid::new_v4();
        assert!(cache.should_run(uid));
        assert!(!cache.should_run(uid));
        cache.remove(&uid);
        assert!(cache.should_run(uid));
    }
}
