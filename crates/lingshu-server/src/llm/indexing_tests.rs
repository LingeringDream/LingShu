use super::*;
use serde_json::json;
use wiremock::{
    matchers::{method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
#[ignore = "Requires empty dedicated MEMORY_TEST_DATABASE_URL; applies migrations"]
async fn durable_index_retry_edit_and_delete() {
    let db = PgPool::connect(&std::env::var("MEMORY_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM memory_index_jobs")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(pending, 0, "use an empty dedicated test database");
    let user = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users(id,email,display_name,password_hash) VALUES($1,$2,'test','unused')",
    )
    .bind(user)
    .bind(format!("{user}@test.local"))
    .execute(&db)
    .await
    .unwrap();
    // A rolled-back memory must not leave a job behind.
    let mut tx = db.begin().await.unwrap();
    sqlx::query("INSERT INTO memories(user_id,memory_type,content) VALUES($1,'fact','rollback')")
        .bind(user)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM memory_index_jobs")
            .fetch_one(&db)
            .await
            .unwrap(),
        0
    );
    let id:Uuid=sqlx::query_scalar("INSERT INTO memories(user_id,memory_type,content) VALUES($1,'fact','original') RETURNING id")
        .bind(user).fetch_one(&db).await.unwrap();
    let server = MockServer::start().await;
    let llm = LlmClient::new(reqwest::Client::new(), &server.uri(), None, None);
    let vector = QdrantClient::new(&server.uri());
    Mock::given(method("POST"))
        .and(path("/api/embeddings"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    assert!(process_one(&db, &vector, &llm, "test").await.unwrap());
    let attempts: i32 =
        sqlx::query_scalar("SELECT attempts FROM memory_index_jobs WHERE memory_id=$1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(attempts, 1);
    assert!(
        !process_one(&db, &vector, &llm, "test").await.unwrap(),
        "backoff applies"
    );
    server.reset().await;
    let entered = std::sync::Arc::new(tokio::sync::Notify::new());
    let notify = entered.clone();
    Mock::given(method("POST"))
        .and(path("/api/embeddings"))
        .respond_with(move |_: &wiremock::Request| {
            notify.notify_one();
            ResponseTemplate::new(200)
                .set_body_json(json!({"embedding":[0.1,0.2,0.3]}))
                .set_delay(std::time::Duration::from_millis(300))
        })
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/collections/memories/points"))
        .and(query_param("wait", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok"})))
        .mount(&server)
        .await;
    sqlx::query("UPDATE memory_index_jobs SET available_at=NOW() WHERE memory_id=$1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    let worker_db = db.clone();
    let worker_vector = vector.clone();
    let worker_llm = llm.clone();
    let worker = tokio::spawn(async move {
        process_one(&worker_db, &worker_vector, &worker_llm, "test")
            .await
            .unwrap()
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified())
        .await
        .unwrap();
    // Edit while the old snapshot is being embedded. Completion of the old
    // generation must preserve the newer pending job.
    sqlx::query("UPDATE memories SET content='revised' WHERE id=$1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    worker.await.unwrap();
    let generation: i64 =
        sqlx::query_scalar("SELECT generation FROM memory_index_jobs WHERE memory_id=$1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(generation, 2);
    assert!(process_one(&db, &vector, &llm, "test").await.unwrap());
    assert!(!process_one(&db, &vector, &llm, "test").await.unwrap());
    let requests = server.received_requests().await.unwrap();
    assert!(requests.iter().any(|r| r.url.path() == "/api/embeddings"
        && String::from_utf8_lossy(&r.body).contains("revised")));
    // A crash leaves a lease; once expired, a new process can resume it.
    sqlx::query("UPDATE memories SET content='after restart' WHERE id=$1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("UPDATE memory_index_jobs SET lease_id=$2,locked_until=NOW()-INTERVAL '1 second' WHERE memory_id=$1")
        .bind(id).bind(Uuid::new_v4()).execute(&db).await.unwrap();
    assert!(process_one(&db, &vector, &llm, "test").await.unwrap());
    Mock::given(method("POST"))
        .and(path("/collections/memories/points/delete"))
        .and(query_param("wait", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok"})))
        .expect(1)
        .mount(&server)
        .await;
    sqlx::query("DELETE FROM memories WHERE id=$1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    assert!(process_one(&db, &vector, &llm, "test").await.unwrap());
    assert!(!process_one(&db, &vector, &llm, "test").await.unwrap());
    server.verify().await;
    db.close().await;
}
