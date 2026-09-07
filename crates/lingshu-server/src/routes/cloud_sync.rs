//! Local half of E2EE sync. No cloud credentials or encryption keys enter this API.
use crate::{
    auth::{self, AuthUser},
    error::AppError,
    state::AppState,
};
use axum::{
    extract::{Path, Query, State},
    routing, Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

// Explicit portable fields: never export user IDs, project/team ACLs, vectors,
// local access counters or foreign-key links to unsynchronized objects.
const FIELDS: &[&str] = &[
    "id",
    "memory_type",
    "content",
    "importance",
    "metadata",
    "tier",
    "source_memory_ids",
    "memory_layer",
    "owner_agent_id",
    "scene_tags",
    "occurred_at",
    "valid_from",
    "valid_to",
    "source_type",
    "source_trust",
    "fact_key",
    "fact_value",
    "conflict_status",
    "retention_status",
    "context_scope",
    "deleted_at",
    "created_at",
    "updated_at",
];
fn projection() -> String {
    format!(
        "jsonb_build_object({})",
        FIELDS
            .iter()
            .map(|f| format!("'{f}', m.{f}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}
const SCOPE: &str = "m.user_id = $1 AND m.project_id IS NULL AND m.memory_scope = 'private'";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/cloud-sync/memories", routing::get(list))
        .route("/api/v1/cloud-sync/memories/:id", routing::post(apply))
}
#[derive(Deserialize)]
struct Page {
    after: Option<Uuid>,
}

async fn list(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Query(page): Query<Page>,
) -> Result<Json<Value>, AppError> {
    let user = auth::require_user(auth).await?;
    let p = projection();
    let rows: Vec<(Value, String)> = sqlx::query_as(&format!(
        "SELECT {p}, md5(({p})::text) FROM memories m WHERE {SCOPE} AND ($2::uuid IS NULL OR m.id > $2) ORDER BY m.id LIMIT 100"
    )).bind(user).bind(page.after).fetch_all(&state.db).await?;
    let next = if rows.len() == 100 {
        rows.last().map(|r| r.0["id"].clone())
    } else {
        None
    };
    Ok(Json(
        json!({"user_id": user, "records": rows.into_iter().map(|(data, token)| json!({"data": data, "token": token})).collect::<Vec<_>>(), "next": next}),
    ))
}

#[derive(Deserialize)]
struct Apply {
    data: Value,
    expected_token: Option<String>,
    user_id: Uuid,
}

fn validate(data: &Value, id: Uuid) -> Result<(), AppError> {
    let valid = data
        .as_object()
        .is_some_and(|o| o.len() == FIELDS.len() && FIELDS.iter().all(|f| o.contains_key(*f)))
        && data["id"].as_str().and_then(|s| Uuid::parse_str(s).ok()) == Some(id)
        && data["content"]
            .as_str()
            .is_some_and(|s| !s.trim().is_empty())
        && data["importance"]
            .as_f64()
            .is_some_and(|n| (0.0..=1.0).contains(&n))
        && serde_json::to_vec(data).is_ok_and(|b| b.len() <= 35_000);
    if !valid {
        return Err(AppError::BadRequest(
            "无效的同步记忆或记忆超过 35 KB".into(),
        ));
    }
    Ok(())
}

async fn apply(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<Apply>,
) -> Result<Json<Value>, AppError> {
    let user = auth::require_user(auth).await?;
    validate(&req.data, id)?;
    if req.user_id != user {
        return Err(AppError::Conflict("本地账户已切换，请重新同步".into()));
    }
    let p = projection();
    let mut tx = state.db.begin().await?;
    // Lock existing rows; INSERT's primary key also protects concurrent creation.
    let current: Option<(String,)> = sqlx::query_as(&format!(
        "SELECT md5(({p})::text) FROM memories m WHERE {SCOPE} AND m.id = $2 FOR UPDATE"
    ))
    .bind(user)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    if current.as_ref().map(|r| &r.0) != req.expected_token.as_ref() {
        return Err(AppError::Conflict("本地记忆已变化，请重新同步".into()));
    }
    let columns = FIELDS.join(",");
    let sql = if current.is_some() {
        let assignments = FIELDS
            .iter()
            .filter(|f| **f != "id")
            .map(|f| format!("{f} = v.{f}"))
            .collect::<Vec<_>>()
            .join(",");
        format!("UPDATE memories m SET {assignments}, vector_id = NULL FROM jsonb_populate_record(NULL::memories, $3::jsonb) v WHERE {SCOPE} AND m.id = $2")
    } else {
        format!("INSERT INTO memories (user_id,{columns}) SELECT $1,{} FROM jsonb_populate_record(NULL::memories, $3::jsonb) v WHERE v.id = $2 ON CONFLICT DO NOTHING", FIELDS.iter().map(|f| format!("v.{f}")).collect::<Vec<_>>().join(","))
    };
    let changed = sqlx::query(&sql)
        .bind(user)
        .bind(id)
        .bind(&req.data)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if changed != 1 {
        return Err(AppError::Conflict("记忆标识已被占用，请重新同步".into()));
    }
    let token: String = sqlx::query_scalar(&format!(
        "SELECT md5(({p})::text) FROM memories m WHERE {SCOPE} AND m.id = $2"
    ))
    .bind(user)
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    // Rebuild the receiving device's vector using its own embedding provider.
    if let Some(vector) = &state.vector {
        if !req.data["deleted_at"].is_null() {
            if let Err(error) = vector.delete_points("memories", &[id]).await {
                tracing::warn!(%error, "Sync vector deletion failed");
            }
        } else if let Err(error) = crate::llm::memory::upsert_memory_vector(
            &state.db,
            vector,
            &state.llm,
            &state.config.llm.embed_model,
            user,
            id,
            req.data["content"].as_str().unwrap(),
        )
        .await
        {
            tracing::warn!(%error, "Sync vector rebuild deferred; memory is stored locally");
        }
    }
    Ok(Json(json!({"token": token})))
}

#[cfg(test)]
mod tests {
    use super::*;
    // Opt-in integration test against a disposable database; never uses DATABASE_URL.
    #[tokio::test]
    #[ignore]
    async fn postgres_round_trip_scope_and_concurrency() {
        let url = std::env::var("CLOUD_SYNC_TEST_DATABASE_URL")
            .expect("set a disposable sync test database URL");
        assert!(url.ends_with("/lingshu_sync_test"));
        let admin = sqlx::PgPool::connect(&format!(
            "{}/postgres",
            url.trim_end_matches("/lingshu_sync_test")
        ))
        .await
        .unwrap();
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = 'lingshu_sync_test')",
        )
        .fetch_one(&admin)
        .await
        .unwrap();
        if !exists {
            sqlx::query("CREATE DATABASE lingshu_sync_test")
                .execute(&admin)
                .await
                .unwrap();
        }
        admin.close().await;
        let config = serde_json::from_value(
            json!({"database": {"url": url}, "redis": {"url": ""}, "qdrant": {"url": ""}}),
        )
        .unwrap();
        let state = AppState::new(&config).await.unwrap();
        let user = Uuid::new_v4();
        let other = Uuid::new_v4();
        for uid in [user, other] {
            sqlx::query("INSERT INTO users (id,email,display_name,password_hash) VALUES ($1,$2,'Sync test','not-a-password')")
                .bind(uid).bind(format!("{uid}@example.test")).execute(&state.db).await.unwrap();
        }
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO memories (id,user_id,memory_type,content) VALUES ($1,$2,'fact','Original')")
            .bind(id).bind(user).execute(&state.db).await.unwrap();
        sqlx::query("INSERT INTO memories (user_id,memory_type,content,memory_scope) VALUES ($1,'fact','Shared secret','shared')")
            .bind(user).execute(&state.db).await.unwrap();
        let auth = || Some(AuthUser { user_id: user });
        let snapshot = list(State(state.clone()), auth(), Query(Page { after: None }))
            .await
            .unwrap()
            .0;
        assert_eq!(snapshot["records"].as_array().unwrap().len(), 1);
        let row = &snapshot["records"][0];
        let data = row["data"].clone();
        assert!(data.get("user_id").is_none());
        assert!(data.get("vector_id").is_none());
        let token = row["token"].as_str().unwrap().to_string();
        let mut updated = data.clone();
        updated["content"] = json!("Updated");
        let request = || Apply {
            data: updated.clone(),
            expected_token: Some(token.clone()),
            user_id: user,
        };
        let (a, b) = tokio::join!(
            apply(State(state.clone()), auth(), Path(id), Json(request())),
            apply(State(state.clone()), auth(), Path(id), Json(request()))
        );
        assert_ne!(
            a.is_ok(),
            b.is_ok(),
            "exactly one concurrent writer must succeed"
        );
        assert!(matches!(
            if a.is_err() { a } else { b },
            Err(AppError::Conflict(_))
        ));
        let foreign = Apply {
            data: data.clone(),
            expected_token: None,
            user_id: other,
        };
        assert!(matches!(
            apply(
                State(state.clone()),
                Some(AuthUser { user_id: other }),
                Path(id),
                Json(foreign)
            )
            .await,
            Err(AppError::Conflict(_))
        ));
        let incoming_id = Uuid::new_v4();
        let mut incoming = data.clone();
        incoming["id"] = json!(incoming_id);
        let created = apply(
            State(state.clone()),
            auth(),
            Path(incoming_id),
            Json(Apply {
                data: incoming.clone(),
                expected_token: None,
                user_id: user,
            }),
        )
        .await
        .unwrap()
        .0;
        incoming["deleted_at"] = json!("2026-09-07T00:00:00+00:00");
        let _ = apply(
            State(state.clone()),
            auth(),
            Path(incoming_id),
            Json(Apply {
                data: incoming,
                expected_token: Some(created["token"].as_str().unwrap().into()),
                user_id: user,
            }),
        )
        .await
        .unwrap();
        let deleted: bool =
            sqlx::query_scalar("SELECT deleted_at IS NOT NULL FROM memories WHERE id = $1")
                .bind(incoming_id)
                .fetch_one(&state.db)
                .await
                .unwrap();
        assert!(deleted);
        assert!(matches!(
            list(State(state.clone()), None, Query(Page { after: None })).await,
            Err(AppError::Unauthorized(_))
        ));
        for uid in [user, other] {
            sqlx::query("DELETE FROM users WHERE id = $1")
                .bind(uid)
                .execute(&state.db)
                .await
                .unwrap();
        }
        state.db.close().await;
    }
    #[test]
    fn rejects_nonportable_fields_and_oversized_payloads() {
        let id = Uuid::new_v4();
        let mut data = json!({});
        for field in FIELDS {
            data[*field] = Value::Null;
        }
        data["id"] = json!(id);
        data["content"] = json!("memory");
        data["importance"] = json!(0.5);
        assert!(validate(&data, id).is_ok());
        data["user_id"] = json!(Uuid::new_v4());
        assert!(validate(&data, id).is_err());
        data.as_object_mut().unwrap().remove("user_id");
        data["content"] = json!("a".repeat(35_001));
        assert!(validate(&data, id).is_err());
    }
}
