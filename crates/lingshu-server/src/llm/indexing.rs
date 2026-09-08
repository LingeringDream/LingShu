//! Durable vector indexing. Leases survive process crashes; generation checks
//! ensure an edit made during embedding is not accidentally acknowledged.
#[cfg(test)]
#[path = "indexing_tests.rs"]
mod tests;
use super::client::LlmClient;
use chrono::{DateTime, Utc};
use lingshu_vector::search::QdrantClient;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct Job {
    memory_id: Uuid,
    user_id: Uuid,
    generation: i64,
    attempts: i32,
}

pub fn start(state: crate::state::AppState, vector: QdrantClient) {
    tokio::spawn(async move {
        loop {
            if state.db.is_closed() {
                break;
            }
            match process_with_settings(
                &state.db,
                &vector,
                &state.llm,
                &state.config.llm.embed_model,
                Some(&state),
            )
            .await
            {
                Ok(true) => continue,
                Ok(false) => {}
                Err(error) => tracing::warn!(%error, "Memory index worker failed; retrying"),
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
}

pub(crate) async fn process_one(
    db: &PgPool,
    vector: &QdrantClient,
    llm: &LlmClient,
    model: &str,
) -> anyhow::Result<bool> {
    process_with_settings(db, vector, llm, model, None).await
}

async fn process_with_settings(
    db: &PgPool,
    vector: &QdrantClient,
    llm: &LlmClient,
    model: &str,
    state: Option<&crate::state::AppState>,
) -> anyhow::Result<bool> {
    let lease = Uuid::new_v4();
    let job: Option<Job> = sqlx::query_as(
        "UPDATE memory_index_jobs SET lease_id=$1, locked_until=NOW()+INTERVAL '2 minutes'
         WHERE memory_id=(SELECT memory_id FROM memory_index_jobs
           WHERE available_at<=NOW() AND (locked_until IS NULL OR locked_until<NOW())
           ORDER BY available_at,memory_id FOR UPDATE SKIP LOCKED LIMIT 1)
         RETURNING memory_id,user_id,generation,attempts",
    )
    .bind(lease)
    .fetch_optional(db)
    .await?;
    let Some(job) = job else {
        return Ok(false);
    };
    let result = tokio::time::timeout(std::time::Duration::from_secs(60), async {
        let memory: Option<(String, Option<DateTime<Utc>>)> =
            sqlx::query_as("SELECT content,deleted_at FROM memories WHERE id=$1 AND user_id=$2")
                .bind(job.memory_id)
                .bind(job.user_id)
                .fetch_optional(db)
                .await?;
        match memory {
            Some((content, None)) => {
                // Background indexing must use the same saved provider/key as
                // this user's chat, rather than silently reverting to Ollama.
                let effective_llm = if let Some(state) = state {
                    let settings =
                        crate::routes::settings::llm_settings_for_user(state, job.user_id).await;
                    crate::routes::settings::resolve_client_for_user(state, job.user_id, &settings)
                        .await
                        .map_err(|error| anyhow::anyhow!(error.to_string()))?
                } else {
                    llm.clone()
                };
                super::memory::index_memory_vector(
                    db,
                    vector,
                    &effective_llm,
                    model,
                    job.user_id,
                    job.memory_id,
                    &content,
                )
                .await?
            }
            _ => vector.delete_points("memories", &[job.memory_id]).await?,
        }
        Ok::<(), anyhow::Error>(())
    })
    .await;
    match result {
        Ok(Ok(())) => {
            sqlx::query("DELETE FROM memory_index_jobs WHERE memory_id=$1 AND generation=$2 AND lease_id=$3")
                .bind(job.memory_id).bind(job.generation).bind(lease).execute(db).await?;
            // An intervening edit has a newer generation: release it immediately.
            sqlx::query("UPDATE memory_index_jobs SET lease_id=NULL,locked_until=NULL WHERE memory_id=$1 AND lease_id=$2")
                .bind(job.memory_id).bind(lease).execute(db).await?;
        }
        failure => {
            let message = match failure {
                Ok(Err(error)) => error.to_string(),
                Err(_) => "Index operation timed out".to_string(),
                Ok(Ok(())) => unreachable!(),
            };
            let delay = 2_i32.pow((job.attempts.max(0) as u32).min(8));
            sqlx::query("UPDATE memory_index_jobs SET lease_id=NULL,locked_until=NULL,
                attempts=CASE WHEN generation=$2 THEN attempts+1 ELSE attempts END,
                last_error=$4,
                available_at=CASE WHEN generation=$2 THEN NOW()+make_interval(secs=>$5::double precision) ELSE NOW() END
                WHERE memory_id=$1 AND lease_id=$3")
                .bind(job.memory_id).bind(job.generation).bind(lease).bind(message)
                .bind(delay as f64).execute(db).await?;
        }
    }
    Ok(true)
}
