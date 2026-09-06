use super::*;
use serde_json::json;

fn request(agent: &str, scope: &str, value: &str) -> CreateGatewayMemoryRequest {
    serde_json::from_value(json!({
        "agent_id":agent,"context_scope":scope,"memory_scope":"global",
        "content":format!("database {value}"),"fact_key":"database","fact_value":value
    }))
    .unwrap()
}

async fn status(db: &PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT conflict_status FROM memories WHERE id=$1")
        .bind(id)
        .fetch_one(db)
        .await
        .unwrap()
}

async fn search(state: &AppState, user: Uuid, agent: &str, scope: &str) -> GatewaySearchResponse {
    gateway_search(
        State(state.clone()),
        Some(AuthUser { user_id: user }),
        Json(
            serde_json::from_value(
                json!({"query":"database","agent_id":agent,"context_scope":scope}),
            )
            .unwrap(),
        ),
    )
    .await
    .unwrap()
    .0
}

#[tokio::test]
#[ignore = "Requires dedicated MEMORY_TEST_DATABASE_URL; applies migrations"]
async fn concurrent_memory_writes_and_reviews() {
    let mut config: crate::config::AppConfig = serde_json::from_value(json!({})).unwrap();
    config.database.url =
        std::env::var("MEMORY_TEST_DATABASE_URL").expect("dedicated test DB required");
    config.redis.url.clear();
    config.qdrant.url.clear();
    let state = AppState::new(&config).await.unwrap();
    let db = &state.db;
    let user = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,email,display_name,password_hash) VALUES($1,$2,'concurrency test','unused')")
        .bind(user).bind(format!("{user}@example.test")).execute(db).await.unwrap();

    // Hold the fact lock until both independent connections are waiting on it.
    let mut gate = db.begin().await.unwrap();
    lock_fact(&mut gate, user, "project:race", "database")
        .await
        .unwrap();
    let a_db = db.clone();
    let b_db = db.clone();
    let a = tokio::spawn(async move {
        persist_gateway_memory(
            &a_db,
            user,
            &request("claude", "project:race", "PostgreSQL"),
        )
        .await
        .unwrap()
    });
    let b = tokio::spawn(async move {
        persist_gateway_memory(&b_db, user, &request("cursor", "project:race", "MySQL"))
            .await
            .unwrap()
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let waiters: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_locks WHERE locktype='advisory' AND NOT granted AND database=(SELECT oid FROM pg_database WHERE datname=current_database())")
                .fetch_one(db).await.unwrap();
            if waiters >= 2 { break; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("both writes must overlap");
    gate.commit().await.unwrap();
    let (a, b) = (a.await.unwrap(), b.await.unwrap());
    assert_eq!(status(db, a).await, "disputed");
    assert_eq!(status(db, b).await, "disputed");
    let recalled = search(&state, user, "codex", "project:race").await;
    assert!(recalled.results.is_empty());
    assert_eq!(recalled.disputed_memories.len(), 2);

    // Third conflicting value cannot bypass an existing dispute.
    let c = persist_gateway_memory(db, user, &request("codex", "project:race", "SQLite"))
        .await
        .unwrap();
    assert_eq!(status(db, c).await, "disputed");
    let events: i64 =
        sqlx::query_scalar("SELECT count(*) FROM memory_conflict_events WHERE user_id=$1")
            .bind(user)
            .fetch_one(db)
            .await
            .unwrap();
    assert_eq!(events, 3);
    // Reconciliation is itself idempotent.
    reconcile_memory_conflicts(db, user, c, "database")
        .await
        .unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM memory_conflict_events WHERE user_id=$1")
            .bind(user)
            .fetch_one(db)
            .await
            .unwrap();
    assert_eq!(count, 3);
    let ab: (Uuid,Uuid) = sqlx::query_as("SELECT id,incumbent_memory_id FROM memory_conflict_events WHERE user_id=$1 AND (incumbent_memory_id=$2 AND challenger_memory_id=$3 OR incumbent_memory_id=$3 AND challenger_memory_id=$2)")
        .bind(user).bind(a).bind(b).fetch_one(db).await.unwrap();
    let review = |winner: &str| {
        serde_json::from_value(json!({"winner":winner,"reviewer_agent_id":"lingshu"})).unwrap()
    };
    let first = resolve_conflict_review(
        State(state.clone()),
        Some(AuthUser { user_id: user }),
        Path(ab.0),
        Json(review(if ab.1 == a { "incumbent" } else { "challenger" })),
    )
    .await
    .unwrap()
    .0;
    assert!(first.experience_memory_id.is_none());
    assert_eq!(status(db, a).await, "disputed");
    assert_eq!(status(db, b).await, "superseded");
    let ac: (Uuid,Uuid) = sqlx::query_as("SELECT id,incumbent_memory_id FROM memory_conflict_events WHERE user_id=$1 AND (incumbent_memory_id=$2 AND challenger_memory_id=$3 OR incumbent_memory_id=$3 AND challenger_memory_id=$2)")
        .bind(user).bind(a).bind(c).fetch_one(db).await.unwrap();
    let final_review = resolve_conflict_review(
        State(state.clone()),
        Some(AuthUser { user_id: user }),
        Path(ac.0),
        Json(review(if ac.1 == a { "incumbent" } else { "challenger" })),
    )
    .await
    .unwrap()
    .0;
    assert!(final_review.experience_memory_id.is_some());
    assert_eq!(status(db, a).await, "active");
    assert_eq!(status(db, c).await, "superseded");
    assert!(resolve_conflict_review(
        State(state.clone()),
        Some(AuthUser { user_id: user }),
        Path(ab.0),
        Json(review("challenger"))
    )
    .await
    .is_err());

    // Same value stays consistent; business scope, ACL and time are separate.
    let d = persist_gateway_memory(db, user, &request("cursor", "project:other", "MySQL"))
        .await
        .unwrap();
    let e = persist_gateway_memory(db, user, &request("claude", "project:other", "MySQL"))
        .await
        .unwrap();
    assert_eq!(status(db, d).await, "active");
    assert_eq!(status(db, e).await, "active");
    let mut private = request("private_agent", "project:other", "secret");
    private.memory_scope = "private".into();
    let private_id = persist_gateway_memory(db, user, &private).await.unwrap();
    assert_eq!(status(db, private_id).await, "active");
    let visible = search(&state, user, "cursor", "project:other").await;
    assert!(visible.disputed_memories.is_empty());
    assert!(visible
        .results
        .iter()
        .all(|r| r.memory.id != private_id && r.memory.context_scope == "project:other"));
    let mut past = request("claude", "project:time", "old");
    past.valid_from = Some("2020-01-01T00:00:00Z".parse().unwrap());
    past.valid_to = Some("2021-01-01T00:00:00Z".parse().unwrap());
    let old = persist_gateway_memory(db, user, &past).await.unwrap();
    let current = persist_gateway_memory(db, user, &request("cursor", "project:time", "new"))
        .await
        .unwrap();
    assert_eq!(status(db, old).await, "active");
    assert_eq!(status(db, current).await, "active");

    // Concurrent retry is one write; reuse with changed content is rejected.
    let mut retry = request("claude", "project:retry", "PostgreSQL");
    retry.request_id = Some("write-1".into());
    let (r1, r2) = tokio::join!(
        persist_gateway_memory(db, user, &retry),
        persist_gateway_memory(db, user, &retry)
    );
    assert_eq!(r1.unwrap(), r2.unwrap());
    retry.content = "changed".into();
    assert!(matches!(
        persist_gateway_memory(db, user, &retry).await,
        Err(AppError::Conflict(_))
    ));

    // Self-reported confidence, popularity and importance cannot override a claim.
    let low = request("claude", "project:evidence", "PostgreSQL");
    let low_id = persist_gateway_memory(db, user, &low).await.unwrap();
    let mut high = request("claude", "project:evidence", "MySQL");
    high.source_trust = 1.0;
    high.importance = 1.0;
    sqlx::query("UPDATE memories SET access_count=10000 WHERE id=$1")
        .bind(low_id)
        .execute(db)
        .await
        .unwrap();
    let high_id = persist_gateway_memory(db, user, &high).await.unwrap();
    assert_eq!(status(db, low_id).await, "disputed");
    assert_eq!(status(db, high_id).await, "disputed");
    let mut future = request("claude", "project:future", "future");
    future.valid_from = Some("2100-01-01T00:00:00Z".parse().unwrap());
    persist_gateway_memory(db, user, &future).await.unwrap();
    assert!(search(&state, user, "claude", "project:future")
        .await
        .results
        .is_empty());
    let other_user = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,email,display_name,password_hash) VALUES($1,$2,'other tenant','unused')")
        .bind(other_user).bind(format!("{other_user}@example.test")).execute(db).await.unwrap();
    let other_id = persist_gateway_memory(
        db,
        other_user,
        &request("claude", "project:race", "other tenant"),
    )
    .await
    .unwrap();
    assert_eq!(status(db, other_id).await, "active");
    assert_eq!(status(db, a).await, "active");

    // Directed sharing only joins facts after effective readership matches.
    let mut left = request("claude", "project:shared", "PostgreSQL");
    left.memory_scope = "shared".into();
    let mut right = request("cursor", "project:shared", "MySQL");
    right.memory_scope = "shared".into();
    let left_id = persist_gateway_memory(db, user, &left).await.unwrap();
    let right_id = persist_gateway_memory(db, user, &right).await.unwrap();
    assert_eq!(status(db, left_id).await, "active");
    for (id, owner, reader) in [
        (left_id, "claude", "cursor"),
        (right_id, "cursor", "claude"),
    ] {
        let _ = grant_memory_access(
            State(state.clone()),
            Some(AuthUser { user_id: user }),
            Path(id),
            Json(
                serde_json::from_value(json!({"owner_agent_id":owner,"grantee_agent_id":reader}))
                    .unwrap(),
            ),
        )
        .await
        .unwrap();
    }
    assert_eq!(status(db, left_id).await, "disputed");
    assert_eq!(status(db, right_id).await, "disputed");
    let outsider = search(&state, user, "outsider", "project:shared").await;
    assert!(outsider.results.is_empty() && outsider.disputed_memories.is_empty());
}
