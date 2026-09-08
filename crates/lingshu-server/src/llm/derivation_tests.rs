use sqlx::PgPool;
use uuid::Uuid;

async fn memory(db: &PgPool, user: Uuid, sources: &[Uuid]) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO memories(user_id,memory_type,content,tier,source_memory_ids)
        VALUES($1,'fact','凤凰项目使用 PostgreSQL',$2,$3) RETURNING id",
    )
    .bind(user)
    .bind(if sources.is_empty() { "raw" } else { "derived" })
    .bind(sources)
    .fetch_one(db)
    .await
    .unwrap()
}

async fn stale(db: &PgPool, id: Uuid) -> bool {
    sqlx::query_scalar("SELECT derivation_stale FROM memories WHERE id=$1")
        .bind(id)
        .fetch_one(db)
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "Requires dedicated MEMORY_TEST_DATABASE_URL; applies migrations"]
async fn derivation_invalidation_is_transitive_and_transactional() {
    let db = PgPool::connect(&std::env::var("MEMORY_TEST_DATABASE_URL").unwrap())
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
    let source = memory(&db, user, &[]).await;
    let derived = memory(&db, user, &[source]).await;
    let descendant = memory(&db, user, &[derived]).await;
    assert!(!stale(&db, derived).await);
    assert!(!stale(&db, descendant).await);
    sqlx::query("UPDATE memories SET importance=importance*0.5, access_count=access_count+1,updated_at=NOW() WHERE id=$1")
        .bind(source).execute(&db).await.unwrap();
    assert!(
        !stale(&db, derived).await,
        "access/demotion must not invalidate"
    );
    let mut tx = db.begin().await.unwrap();
    sqlx::query("UPDATE memories SET content='changed' WHERE id=$1")
        .bind(source)
        .execute(&mut *tx)
        .await
        .unwrap();
    let inside: bool = sqlx::query_scalar("SELECT derivation_stale FROM memories WHERE id=$1")
        .bind(descendant)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(inside);
    tx.rollback().await.unwrap();
    assert!(!stale(&db, descendant).await);
    sqlx::query("UPDATE memories SET content='changed' WHERE id=$1")
        .bind(source)
        .execute(&db)
        .await
        .unwrap();
    assert!(stale(&db, derived).await && stale(&db, descendant).await);
    sqlx::query("UPDATE memories SET derivation_stale=false,metadata='{}',retention_status='active' WHERE id=$1")
        .bind(derived).execute(&db).await.unwrap();
    assert!(
        stale(&db, derived).await,
        "sync cannot reactivate stale evidence"
    );

    let llm =
        super::client::LlmClient::new(reqwest::Client::new(), "http://127.0.0.1:1", None, None);
    for mode in ["adaptive", "static"] {
        let req =
            serde_json::from_value(serde_json::json!({"query":"凤凰项目", "mode":mode})).unwrap();
        let result = super::retrieval::search(&db, &None, &llm, "unused", user, req)
            .await
            .unwrap();
        assert!(!result
            .results
            .iter()
            .any(|r| [derived, descendant].contains(&r.memory.id)));
    }
    // Each source mutation independently invalidates fresh summaries.
    for mutation in [
        "UPDATE memories SET deleted_at=NOW() WHERE id=$1",
        "UPDATE memories SET conflict_status='disputed' WHERE id=$1",
        "UPDATE memories SET owner_agent_id='another' WHERE id=$1",
        "UPDATE memories SET context_scope='another' WHERE id=$1",
        "UPDATE memories SET valid_to=NOW() WHERE id=$1",
        "DELETE FROM memories WHERE id=$1",
    ] {
        let s = memory(&db, user, &[]).await;
        let d = memory(&db, user, &[s]).await;
        sqlx::query(mutation).bind(s).execute(&db).await.unwrap();
        assert!(stale(&db, d).await, "{mutation}");
    }
    let s = memory(&db, user, &[]).await;
    let d = memory(&db, user, &[s]).await;
    sqlx::query("INSERT INTO memory_access_grants(memory_id,grantee_agent_id,can_read) VALUES($1,'test',true)")
        .bind(s).execute(&db).await.unwrap();
    assert!(stale(&db, d).await);
    let before_revoke = memory(&db, user, &[s]).await;
    sqlx::query("DELETE FROM memory_access_grants WHERE memory_id=$1")
        .bind(s)
        .execute(&db)
        .await
        .unwrap();
    assert!(stale(&db, before_revoke).await);
    let missing = memory(&db, user, &[Uuid::new_v4()]).await;
    assert!(stale(&db, missing).await);
    let imported: Uuid = sqlx::query_scalar("INSERT INTO memories(user_id,memory_type,content,tier,source_memory_ids,metadata)
        VALUES($1,'fact','imported','derived',$2,'{\"derivation_review_required\":true}') RETURNING id")
        .bind(user).bind(vec![source]).fetch_one(&db).await.unwrap();
    assert!(
        stale(&db, imported).await,
        "sync marker must survive reimport"
    );
    let expiring = memory(&db, user, &[]).await;
    sqlx::query(
        "UPDATE memories SET valid_to=NOW()+INTERVAL '1 hour',source_trust=0.2 WHERE id=$1",
    )
    .bind(expiring)
    .execute(&db)
    .await
    .unwrap();
    let bounded = memory(&db, user, &[expiring]).await;
    let inherits: bool = sqlx::query_scalar(
        "SELECT d.valid_to=s.valid_to AND d.source_trust=s.source_trust
        FROM memories d,memories s WHERE d.id=$1 AND s.id=$2",
    )
    .bind(bounded)
    .bind(expiring)
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(inherits);
    // A newly built replacement can use current evidence; the old one remains pending.
    let replacement = memory(&db, user, &[source]).await;
    assert!(!stale(&db, replacement).await);
    db.close().await;
}
