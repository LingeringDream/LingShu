//! DB-backed integration tests for high-risk SQL queries.
//!
//! These tests connect to a real Postgres database (the local dev DB
//! specified by DATABASE_URL). Each test cleans up after itself via
//! `DELETE … WHERE user_id` at the end.
//!
//! Run with:
//!   DATABASE_URL="postgres://lingshu:lingshu@localhost:5432/lingshu" \
//!     cargo test --workspace --include-ignored
//!
//! All tests are `#[ignore]` by default to avoid slowing down unit-test
//! iteration. Use `--include-ignored` to run them.

#![allow(dead_code)]

use sqlx::PgPool;
use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════
// Regression: axum 0.7 requires ":param" syntax (not "{param}" from 0.8)
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod router_regression {
    use super::*;
    use axum::body::Body;
    use axum::{routing, Router};
    use tower::util::ServiceExt;

    /// A route with :id must match /test/<uuid> and return 200,
    /// NOT a routing 404 (which has empty body in axum 0.7).
    #[tokio::test]
    async fn colon_param_syntax_matches_path_segment() {
        async fn handler(axum::extract::Path(id): axum::extract::Path<Uuid>) -> String {
            format!("id={id}")
        }

        let app = Router::<()>::new().route("/api/v1/test/:id", routing::get(handler));

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/test/a1b2c3d4-e5f6-7890-abcd-ef1234567890")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            resp.status(),
            200,
            "route with :id must match (got {})",
            resp.status()
        );
    }

    /// Multiple :param segments must all be captured.
    #[tokio::test]
    async fn multiple_colon_params_all_captured() {
        async fn handler(axum::extract::Path((a, b)): axum::extract::Path<(Uuid, Uuid)>) -> String {
            format!("a={a},b={b}")
        }

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let app =
            Router::<()>::new().route("/api/v1/projects/:pid/tasks/:tid", routing::get(handler));

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/api/v1/projects/{id1}/tasks/{id2}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200, "multi-param route must match");
    }

    /// A route with a literal segment after :param must match correctly
    /// (regression: :id/confirm was previously {id}/confirm → 404).
    #[tokio::test]
    async fn param_followed_by_literal_segment_matches() {
        async fn handler(axum::extract::Path(id): axum::extract::Path<Uuid>) -> String {
            format!("confirm:{id}")
        }

        let app = Router::<()>::new().route(
            "/api/v1/calendar/events/:id/confirm",
            routing::post(handler),
        );

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/v1/calendar/events/{}/confirm",
                        Uuid::new_v4()
                    ))
                    .header("content-type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200, ":id/confirm sub-route must match POST");
    }
}

/// Create a pool to the dev database (reads DATABASE_URL).
async fn dev_pool() -> PgPool {
    let db_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
    PgPool::connect(&db_url).await.expect("connect to dev DB")
}

/// Create a test user and return its id. All test data is scoped to this user
/// so cleanup is `DELETE FROM <table> WHERE user_id = $1`.
async fn create_user(pool: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    let email = format!("test-int-{id}@example.com");
    sqlx::query(
        "INSERT INTO users (id, email, display_name, password_hash) \
         VALUES ($1, $2, 'test-int', 'not-a-real-hash')",
    )
    .bind(id)
    .bind(&email)
    .execute(pool)
    .await
    .unwrap();
    id
}

/// Cleanup all test data for a user across all tables used in these tests.
async fn cleanup_user(pool: &PgPool, user_id: Uuid) {
    let tables = [
        "signal_events",
        "thought_queue",
        "personality_snapshots",
        "integrations",
        "memories",
        "calendar_events",
        "messages",
        "task_dependencies",
        "project_members",
        "tasks",
        "projects",
        "conversations",
    ];
    for table in tables {
        let _ = sqlx::query(&format!("DELETE FROM {table} WHERE user_id = $1"))
            .bind(user_id)
            .execute(pool)
            .await;
    }
    // conversations doesn't have user_id directly — skip for now
    // Delete the user last (FKs are ON DELETE CASCADE)
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await;
}

// ═══════════════════════════════════════════════════════════════════════
// Test 1: thoughts list_thoughts — dynamic format! SQL with 3 branches
// ═══════════════════════════════════════════════════════════════════════

mod thoughts {
    use super::*;

    async fn insert_thought(
        pool: &PgPool,
        user_id: Uuid,
        title: &str,
        status: &str,
        scheduled_at: Option<&str>,
    ) -> Uuid {
        let id = Uuid::new_v4();
        let scheduled_at_val: Option<chrono::DateTime<chrono::Utc>> =
            scheduled_at.map(|s| s.parse().unwrap());
        sqlx::query(
            "INSERT INTO thought_queue (id, user_id, title, status, scheduled_at) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(id)
        .bind(user_id)
        .bind(title)
        .bind(status)
        .bind(scheduled_at_val)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    /// Branch 1: filter by status — placeholder count = 3 ($1=user_id, $2=status, $3=limit).
    #[ignore]
    #[tokio::test]
    async fn list_thoughts_by_status() {
        let pool = dev_pool().await;
        let user_id = create_user(&pool).await;
        insert_thought(&pool, user_id, "t1", "pending", None).await;
        insert_thought(&pool, user_id, "t2", "pending", None).await;
        insert_thought(&pool, user_id, "t3", "shown", None).await;

        // The SQL under test (from routes/thoughts.rs:108-116):
        let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
            "SELECT id, title, status FROM thought_queue \
             WHERE user_id = $1 AND status = $2 \
             ORDER BY scheduled_at ASC NULLS FIRST, created_at DESC LIMIT $3",
        )
        .bind(user_id)
        .bind("pending")
        .bind(50i64)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows.len(), 2, "should return 2 pending thoughts");
        for (_, _, status) in &rows {
            assert_eq!(status, "pending");
        }

        cleanup_user(&pool, user_id).await;
    }

    /// Branch 2: active=true filter — placeholder count = 2 ($1=user_id, $2=limit).
    #[ignore]
    #[tokio::test]
    async fn list_thoughts_active_filter() {
        let pool = dev_pool().await;
        let user_id = create_user(&pool).await;
        insert_thought(
            &pool,
            user_id,
            "active1",
            "snoozed",
            Some("2099-01-01T00:00:00Z"),
        )
        .await;
        insert_thought(&pool, user_id, "active2", "pending", None).await;

        // The SQL under test (from routes/thoughts.rs:117-125):
        let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
            "SELECT id, title, status FROM thought_queue \
             WHERE user_id = $1 \
             AND (scheduled_at IS NULL OR scheduled_at <= NOW()) \
             ORDER BY scheduled_at ASC NULLS FIRST, created_at DESC LIMIT $2",
        )
        .bind(user_id)
        .bind(50i64)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(
            rows.len(),
            1,
            "only the non-future-scheduled thought should be active"
        );
        assert_eq!(rows[0].1, "active2");

        cleanup_user(&pool, user_id).await;
    }

    /// Branch 3: default (no status, no active) — placeholder count = 2 ($1=user_id, $2=limit).
    #[ignore]
    #[tokio::test]
    async fn list_thoughts_default_branch() {
        let pool = dev_pool().await;
        let user_id = create_user(&pool).await;
        insert_thought(&pool, user_id, "a", "pending", None).await;
        insert_thought(&pool, user_id, "b", "shown", None).await;

        // The SQL under test (from routes/thoughts.rs:127-133):
        let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
            "SELECT id, title, status FROM thought_queue \
             WHERE user_id = $1 \
             ORDER BY scheduled_at ASC NULLS FIRST, created_at DESC LIMIT $2",
        )
        .bind(user_id)
        .bind(50i64)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows.len(), 2, "default branch should return all thoughts");

        cleanup_user(&pool, user_id).await;
    }

    /// Regression: verify all three branches use consistent parameter indices.
    #[ignore]
    #[tokio::test]
    async fn all_three_branches_placeholder_alignment() {
        let pool = dev_pool().await;
        let user_id = create_user(&pool).await;

        // Branch 1 (status): $1, $2, $3 → 3 binds
        sqlx::query(
            "SELECT * FROM thought_queue WHERE user_id = $1 AND status = $2 \
             ORDER BY scheduled_at ASC NULLS FIRST, created_at DESC LIMIT $3",
        )
        .bind(user_id)
        .bind("pending")
        .bind(50i64)
        .fetch_all(&pool)
        .await
        .expect("status branch: 3 placeholders with 3 binds");

        // Branch 2 (active): $1, $2 → 2 binds
        sqlx::query(
            "SELECT * FROM thought_queue WHERE user_id = $1 \
             AND (scheduled_at IS NULL OR scheduled_at <= NOW()) \
             ORDER BY scheduled_at ASC NULLS FIRST, created_at DESC LIMIT $2",
        )
        .bind(user_id)
        .bind(50i64)
        .fetch_all(&pool)
        .await
        .expect("active branch: 2 placeholders with 2 binds");

        // Branch 3 (default): $1, $2 → 2 binds
        sqlx::query(
            "SELECT * FROM thought_queue WHERE user_id = $1 \
             ORDER BY scheduled_at ASC NULLS FIRST, created_at DESC LIMIT $2",
        )
        .bind(user_id)
        .bind(50i64)
        .fetch_all(&pool)
        .await
        .expect("default branch: 2 placeholders with 2 binds");

        cleanup_user(&pool, user_id).await;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Test 2: consolidation — write derived + soft-demote source in transaction
// ═══════════════════════════════════════════════════════════════════════

mod consolidation {
    use super::*;

    async fn insert_memory_raw(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        content: &str,
        importance: f32,
    ) {
        sqlx::query(
            "INSERT INTO memories (id, user_id, memory_type, content, importance) \
             VALUES ($1, $2, 'fact', $3, $4)",
        )
        .bind(id)
        .bind(user_id)
        .bind(content)
        .bind(importance)
        .execute(pool)
        .await
        .unwrap();
    }

    /// Verify the consolidation transaction: insert derived with
    /// source_memory_ids + tier='derived', then soft-demote sources.
    #[ignore]
    #[tokio::test]
    async fn apply_merge_group_produces_derived_and_demotes_sources() {
        let pool = dev_pool().await;
        let user_id = create_user(&pool).await;

        let s1 = Uuid::new_v4();
        let s2 = Uuid::new_v4();
        let s3 = Uuid::new_v4(); // unrelated
        insert_memory_raw(&pool, s1, user_id, "Rust is great for systems", 0.8).await;
        insert_memory_raw(&pool, s2, user_id, "I use Rust at work daily", 0.7).await;
        insert_memory_raw(&pool, s3, user_id, "unrelated memory", 0.6).await;

        let source_ids: Vec<Uuid> = vec![s1, s2];

        // The transaction under test (from llm/consolidation.rs:292-318):
        let mut tx = pool.begin().await.unwrap();

        // 1. Insert derived memory
        let derived_id = Uuid::new_v4();
        let derived: (Uuid, String, f32, Vec<Uuid>, String) = sqlx::query_as(
            "INSERT INTO memories \
             (id, user_id, memory_type, content, importance, source_memory_ids, tier) \
             VALUES ($1, $2, 'fact', $3, $4, $5, 'derived') \
             RETURNING id, content, importance, source_memory_ids, tier",
        )
        .bind(derived_id)
        .bind(user_id)
        .bind("User frequently uses Rust for systems programming")
        .bind(0.75f32)
        .bind(&source_ids)
        .fetch_one(&mut *tx)
        .await
        .unwrap();

        assert_eq!(derived.3, source_ids, "source_memory_ids should match");
        assert_eq!(derived.4, "derived", "tier should be 'derived'");
        assert!((derived.2 - 0.75).abs() < f32::EPSILON);

        // 2. Soft-demote source memories (importance *= 0.5)
        sqlx::query(
            "UPDATE memories SET importance = importance * $1, updated_at = NOW() \
             WHERE id = ANY($2) AND user_id = $3 AND deleted_at IS NULL",
        )
        .bind(0.5f32)
        .bind(&source_ids)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .unwrap();

        tx.commit().await.unwrap();

        // Verify demotion
        for (sid, expected) in [(s1, 0.4f32), (s2, 0.35f32)] {
            let imp: (f32,) =
                sqlx::query_as("SELECT importance FROM memories WHERE id = $1 AND user_id = $2")
                    .bind(sid)
                    .bind(user_id)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert!(
                (imp.0 - expected).abs() < 0.001,
                "source {sid}: expected ~{expected}, got {}",
                imp.0
            );
        }

        // Unrelated memory untouched
        let imp3: (f32,) =
            sqlx::query_as("SELECT importance FROM memories WHERE id = $1 AND user_id = $2")
                .bind(s3)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!((imp3.0 - 0.6).abs() < 0.001, "unrelated memory unchanged");

        cleanup_user(&pool, user_id).await;
    }

    /// Verify transaction rollback on error.
    #[ignore]
    #[tokio::test]
    async fn merge_group_transaction_rollback_on_error() {
        let pool = dev_pool().await;
        let user_id = create_user(&pool).await;
        let s1 = Uuid::new_v4();
        insert_memory_raw(&pool, s1, user_id, "source memory", 0.8).await;

        let pre_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM memories WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();

        // Start transaction, insert derived, then rollback
        let mut tx = pool.begin().await.unwrap();
        sqlx::query(
            "INSERT INTO memories \
             (id, user_id, memory_type, content, importance, source_memory_ids, tier) \
             VALUES ($1, $2, 'fact', 'derived content', 0.7, $3, 'derived')",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(vec![s1])
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.rollback().await.unwrap();

        let post_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM memories WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(pre_count.0, post_count.0, "rollback restores state");

        cleanup_user(&pool, user_id).await;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Test 3: forgetting sweep — protected ID set (personality + derived sources)
// ═══════════════════════════════════════════════════════════════════════

mod forgetting {
    use super::*;

    async fn insert_memory_with_id(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        content: &str,
        importance: f32,
        tier: &str,
        source_ids: &[Uuid],
    ) {
        sqlx::query(
            "INSERT INTO memories (id, user_id, memory_type, content, importance, tier, source_memory_ids) \
             VALUES ($1, $2, 'fact', $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(user_id)
        .bind(content)
        .bind(importance)
        .bind(tier)
        .bind(source_ids)
        .execute(pool)
        .await
        .unwrap();
    }

    /// Protection from active personality snapshots.
    #[ignore]
    #[tokio::test]
    async fn protected_ids_from_active_personality_snapshot() {
        let pool = dev_pool().await;
        let user_id = create_user(&pool).await;
        let m1 = Uuid::new_v4();
        let m2 = Uuid::new_v4();
        let m3 = Uuid::new_v4();

        insert_memory_with_id(&pool, m1, user_id, "mem 1", 0.8, "raw", &[]).await;
        insert_memory_with_id(&pool, m2, user_id, "mem 2", 0.7, "raw", &[]).await;
        insert_memory_with_id(&pool, m3, user_id, "mem 3", 0.6, "raw", &[]).await;

        // Active personality snapshot referencing m1, m2
        sqlx::query(
            "INSERT INTO personality_snapshots \
             (id, user_id, trait_values, change_reason, source_memory_ids, is_active) \
             VALUES ($1, $2, '{}', 'test', $3, true)",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(vec![m1, m2])
        .execute(&pool)
        .await
        .unwrap();

        // Protection query (from chat.rs:456-459):
        let protected: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT unnest(source_memory_ids) \
             FROM personality_snapshots \
             WHERE user_id = $1 AND is_active = true",
        )
        .bind(user_id)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(protected.len(), 2, "should protect m1 and m2");
        assert!(protected.contains(&m1));
        assert!(protected.contains(&m2));
        assert!(!protected.contains(&m3));

        cleanup_user(&pool, user_id).await;
    }

    /// Protection from derived memory source chains.
    #[ignore]
    #[tokio::test]
    async fn protected_ids_from_derived_memory_sources() {
        let pool = dev_pool().await;
        let user_id = create_user(&pool).await;
        let raw1 = Uuid::new_v4();
        let raw2 = Uuid::new_v4();

        insert_memory_with_id(&pool, raw1, user_id, "raw 1", 0.8, "raw", &[]).await;
        insert_memory_with_id(&pool, raw2, user_id, "raw 2", 0.7, "raw", &[]).await;

        // Derived memory referencing raw1
        insert_memory_with_id(
            &pool,
            Uuid::new_v4(),
            user_id,
            "derived from raw1",
            0.7,
            "derived",
            &[raw1],
        )
        .await;

        // Protection query (from chat.rs:474-476):
        let protected: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT unnest(source_memory_ids) FROM memories \
             WHERE user_id = $1 AND tier = 'derived' AND deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(protected.len(), 1, "should protect raw1 as derived source");
        assert!(protected.contains(&raw1));
        assert!(!protected.contains(&raw2));

        cleanup_user(&pool, user_id).await;
    }

    /// Combined protection: both personality and derived sources.
    #[ignore]
    #[tokio::test]
    async fn protected_id_set_covers_both_sources() {
        let pool = dev_pool().await;
        let user_id = create_user(&pool).await;
        let m_pers = Uuid::new_v4();
        let m_der_src = Uuid::new_v4();
        let m_unprot = Uuid::new_v4();

        insert_memory_with_id(&pool, m_pers, user_id, "personality ref", 0.8, "raw", &[]).await;
        insert_memory_with_id(&pool, m_der_src, user_id, "derived source", 0.7, "raw", &[]).await;
        insert_memory_with_id(&pool, m_unprot, user_id, "unprotected", 0.5, "raw", &[]).await;

        // Personality snapshot → m_pers
        sqlx::query(
            "INSERT INTO personality_snapshots \
             (id, user_id, trait_values, change_reason, source_memory_ids, is_active) \
             VALUES ($1, $2, '{}', 'test', $3, true)",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(vec![m_pers])
        .execute(&pool)
        .await
        .unwrap();

        // Derived memory → m_der_src
        insert_memory_with_id(
            &pool,
            Uuid::new_v4(),
            user_id,
            "derived content",
            0.7,
            "derived",
            &[m_der_src],
        )
        .await;

        // Combined protection set (as in chat.rs run_forgetting_sweep):
        let mut protected = std::collections::HashSet::new();

        let ids1: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT unnest(source_memory_ids) \
             FROM personality_snapshots \
             WHERE user_id = $1 AND is_active = true",
        )
        .bind(user_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        for id in ids1 {
            protected.insert(id);
        }

        let ids2: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT unnest(source_memory_ids) FROM memories \
             WHERE user_id = $1 AND tier = 'derived' AND deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        for id in ids2 {
            protected.insert(id);
        }

        assert!(
            protected.contains(&m_pers),
            "personality-referenced should be protected"
        );
        assert!(
            protected.contains(&m_der_src),
            "derived source should be protected"
        );
        assert!(
            !protected.contains(&m_unprot),
            "unreferenced should NOT be protected"
        );
        assert_eq!(protected.len(), 2, "exactly 2 protected IDs");

        cleanup_user(&pool, user_id).await;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Test 4: integrations — NULL project partial unique index (23505 path)
// ═══════════════════════════════════════════════════════════════════════

mod integrations {
    use super::*;

    /// Migration 0016: partial unique index on (user_id, platform) WHERE project_id IS NULL.
    #[ignore]
    #[tokio::test]
    async fn null_project_unique_constraint_enforced() {
        let pool = dev_pool().await;
        let user_id = create_user(&pool).await;

        // 1. First NULL-project integration: OK
        sqlx::query(
            "INSERT INTO integrations \
             (id, user_id, platform, access_token_encrypted, config) \
             VALUES ($1, $2, 'apple_calendar', $3, '{}')",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(b"dummy-encrypted-token-001".as_slice())
        .execute(&pool)
        .await
        .expect("first NULL-project integration should succeed");

        // 2. Duplicate NULL-project integration: MUST fail with 23505
        let result = sqlx::query(
            "INSERT INTO integrations \
             (id, user_id, platform, access_token_encrypted, config) \
             VALUES ($1, $2, 'apple_calendar', $3, '{}')",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(b"dummy-encrypted-token-002".as_slice())
        .execute(&pool)
        .await;

        match result {
            Err(sqlx::Error::Database(db_err)) => {
                assert_eq!(
                    db_err.code().as_deref(),
                    Some("23505"),
                    "duplicate NULL-project integration must be 23505 unique_violation"
                );
            }
            Ok(_) => {
                panic!("second NULL-project integration should have been rejected by unique index")
            }
            Err(e) => panic!("unexpected error type: {e}"),
        }

        // 3. Same platform with non-NULL project_id: OK (not blocked by partial index)
        let project_id = Uuid::new_v4();
        sqlx::query("INSERT INTO projects (id, name, owner_id) VALUES ($1, 'test proj', $2)")
            .bind(project_id)
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO integrations \
             (id, user_id, project_id, platform, access_token_encrypted, config) \
             VALUES ($1, $2, $3, 'apple_calendar', $4, '{}')",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(project_id)
        .bind(b"dummy-token-003".as_slice())
        .execute(&pool)
        .await
        .expect("non-NULL project_id should not conflict with NULL-project ones");

        cleanup_user(&pool, user_id).await;
    }

    /// Check-before-insert query uses IS NOT DISTINCT FROM for project_id.
    #[ignore]
    #[tokio::test]
    async fn duplicate_check_using_is_not_distinct_from() {
        let pool = dev_pool().await;
        let user_id = create_user(&pool).await;

        // Insert one integration with NULL project
        sqlx::query(
            "INSERT INTO integrations \
             (id, user_id, platform, access_token_encrypted, config) \
             VALUES ($1, $2, 'slack', $3, '{}')",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(b"token-slack".as_slice())
        .execute(&pool)
        .await
        .unwrap();

        // Check-before-insert query (from routes/integrations.rs:151-162):
        let duplicate_exists: (bool,) = sqlx::query_as(
            "SELECT EXISTS(\
                SELECT 1 FROM integrations \
                WHERE user_id = $1 \
                  AND project_id IS NOT DISTINCT FROM $2 \
                  AND platform = $3\
            )",
        )
        .bind(user_id)
        .bind(None::<Uuid>)
        .bind("slack")
        .fetch_one(&pool)
        .await
        .unwrap();

        assert!(
            duplicate_exists.0,
            "IS NOT DISTINCT FROM should detect the duplicate"
        );

        // With a different project_id, NOT a duplicate
        let no_duplicate: (bool,) = sqlx::query_as(
            "SELECT EXISTS(\
                SELECT 1 FROM integrations \
                WHERE user_id = $1 \
                  AND project_id IS NOT DISTINCT FROM $2 \
                  AND platform = $3\
            )",
        )
        .bind(user_id)
        .bind(Some(Uuid::new_v4()))
        .bind("slack")
        .fetch_one(&pool)
        .await
        .unwrap();

        assert!(
            !no_duplicate.0,
            "different project_id should not be a duplicate"
        );

        cleanup_user(&pool, user_id).await;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Test 5: memories — full-column FromRow ↔ SELECT/RETURNING consistency
// ═══════════════════════════════════════════════════════════════════════

mod memories {
    use super::*;

    /// SELECT * must return columns matching the Memory struct, including
    /// tier and source_memory_ids from migration 0018.
    #[ignore]
    #[allow(clippy::type_complexity)]
    #[tokio::test]
    async fn memory_select_star_matches_insert_returning_star() {
        let pool = dev_pool().await;
        let user_id = create_user(&pool).await;
        let memory_id = Uuid::new_v4();
        let source_ids: Vec<Uuid> = vec![Uuid::new_v4(), Uuid::new_v4()];

        // INSERT with explicit tier + source_memory_ids, RETURNING *
        // Column order: id(1), user_id(2), project_id(3), memory_type(4), content(5),
        // importance(6), access_count(7), last_accessed_at(8), vector_id(9),
        // metadata(10), deleted_at(11), created_at(12), updated_at(13),
        // source_memory_ids(14), tier(15)
        let inserted: (
            Uuid,
            Uuid,
            Option<Uuid>,
            String,
            String,
            f32,
            i32,
            Option<chrono::DateTime<chrono::Utc>>,
            Option<String>,
            serde_json::Value,
            Option<chrono::DateTime<chrono::Utc>>,
            chrono::DateTime<chrono::Utc>,
            chrono::DateTime<chrono::Utc>,
            Vec<Uuid>,
            String,
        ) = sqlx::query_as(
            "INSERT INTO memories \
             (id, user_id, memory_type, content, importance, source_memory_ids, tier) \
             VALUES ($1, $2, 'preference', 'test content', 0.75, $3, 'derived') \
             RETURNING id, user_id, project_id, memory_type, content, importance, \
             access_count, last_accessed_at, vector_id, \
             metadata, deleted_at, created_at, updated_at, \
             source_memory_ids, tier",
        )
        .bind(memory_id)
        .bind(user_id)
        .bind(&source_ids)
        .fetch_one(&pool)
        .await
        .unwrap();

        // Verify RETURNING * columns (position-based)
        assert_eq!(inserted.0, memory_id); // 1: id
        assert_eq!(inserted.1, user_id); // 2: user_id
        assert_eq!(inserted.2, None); // 3: project_id
        assert_eq!(inserted.3, "preference"); // 4: memory_type
        assert_eq!(inserted.4, "test content"); // 5: content
        assert!((inserted.5 - 0.75).abs() < f32::EPSILON); // 6: importance
        assert_eq!(inserted.6, 0); // 7: access_count
        assert_eq!(inserted.7, None); // 8: last_accessed_at
        assert_eq!(inserted.8, None); // 9: vector_id
        assert_eq!(inserted.9, serde_json::json!({})); // 10: metadata
        assert_eq!(inserted.10, None); // 11: deleted_at
        assert_eq!(inserted.13, source_ids); // 14: source_memory_ids
        assert_eq!(inserted.14, "derived"); // 15: tier

        // SELECT * must match RETURNING * (same column order)
        let selected = sqlx::query_as::<
            _,
            (
                Uuid,
                Uuid,
                Option<Uuid>,
                String,
                String,
                f32,
                i32,
                Option<chrono::DateTime<chrono::Utc>>,
                Option<String>,
                serde_json::Value,
                Option<chrono::DateTime<chrono::Utc>>,
                chrono::DateTime<chrono::Utc>,
                chrono::DateTime<chrono::Utc>,
                Vec<Uuid>,
                String,
            ),
        >("SELECT * FROM memories WHERE id = $1 AND user_id = $2")
        .bind(memory_id)
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        // Key columns match between INSERT...RETURNING * and SELECT *
        assert_eq!(selected.0, inserted.0); // id
        assert_eq!(selected.4, inserted.4); // content
        assert_eq!(selected.13, inserted.13); // source_memory_ids (col 14)
        assert_eq!(selected.14, inserted.14); // tier (col 15)

        cleanup_user(&pool, user_id).await;
    }

    /// Defaults: tier → 'raw', source_memory_ids → [].
    #[ignore]
    #[tokio::test]
    async fn memory_defaults_tier_raw_and_empty_source_ids() {
        let pool = dev_pool().await;
        let user_id = create_user(&pool).await;

        let (tier, source_ids): (String, Vec<Uuid>) = sqlx::query_as(
            "INSERT INTO memories (id, user_id, memory_type, content, importance) \
             VALUES ($1, $2, 'fact', 'default test', 0.5) \
             RETURNING tier, source_memory_ids",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(tier, "raw", "tier should default to 'raw'");
        assert!(
            source_ids.is_empty(),
            "source_memory_ids should default to empty array"
        );

        cleanup_user(&pool, user_id).await;
    }

    /// Both 'raw' and 'derived' tier values are valid.
    #[ignore]
    #[tokio::test]
    async fn memory_tier_values_raw_and_derived() {
        let pool = dev_pool().await;
        let user_id = create_user(&pool).await;

        let tier_raw: (String,) = sqlx::query_as(
            "INSERT INTO memories (id, user_id, memory_type, content, importance, tier) \
             VALUES ($1, $2, 'fact', 'raw memory', 0.6, 'raw') \
             RETURNING tier",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(tier_raw.0, "raw");

        let tier_derived: (String,) = sqlx::query_as(
            "INSERT INTO memories (id, user_id, memory_type, content, importance, tier) \
             VALUES ($1, $2, 'fact', 'derived memory', 0.6, 'derived') \
             RETURNING tier",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(tier_derived.0, "derived");

        cleanup_user(&pool, user_id).await;
    }

    /// access_count bump + last_accessed_at update.
    #[ignore]
    #[tokio::test]
    async fn memory_access_count_bump() {
        let pool = dev_pool().await;
        let user_id = create_user(&pool).await;
        let id = Uuid::new_v4();

        sqlx::query(
            "INSERT INTO memories (id, user_id, memory_type, content, importance) \
             VALUES ($1, $2, 'fact', 'test', 0.5)",
        )
        .bind(id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

        // Bump (from chat.rs memory retrieval path)
        sqlx::query(
            "UPDATE memories SET access_count = access_count + 1, \
             last_accessed_at = NOW() WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

        let (count, last): (i32, Option<chrono::DateTime<chrono::Utc>>) =
            sqlx::query_as("SELECT access_count, last_accessed_at FROM memories WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(count, 1, "access_count should be bumped from 0 to 1");
        assert!(last.is_some(), "last_accessed_at should be set after bump");

        cleanup_user(&pool, user_id).await;
    }
}
