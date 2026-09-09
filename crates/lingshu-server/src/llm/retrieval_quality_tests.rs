//! Paired synthetic regression, not an independent real-world quality benchmark.
use super::super::evolutionary_memory::lexical_similarity;
use super::*;
use serde_json::json;

fn ranked(rows: &[GatewayMemoryRow], query: &str, old: bool) -> Vec<Uuid> {
    let weights = RetrievalWeights::for_query(query).normalized();
    let now = Utc::now();
    let mut scores: Vec<_> = rows
        .iter()
        .map(|r| {
            let memory = RankableMemory {
                id: r.id,
                content: r.content.clone(),
                importance: r.importance,
                access_count: r.access_count,
                occurred_at: r.occurred_at,
                valid_to: r.valid_to,
                source_trust: r.source_trust,
                conflict_status: r.conflict_status.clone(),
                retention_status: r.retention_status.clone(),
                semantic_score: 0.0,
            };
            let s = score_memory(query, &memory, &weights, now);
            let total = if old {
                s.total + weights.lexical * (lexical_similarity(query, &r.content) - s.lexical)
            } else {
                s.total
            };
            (r.id, total, r.occurred_at)
        })
        .collect();
    scores.sort_by(|a, b| {
        b.1.total_cmp(&a.1)
            .then_with(|| b.2.cmp(&a.2))
            .then_with(|| a.0.cmp(&b.0))
    });
    scores.into_iter().map(|s| s.0).collect()
}

#[tokio::test]
#[ignore = "Requires disposable MEMORY_TEST_DATABASE_URL; paired quality/latency report"]
async fn paired_retrieval_quality() {
    let db = sqlx::PgPool::connect(&std::env::var("MEMORY_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    let user = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users(id,email,display_name,password_hash) VALUES($1,$2,'quality','unused')",
    )
    .bind(user)
    .bind(format!("{user}@test.local"))
    .execute(&db)
    .await
    .unwrap();
    let long = format!(
        "凤凰项目数据库使用 PostgreSQL。{}",
        "以下是部署过程的详细背景说明，不涉及其他技术决策。".repeat(120)
    );
    let cases = [
        (
            "number",
            "无关记录 42424",
            "无关记录 42424 abcdefghijklmnopqrstuvwxyz",
            "无关记录 424240",
        ),
        (
            "ticket",
            "工单 AB-42",
            "工单 AB-42 已处理，附带客户沟通的详细说明",
            "工单 AB-420",
        ),
        (
            "version",
            "版本 2.0",
            "版本 2.0 使用新接口，详细发布说明已归档",
            "版本 2.0.1",
        ),
        (
            "entity",
            "PostgreSQL",
            "项目已选用 PostgreSQL，附录记录了整个迁移计划。",
            "PostgreSQLish",
        ),
        (
            "entity-boundary",
            "Rust",
            "编译服务以 Rust 实现。详细实现资料另存。",
            "Rustacean",
        ),
        (
            "phrase",
            "蓝鲸发布计划",
            "蓝鲸发布计划：周五发布，完整操作手册见团队共享文档。",
            "蓝鲸发布后的计划",
        ),
        (
            "paraphrase",
            "凤凰项目用什么数据库",
            "凤凰项目后端存储采用 PostgreSQL 数据库",
            "周末去看电影",
        ),
        ("short", "后端", "凤凰项目后端采用 Rust", "前台页面布局"),
        (
            "long-body",
            "凤凰项目数据库",
            long.as_str(),
            "凤凰项目的数据仓库",
        ),
        (
            "english-phrase",
            "river project",
            "The river project uses Rust. More implementation details are archived.",
            "rivers project",
        ),
    ];
    let mut targets = Vec::new();
    for (name, _, content, distractor) in &cases {
        let id: Uuid = sqlx::query_scalar("INSERT INTO memories(user_id,memory_type,content,context_scope) VALUES($1,'fact',$2,$3) RETURNING id")
            .bind(user).bind(content).bind(name).fetch_one(&db).await.unwrap();
        targets.push(id);
        // Frequently accessed near matches must not automatically beat exact evidence.
        sqlx::query(
            "INSERT INTO memories(user_id,memory_type,content,context_scope,access_count)
            SELECT $1,'fact',$2 || ' ' || n,$3,100 FROM generate_series(1,250) n",
        )
        .bind(user)
        .bind(distractor)
        .bind(name)
        .execute(&db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO memories(user_id,memory_type,content,context_scope)
            SELECT $1,'fact','unrelated background ' || n,$2 FROM generate_series(1,350) n",
        )
        .bind(user)
        .bind(name)
        .execute(&db)
        .await
        .unwrap();
    }
    sqlx::query("ANALYZE memories").execute(&db).await.unwrap();
    let mut report = Vec::new();
    let mut totals = [[0usize; 3]; 3];
    let llm = super::super::client::LlmClient::new(
        reqwest::Client::new(),
        "http://127.0.0.1:1",
        None,
        None,
    );
    for ((name, query, _, _), target) in cases.iter().zip(targets) {
        let req = || {
            serde_json::from_value(json!({"query":query,"context_scope":name,"top_k":20})).unwrap()
        };
        let started = Instant::now();
        let old: Vec<GatewayMemoryRow> = sqlx::query_as(&format!("{MEMORY_SELECT}
            AND m.context_scope=$3 AND m.owner_agent_id=$2 AND m.memory_scope='private'
            AND NOT m.derivation_stale AND m.conflict_status='active' AND m.retention_status='active'
            AND m.valid_from<=NOW() AND (m.valid_to IS NULL OR m.valid_to>NOW())
            AND memory_search_terms(m.content) && memory_search_terms($4)
            ORDER BY (SELECT count(*) FROM unnest(memory_search_terms(m.content)) term
              WHERE term=ANY(memory_search_terms($4))) DESC,m.occurred_at DESC,m.id LIMIT 200"))
            .bind(user).bind("lingshu").bind(name).bind(query).fetch_all(&db).await.unwrap();
        let old_ms = started.elapsed().as_secs_f64() * 1000.0;
        let started = Instant::now();
        let new = candidate_rows(&db, user, &req(), &[]).await.unwrap();
        let new_ms = started.elapsed().as_secs_f64() * 1000.0;
        let arms = [
            ranked(&old, query, true),
            ranked(&new, query, true),
            ranked(&new, query, false),
        ];
        let ranks: Vec<_> = arms
            .iter()
            .map(|ids| ids.iter().position(|id| *id == target).map(|i| i + 1))
            .collect();
        for (arm, rank) in ranks.iter().enumerate() {
            for (metric, k) in [1, 5, 20].iter().enumerate() {
                totals[arm][metric] += usize::from(rank.is_some_and(|r| r <= *k));
            }
        }
        let actual = search(&db, &None, &llm, "unused", user, req())
            .await
            .unwrap();
        assert_eq!(
            actual
                .results
                .iter()
                .position(|r| r.memory.id == target)
                .map(|i| i + 1),
            ranks[2].filter(|r| *r <= 20)
        );
        report.push(json!({"case":name,"ranks_old_new_reranked":ranks,"old_candidate_ms":old_ms,"new_candidate_ms":new_ms}));
    }
    eprintln!(
        "{}",
        json!({"suite":"paired-synthetic-quality","queries":report,
        "hits_at_1_5_20_old_new_reranked":totals,"query_count":cases.len(),"embedding":"disabled"})
    );
    assert!(
        totals[2][1] > totals[1][1],
        "reranking must improve exact evidence"
    );
    assert!(
        totals[2][2] >= totals[0][2],
        "new retrieval must not lose Recall@20 on this fixture"
    );
    assert_eq!(
        totals[2][0],
        cases.len(),
        "exact evidence and paraphrase regressions"
    );
    db.close().await;
}
