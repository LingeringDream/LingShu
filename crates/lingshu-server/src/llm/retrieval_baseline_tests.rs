//! Small deterministic production-path regression baseline, not a quality benchmark.
use super::*;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct Fixture {
    records: Vec<Record>,
    queries: Vec<Query>,
}
#[derive(Deserialize)]
struct Record {
    key: String,
    content: String,
    scope: String,
    owner: String,
    status: String,
    age_days: i32,
}
#[derive(Deserialize)]
struct Query {
    name: String,
    query: String,
    scope: String,
    expected: Vec<String>,
}

#[tokio::test]
#[ignore = "Requires dedicated MEMORY_TEST_DATABASE_URL; applies migrations; emits JSON metrics"]
async fn production_retrieval_baseline() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("retrieval_baseline_fixture.json")).unwrap();
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
    let mut ids = HashMap::new();
    for r in fixture.records {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO memories(user_id,memory_type,content,context_scope,owner_agent_id,
            occurred_at,valid_from,valid_to,conflict_status)
            VALUES($1,'fact',$2,$3,$4,NOW()-make_interval(days=>$5),NOW()-make_interval(days=>$5),
              CASE WHEN $6='expired' THEN NOW()-INTERVAL '1 year' END,
              CASE WHEN $6='disputed' THEN 'disputed' ELSE 'active' END) RETURNING id",
        )
        .bind(user)
        .bind(r.content)
        .bind(r.scope)
        .bind(r.owner)
        .bind(r.age_days)
        .bind(r.status)
        .fetch_one(&db)
        .await
        .unwrap();
        ids.insert(r.key, id);
    }
    sqlx::query("INSERT INTO memories(user_id,memory_type,content,context_scope,occurred_at)
        SELECT $1,'fact','unrelated filler ' || n,'project',NOW()-INTERVAL '1 day' FROM generate_series(1,650) n")
        .bind(user).execute(&db).await.unwrap();
    let llm = super::super::client::LlmClient::new(
        reqwest::Client::new(),
        "http://127.0.0.1:1",
        None,
        None,
    );
    let mut report = Vec::new();
    let mut relevant = 0;
    let mut production_hits = 0;
    let mut baseline_hits = 0;
    for q in fixture.queries {
        let expected: HashSet<Uuid> = q.expected.iter().map(|k| ids[k]).collect();
        let req =
            serde_json::from_value(json!({"query":q.query,"context_scope":q.scope,"top_k":5}))
                .unwrap();
        let result = search(&db, &None, &llm, "unused", user, req).await.unwrap();
        let actual: HashSet<Uuid> = result.results.iter().map(|r| r.memory.id).collect();
        // Same current eligibility and private scope, but the former 500-row
        // recency window; lexical overlap ranking is kept explicit.
        let baseline: Vec<Uuid> = sqlx::query_scalar(
            "WITH recent AS MATERIALIZED (
            SELECT id,content,occurred_at FROM memories WHERE user_id=$1 AND context_scope=$2
            AND owner_agent_id='lingshu' AND memory_scope='private' AND deleted_at IS NULL
            AND NOT derivation_stale AND conflict_status='active' AND retention_status='active'
            AND valid_from<=NOW() AND (valid_to IS NULL OR valid_to>NOW())
            ORDER BY occurred_at DESC,id LIMIT 500)
            SELECT id FROM recent WHERE memory_search_terms(content) && memory_search_terms($3)
            ORDER BY (SELECT count(*) FROM unnest(memory_search_terms(content)) term
              WHERE term=ANY(memory_search_terms($3))) DESC,occurred_at DESC,id LIMIT 5",
        )
        .bind(user)
        .bind(&q.scope)
        .bind(&q.query)
        .fetch_all(&db)
        .await
        .unwrap();
        relevant += expected.len();
        production_hits += expected.intersection(&actual).count();
        baseline_hits += baseline.iter().filter(|id| expected.contains(id)).count();
        report.push(json!({"case":q.name,"expected":expected.len(),"production_hits":expected.intersection(&actual).count(),
            "recent500_hits":baseline.iter().filter(|id|expected.contains(id)).count(),
            "unexpected_results":actual.difference(&expected).count(),"latency_ms":result.latency_ms}));
        assert_eq!(actual, expected, "{}", q.name);
    }
    println!(
        "{}",
        json!({"suite":"production-lexical-regression-v1","embedding":"disabled","queries":report,
        "recall_at_5":production_hits as f64/relevant as f64,"recent500_recall_at_5":baseline_hits as f64/relevant as f64,
        "note":"synthetic regression fixture; not real-model accuracy or a load benchmark"})
    );
    assert!(production_hits > baseline_hits);
    db.close().await;
}
