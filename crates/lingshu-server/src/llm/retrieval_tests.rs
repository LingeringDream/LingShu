use super::*;
use serde_json::json;

#[tokio::test]
#[ignore = "Requires dedicated MEMORY_TEST_DATABASE_URL; applies migrations"]
async fn recall_old_evidence_with_acl_and_chat_parity() {
    let db = sqlx::PgPool::connect(&std::env::var("MEMORY_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    let user = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users(id,email,display_name,password_hash) VALUES($1,$2,'test','unused')",
    )
    .bind(user)
    .bind(format!("{user}@test.local"))
    .execute(&db)
    .await
    .unwrap();
    let old: Uuid = sqlx::query_scalar("INSERT INTO memories(user_id,memory_type,content,occurred_at,valid_from,context_scope)
        VALUES($1,'fact','凤凰项目数据库使用 PostgreSQL',NOW()-INTERVAL '2 years',NOW()-INTERVAL '2 years','phoenix') RETURNING id")
        .bind(user).fetch_one(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO memories(user_id,memory_type,content,context_scope)
        SELECT $1,'fact','unrelated lunch ' || n,'phoenix' FROM generate_series(1,650) n",
    )
    .bind(user)
    .execute(&db)
    .await
    .unwrap();
    // Highly relevant but unauthorized / future / expired / wrong-layer rows
    // cannot consume the lexical candidate budget ahead of visible evidence.
    sqlx::query("INSERT INTO memories(user_id,memory_type,content,context_scope,owner_agent_id,memory_scope)
        SELECT $1,'fact','凤凰项目数据库使用 PostgreSQL','phoenix','secret-agent','private' FROM generate_series(1,250)")
        .bind(user).execute(&db).await.unwrap();
    let hidden: Uuid = sqlx::query_scalar(
        "SELECT id FROM memories WHERE user_id=$1 AND owner_agent_id='secret-agent' LIMIT 1",
    )
    .bind(user)
    .fetch_one(&db)
    .await
    .unwrap();
    sqlx::query("INSERT INTO memories(user_id,memory_type,content,context_scope,valid_from)
        SELECT $1,'fact','凤凰项目数据库使用 PostgreSQL','phoenix',NOW()+INTERVAL '1 year' FROM generate_series(1,250)")
        .bind(user).execute(&db).await.unwrap();
    let req = || {
        serde_json::from_value::<GatewaySearchRequest>(json!({
            "query":"凤凰项目数据库", "context_scope":"phoenix", "agent_id":"lingshu", "top_k":5
        }))
        .unwrap()
    };
    let llm = super::super::client::LlmClient::new(
        reqwest::Client::new(),
        "http://127.0.0.1:1",
        None,
        None,
    );
    let found = search(&db, &None, &llm, "unused", user, req())
        .await
        .unwrap();
    assert_eq!(found.results.len(), 1);
    assert_eq!(found.results[0].memory.id, old);
    let candidates = candidate_rows(&db, user, &req(), &[old, hidden])
        .await
        .unwrap();
    assert!(candidates.iter().any(|m| m.id == old));
    assert!(!candidates.iter().any(|m| m.id == hidden));
    let mut semantic_only = req();
    semantic_only.query = "totally unmatched query".into();
    assert_eq!(
        candidate_rows(&db, user, &semantic_only, &[old])
            .await
            .unwrap()[0]
            .id,
        old
    );
    let chat = crate::routes::chat::fetch_memory_context(
        &db,
        &None,
        &llm,
        "unused",
        user,
        "凤凰项目数据库",
    )
    .await;
    assert!(chat.contains("凤凰项目数据库使用 PostgreSQL"));
    let count: i32 = sqlx::query_scalar("SELECT access_count FROM memories WHERE id=$1")
        .bind(old)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 2, "chat must not count retrieval twice");
    // A grant left on a now-private record must not make it visible.
    sqlx::query("INSERT INTO memory_access_grants(memory_id,grantee_agent_id,can_read) VALUES($1,'lingshu',true)")
        .bind(hidden).execute(&db).await.unwrap();
    assert!(!candidate_rows(&db, user, &req(), &[hidden])
        .await
        .unwrap()
        .iter()
        .any(|m| m.id == hidden));
    sqlx::query("UPDATE memories SET conflict_status='disputed' WHERE id=$1")
        .bind(old)
        .execute(&db)
        .await
        .unwrap();
    let disputed = search(&db, &None, &llm, "unused", user, req())
        .await
        .unwrap();
    assert!(disputed.results.is_empty());
    assert_eq!(disputed.disputed_memories[0].id, old);
    let chat = crate::routes::chat::fetch_memory_context(
        &db,
        &None,
        &llm,
        "unused",
        user,
        "凤凰项目数据库",
    )
    .await;
    assert!(chat.contains("待裁决主张"));
    let mut other_scope = req();
    other_scope.context_scope = Some("elsewhere".into());
    assert!(search(&db, &None, &llm, "unused", user, other_scope)
        .await
        .unwrap()
        .results
        .is_empty());
    // Valid-time recall is distinct from current recall and never resurrects a
    // superseded claim as a confirmed result.
    sqlx::query("UPDATE memories SET conflict_status='active', retention_status='stale', valid_to=NOW()-INTERVAL '1 year' WHERE id=$1")
        .bind(old).execute(&db).await.unwrap();
    assert!(search(&db, &None, &llm, "unused", user, req())
        .await
        .unwrap()
        .results
        .is_empty());
    let mut past = req();
    past.as_of = Some(Utc::now() - chrono::Duration::days(500));
    let recalled = search(&db, &None, &llm, "unused", user, past.clone())
        .await
        .unwrap();
    assert_eq!(recalled.results[0].memory.id, old);
    sqlx::query(
        "UPDATE memories SET conflict_status='superseded', retention_status='filtered' WHERE id=$1",
    )
    .bind(old)
    .execute(&db)
    .await
    .unwrap();
    let historical = search(&db, &None, &llm, "unused", user, past.clone())
        .await
        .unwrap();
    assert!(historical.results.is_empty());
    assert_eq!(historical.historical_memories[0].id, old);
    let audited: Vec<Uuid> =
        sqlx::query_scalar("SELECT historical_result_ids FROM memory_retrieval_runs WHERE id=$1")
            .bind(historical.run_id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(audited, vec![old]);
    sqlx::query("UPDATE memories SET owner_agent_id='secret-agent' WHERE id=$1")
        .bind(old)
        .execute(&db)
        .await
        .unwrap();
    assert!(search(&db, &None, &llm, "unused", user, past.clone())
        .await
        .unwrap()
        .historical_memories
        .is_empty());
    past.mode = "static".into();
    assert!(search(&db, &None, &llm, "unused", user, past)
        .await
        .is_err());
    db.close().await;
}
