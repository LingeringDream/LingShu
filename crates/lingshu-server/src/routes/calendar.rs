use axum::{
    extract::{Path, Query},
    routing, Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::llm::client::ChatMessage;
use crate::routes::permissions::{permissions_for_user, PermissionSettings};
use crate::routes::settings::llm_settings_for_user;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/calendar/parse", routing::post(parse_calendar))
        .route(
            "/api/v1/calendar/events",
            routing::get(list_events).post(create_event),
        )
        .route(
            "/api/v1/calendar/events/:id",
            routing::patch(update_event).delete(delete_event),
        )
        .route(
            "/api/v1/calendar/events/:id/confirm",
            routing::post(confirm_event),
        )
        .route(
            "/api/v1/calendar/events/:id/apple-event",
            routing::post(save_apple_event_id),
        )
        .route(
            "/api/v1/calendar/events/:id/external",
            routing::patch(write_external_event_id),
        )
}

// ── Types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, ToSchema)]
pub struct ParseRequest {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ParsedEvent {
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_time: String,
    pub end_time: String,
    pub attendees: Vec<String>,
    pub calendar_name: String,
    pub confidence: f32,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EventResponse {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub attendees: serde_json::Value,
    pub status: String,
    pub calendar_name: String,
    pub parse_confidence: Option<f32>,
    pub source_input: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateEventRequest {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    #[serde(default)]
    pub attendees: Vec<String>,
    #[serde(default = "default_calendar")]
    pub calendar_name: String,
    #[serde(default)]
    pub source_input: Option<String>,
    #[serde(default)]
    pub parse_confidence: Option<f32>,
}

fn default_calendar() -> String {
    "default".to_string()
}

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

// ── Helpers ────────────────────────────────────────────────────────

async fn check_calendar_permission(
    state: &AppState,
    user_id: Uuid,
) -> Result<PermissionSettings, AppError> {
    let perms = permissions_for_user(state, user_id).await;
    if !perms.l1_calendar {
        return Err(AppError::Forbidden(
            "Calendar access (L1) is not enabled. Enable it via PATCH /api/v1/permissions.".into(),
        ));
    }
    Ok(perms)
}

fn validate_event_time(start: DateTime<Utc>, end: DateTime<Utc>) -> Result<(), AppError> {
    if end <= start {
        return Err(AppError::Validation(
            "end_time must be after start_time".into(),
        ));
    }
    Ok(())
}

/// Parse an RFC 3339 time string into [`DateTime<Utc>`].
fn parse_time(raw: &str) -> Result<DateTime<Utc>, AppError> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| {
            AppError::Validation(format!(
                "Invalid time '{raw}': expected RFC 3339 format (e.g. 2026-06-06T15:00:00+08:00). {e}"
            ))
        })
}

/// Insert a calendar event and return the row. Shared by parse_calendar
/// (auto-create after LLM parse) and create_event (manual / future flows).
async fn insert_calendar_event(
    db: &sqlx::PgPool,
    user_id: Uuid,
    perms: &PermissionSettings,
    req: CreateEventRequest,
) -> Result<EventRow, AppError> {
    validate_event_time(req.start_time, req.end_time)?;
    let attendees = serde_json::to_value(&req.attendees).unwrap_or_default();

    let status = if perms.l1_require_confirmation {
        "pending_confirmation"
    } else {
        "confirmed"
    };

    let row: EventRow = sqlx::query_as(
        "INSERT INTO calendar_events \
         (user_id, title, description, location, start_time, end_time, \
          attendees, calendar_name, parse_confidence, source_input, status) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) \
         RETURNING id, title, description, location, start_time, end_time, \
         attendees, status, calendar_name, parse_confidence, source_input, created_at",
    )
    .bind(user_id)
    .bind(&req.title)
    .bind(&req.description)
    .bind(&req.location)
    .bind(req.start_time)
    .bind(req.end_time)
    .bind(&attendees)
    .bind(&req.calendar_name)
    .bind(req.parse_confidence)
    .bind(&req.source_input)
    .bind(status)
    .fetch_one(db)
    .await?;

    Ok(row)
}

/// Safe JSON extraction: find a pair of brackets { } or [ ] in LLM output,
/// guarding against malformed responses.
fn extract_json_slice(raw: &str) -> &str {
    let text = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    // Try { } first, then [ ]
    if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) {
        if start < end {
            return &text[start..=end];
        }
    }
    if let (Some(start), Some(end)) = (text.find('['), text.rfind(']')) {
        if start < end {
            return &text[start..=end];
        }
    }
    // Fallback: return the trimmed text as-is
    text
}

// ── Public helpers (reusable from chat route) ─────────────────────

/// Parse natural language text and create a calendar event.
/// Shared by the `/api/v1/calendar/parse` endpoint and the chat tool-calling path.
pub async fn parse_and_create_event(
    state: &AppState,
    user_id: Uuid,
    text: &str,
) -> Result<EventResponse, AppError> {
    let perms = check_calendar_permission(state, user_id).await?;
    let settings = llm_settings_for_user(state, user_id).await;
    let model = settings.model.clone();

    if model.is_empty() {
        return Err(AppError::Internal(anyhow::anyhow!("Model not configured.")));
    }

    // Use per-user provider overrides so calendar parsing uses the same
    // LLM backend as chat (e.g. DeepSeek) instead of falling back to Ollama.
    let llm = if settings.provider == "openai" && !settings.api_base_url.is_empty() {
        state.llm.with_overrides(
            if settings.api_key.is_empty() { None } else { Some(settings.api_key.clone()) },
            if settings.api_base_url.is_empty() { None } else { Some(settings.api_base_url.clone()) },
        )
    } else {
        state.llm.clone()
    };

    let now = Utc::now().to_rfc3339();
    let prompt = format!(
        r###"你是 灵枢（LingShu）的日历解析引擎。请从用户的自然语言输入中提取一个日程事件。

今天是 {now}。所有用户的时间表述都是相对于此日期的。

## 解析规则
- **title**：简短日程标题。如果用户说「和张三开会讨论需求」，标题应为「与张三讨论需求」而非「开会」。
- **start_time / end_time**：ISO 8601 格式。如果用户只说开始时间没说时长，默认持续 1 小时。如果只说「下午」，默认 14:00。注意区分「明天下午」和「今天下午」。
- **description**：备注、会议链接、讨论要点等额外信息。没有则设为 null。
- **location**：线下地点或线上会议链接。没有则设为 null。
- **attendees**：参与人姓名数组。从「和张三李四一起」「叫上王五」等表述中提取。没有则为空数组 []。
- **calendar_name**：根据语境推断。提到工作/会议/客户 → "work"；提到私人/家人/健身/看病 → "personal"；其余 → "default"。
- **confidence**：你的解析置信度。时间表述模糊时降低，例如只说「下周」而非具体日期，或参与人不明确。

## 示例
输入：「明天下午3点和产品组开需求评审会，在3楼会议室」
输出：
```json
{{
  "title": "产品组需求评审会",
  "description": null,
  "location": "3楼会议室",
  "start_time": "...T15:00:00+08:00",
  "end_time": "...T16:00:00+08:00",
  "attendees": ["产品组"],
  "calendar_name": "work",
  "confidence": 0.9
}}
```

## 重要
严格返回一个 JSON 对象，不要包含 markdown 标记或额外解释。

输入：{text}
JSON："###
    );

    let messages = vec![ChatMessage::user(prompt)];

    let mut response = String::new();
    let mut last_err = String::new();
    for attempt in 1..=3 {
        match llm.chat(&model, messages.clone(), None).await {
            Ok(r) => {
                response = r;
                last_err.clear();
                break;
            }
            Err(e) => {
                last_err = e.to_string();
                tracing::warn!(%model, attempt, %last_err, "calendar llm call failed, retrying");
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    if response.is_empty() && !last_err.is_empty() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "Calendar LLM call failed after 3 attempts (model={model}): {last_err}"
        )));
    }

    let slice = extract_json_slice(&response);
    let parsed: ParsedEvent = serde_json::from_str(slice)
        .map_err(|e| AppError::Validation(format!("Failed to parse calendar JSON: {e}")))?;

    let create_req = CreateEventRequest {
        title: parsed.title,
        description: parsed.description,
        location: parsed.location,
        start_time: parse_time(&parsed.start_time)?,
        end_time: parse_time(&parsed.end_time)?,
        attendees: parsed.attendees,
        calendar_name: parsed.calendar_name,
        source_input: Some(text.to_string()),
        parse_confidence: Some(parsed.confidence),
    };

    let row = insert_calendar_event(&state.db, user_id, &perms, create_req).await?;
    Ok(row.into_response())
}

/// List calendar events for a user. Shared by the `/api/v1/calendar/events`
/// endpoint and the chat tool-calling path.
pub async fn list_user_events(
    state: &AppState,
    user_id: Uuid,
    limit: Option<i64>,
) -> Result<Vec<EventResponse>, AppError> {
    check_calendar_permission(state, user_id).await?;
    let limit = limit.unwrap_or(50).min(200);

    let rows: Vec<EventRow> = sqlx::query_as(
        "SELECT id, title, description, location, start_time, end_time, \
         attendees, status, calendar_name, parse_confidence, source_input, created_at \
         FROM calendar_events WHERE user_id = $1 \
         ORDER BY start_time DESC LIMIT $2",
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    Ok(rows.into_iter().map(EventRow::into_response).collect())
}

/// Delete a calendar event by id. Shared by the HTTP endpoint and the
/// chat tool-calling path. Unlike the HTTP handler, this does not require
/// the confirmation flag — the chat tool call itself implies user intent.
pub async fn delete_user_event(
    state: &AppState,
    user_id: Uuid,
    event_id: Uuid,
) -> Result<(), AppError> {
    check_calendar_permission(state, user_id).await?;

    let rows = sqlx::query("DELETE FROM calendar_events WHERE id = $1 AND user_id = $2")
        .bind(event_id)
        .bind(user_id)
        .execute(&state.db)
        .await?
        .rows_affected();

    if rows == 0 {
        return Err(AppError::NotFound("Event not found".into()));
    }
    Ok(())
}

// ── Handler: parse ─────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/api/v1/calendar/parse",
    request_body = ParseRequest,
    responses((status = 201, description = "Parsed and created calendar event", body = EventResponse))
)]
async fn parse_calendar(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Json(req): Json<ParseRequest>,
) -> Result<(axum::http::StatusCode, Json<EventResponse>), AppError> {
    let user_id = auth::require_user(auth).await?;
    let event = parse_and_create_event(&state, user_id, &req.text).await?;
    Ok((axum::http::StatusCode::CREATED, Json(event)))
}

// ── Handler: list ──────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/calendar/events",
    responses((status = 200, body = Vec<EventResponse>))
)]
async fn list_events(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Query(params): Query<ListParams>,
) -> Result<Json<Vec<EventResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let events = list_user_events(&state, user_id, params.limit).await?;
    Ok(Json(events))
}

// ── Handler: create ────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/api/v1/calendar/events",
    request_body = CreateEventRequest,
    responses((status = 201, body = EventResponse))
)]
async fn create_event(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Json(req): Json<CreateEventRequest>,
) -> Result<(axum::http::StatusCode, Json<EventResponse>), AppError> {
    let user_id = auth::require_user(auth).await?;
    let perms = check_calendar_permission(&state, user_id).await?;
    let row = insert_calendar_event(&state.db, user_id, &perms, req).await?;
    Ok((axum::http::StatusCode::CREATED, Json(row.into_response())))
}

// ── Handler: update ────────────────────────────────────────────────

#[utoipa::path(
    patch,
    path = "/api/v1/calendar/events/{id}",
    request_body = CreateEventRequest,
    responses((status = 200, body = EventResponse))
)]
async fn update_event(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateEventRequest>,
) -> Result<Json<EventResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let perms = check_calendar_permission(&state, user_id).await?;
    if perms.l1_require_confirmation {
        return Err(AppError::Forbidden(
            "Calendar updates require an explicit confirmation flow before execution.".into(),
        ));
    }
    validate_event_time(req.start_time, req.end_time)?;
    let attendees = serde_json::to_value(&req.attendees).unwrap_or_default();

    let row: EventRow = sqlx::query_as(
        "UPDATE calendar_events SET title=$1, description=$2, location=$3, \
         start_time=$4, end_time=$5, attendees=$6, calendar_name=$7, \
         updated_at=NOW() \
         WHERE id=$8 AND user_id=$9 \
         RETURNING id, title, description, location, start_time, end_time, \
         attendees, status, calendar_name, parse_confidence, source_input, created_at",
    )
    .bind(&req.title)
    .bind(&req.description)
    .bind(&req.location)
    .bind(req.start_time)
    .bind(req.end_time)
    .bind(&attendees)
    .bind(&req.calendar_name)
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Event not found".into()))?;

    Ok(Json(row.into_response()))
}

// ── Handler: confirm ───────────────────────────────────────────────

/// Confirm a `pending_confirmation` event, flipping it to `confirmed`.
///
/// This closes the L1 confirmation loop: when `l1_require_confirmation` is on
/// (the default), `parse`/`create` persist events as `pending_confirmation`,
/// and this endpoint is the explicit, user-driven approval step that activates
/// them. Idempotency: only rows currently in `pending_confirmation` are
/// affected, so a second call returns 404 rather than silently re-confirming.
#[utoipa::path(
    post,
    path = "/api/v1/calendar/events/{id}/confirm",
    params(("id" = Uuid, Path, description = "Event ID")),
    responses(
        (status = 200, body = EventResponse),
        (status = 403, description = "Calendar access (L1) not enabled"),
        (status = 404, description = "Event not found or not pending confirmation")
    )
)]
async fn confirm_event(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<EventResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    check_calendar_permission(&state, user_id).await?;

    let row: EventRow = sqlx::query_as(
        "UPDATE calendar_events SET status = 'confirmed', updated_at = NOW() \
         WHERE id = $1 AND user_id = $2 AND status = 'pending_confirmation' \
         RETURNING id, title, description, location, start_time, end_time, \
         attendees, status, calendar_name, parse_confidence, source_input, created_at",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Event not found or not pending confirmation".into()))?;

    Ok(Json(row.into_response()))
}

// ── Handler: save apple_event_id ──────────────────────────────────────

/// Store the EventKit `eventIdentifier` after syncing to Apple Calendar.
///
/// Called by the frontend after a successful EventKit `create_calendar_event`
/// Tauri command. The `apple_event_id` is used for future update/delete
/// sync and deduplication.
///
/// Only confirmed events can receive an apple_event_id; pending ones
/// should be confirmed first.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SaveAppleEventRequest {
    apple_event_id: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/calendar/events/{id}/apple-event",
    request_body = SaveAppleEventRequest,
    responses(
        (status = 200, body = EventResponse),
        (status = 404, description = "Event not found"),
        (status = 409, description = "Event is not confirmed — confirm first"),
        (status = 422, description = "Validation error")
    )
)]
async fn save_apple_event_id(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<SaveAppleEventRequest>,
) -> Result<Json<EventResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    check_calendar_permission(&state, user_id).await?;

    let apple_event_id = req.apple_event_id.trim();
    if apple_event_id.is_empty() {
        return Err(AppError::Validation(
            "apple_event_id must not be empty".into(),
        ));
    }

    // Only save apple_event_id on confirmed events.
    let row = sqlx::query_as::<_, EventRow>(
        "UPDATE calendar_events SET apple_event_id = $1, updated_at = NOW() \
         WHERE id = $2 AND user_id = $3 AND status = 'confirmed' \
         RETURNING id, title, description, location, start_time, end_time, \
         attendees, status, calendar_name, parse_confidence, source_input, created_at",
    )
    .bind(apple_event_id)
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;

    let Some(row) = row else {
        let status: Option<String> =
            sqlx::query_scalar("SELECT status FROM calendar_events WHERE id = $1 AND user_id = $2")
                .bind(id)
                .bind(user_id)
                .fetch_optional(&state.db)
                .await?;

        return match status.as_deref() {
            Some("confirmed") | None => Err(AppError::NotFound("Event not found".into())),
            Some(_) => Err(AppError::Conflict(
                "Confirm the event before syncing to Apple Calendar".into(),
            )),
        };
    };

    Ok(Json(row.into_response()))
}

// ── Handler: write external_event_id ──────────────────────────────────

/// Store the EventKit `eventIdentifier` after syncing to the system calendar.
///
/// Called by the frontend after a successful EventKit `create_calendar_event`
/// Tauri command. Writes the opaque eventIdentifier to `external_event_id`
/// and sets `synced_to_eventkit = true`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct WriteExternalEventRequest {
    external_event_id: String,
}

#[utoipa::path(
    patch,
    path = "/api/v1/calendar/events/{id}/external",
    request_body = WriteExternalEventRequest,
    responses(
        (status = 200, body = EventResponse, description = "external_event_id saved"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Calendar L1 permission not enabled"),
        (status = 404, description = "Event not found"),
        (status = 409, description = "Event is not confirmed — confirm first"),
        (status = 422, description = "Validation error"),
    )
)]
async fn write_external_event_id(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<WriteExternalEventRequest>,
) -> Result<Json<EventResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    check_calendar_permission(&state, user_id).await?;

    let external_event_id = req.external_event_id.trim();
    if external_event_id.is_empty() {
        return Err(AppError::Validation(
            "external_event_id must not be empty".into(),
        ));
    }

    let row = sqlx::query_as::<_, EventRow>(
        "UPDATE calendar_events \
         SET external_event_id = $1, synced_to_eventkit = true, updated_at = NOW() \
         WHERE id = $2 AND user_id = $3 AND status = 'confirmed' \
         RETURNING id, title, description, location, start_time, end_time, \
         attendees, status, calendar_name, parse_confidence, source_input, created_at",
    )
    .bind(external_event_id)
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;

    let Some(row) = row else {
        // Distinguish "not found" from "exists but not confirmed"
        let status: Option<String> =
            sqlx::query_scalar("SELECT status FROM calendar_events WHERE id = $1 AND user_id = $2")
                .bind(id)
                .bind(user_id)
                .fetch_optional(&state.db)
                .await?;

        return match status.as_deref() {
            Some("confirmed") | None => Err(AppError::NotFound("Event not found".into())),
            Some(_) => Err(AppError::Conflict(
                "Confirm the event before syncing to system calendar".into(),
            )),
        };
    };

    Ok(Json(row.into_response()))
}

// ── Handler: delete ────────────────────────────────────────────────

#[utoipa::path(
    delete,
    path = "/api/v1/calendar/events/{id}",
    responses((status = 204))
)]
async fn delete_event(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, AppError> {
    let user_id = auth::require_user(auth).await?;
    check_calendar_permission(&state, user_id).await?;

    let rows = sqlx::query("DELETE FROM calendar_events WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(&state.db)
        .await?
        .rows_affected();

    if rows == 0 {
        return Err(AppError::NotFound("Event not found".into()));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── FromRow ────────────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct EventRow {
    id: Uuid,
    title: String,
    description: Option<String>,
    location: Option<String>,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    attendees: serde_json::Value,
    status: String,
    calendar_name: String,
    parse_confidence: Option<f32>,
    source_input: Option<String>,
    created_at: DateTime<Utc>,
}

impl EventRow {
    fn into_response(self) -> EventResponse {
        EventResponse {
            id: self.id,
            title: self.title,
            description: self.description,
            location: self.location,
            start_time: self.start_time,
            end_time: self.end_time,
            attendees: self.attendees,
            status: self.status,
            calendar_name: self.calendar_name,
            parse_confidence: self.parse_confidence,
            source_input: self.source_input,
            created_at: self.created_at,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_time_valid_rfc3339_succeeds() {
        let result = parse_time("2026-06-06T15:00:00+08:00").expect("valid RFC 3339 should parse");
        assert_eq!(result.to_rfc3339(), "2026-06-06T07:00:00+00:00");
    }

    #[test]
    fn parse_time_valid_rfc3339_utc_succeeds() {
        let result = parse_time("2026-12-01T09:30:00Z").expect("UTC RFC 3339 should parse");
        assert_eq!(result.to_rfc3339(), "2026-12-01T09:30:00+00:00");
    }

    #[test]
    fn parse_time_invalid_returns_validation() {
        let err = parse_time("tomorrow 3pm").unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(
                    msg.contains("tomorrow 3pm"),
                    "message should include the bad input"
                );
                assert!(
                    msg.contains("RFC 3339"),
                    "message should mention expected format"
                );
            }
            other => panic!("expected Validation error, got {other:?}"),
        }
    }

    #[test]
    fn parse_time_empty_string_returns_validation() {
        let err = parse_time("").unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn validate_event_time_end_after_start_is_ok() {
        let start = parse_time("2026-06-06T09:00:00Z").unwrap();
        let end = parse_time("2026-06-06T10:00:00Z").unwrap();
        assert!(validate_event_time(start, end).is_ok());
    }

    #[test]
    fn validate_event_time_end_equals_start_is_err() {
        let t = parse_time("2026-06-06T09:00:00Z").unwrap();
        let err = validate_event_time(t, t).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn validate_event_time_end_before_start_is_err() {
        let start = parse_time("2026-06-06T10:00:00Z").unwrap();
        let end = parse_time("2026-06-06T09:00:00Z").unwrap();
        let err = validate_event_time(start, end).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}
