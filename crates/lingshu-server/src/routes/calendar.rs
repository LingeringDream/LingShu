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
            "/api/v1/calendar/events/{id}",
            routing::patch(update_event).delete(delete_event),
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

// ── Handler: parse ─────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/api/v1/calendar/parse",
    request_body = ParseRequest,
    responses((status = 200, body = ParsedEvent))
)]
async fn parse_calendar(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Json(req): Json<ParseRequest>,
) -> Result<Json<ParsedEvent>, AppError> {
    let user_id = auth::require_user(auth).await?;
    check_calendar_permission(&state, user_id).await?;

    let settings = llm_settings_for_user(&state, user_id).await;
    let model = settings.model.clone();

    if model.is_empty() {
        return Err(AppError::Internal(anyhow::anyhow!("Model not configured.")));
    }

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

输入：{input}
JSON："###,
        now = now,
        input = req.text
    );

    let messages = vec![ChatMessage {
        role: "user".to_string(),
        content: prompt,
    }];

    let response = state
        .llm
        .chat(&model, messages, None)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Calendar parse failed: {e}")))?;

    let slice = extract_json_slice(&response);
    let parsed: ParsedEvent = serde_json::from_str(slice)
        .map_err(|e| AppError::Validation(format!("Failed to parse calendar JSON: {e}")))?;
    Ok(Json(parsed))
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
    check_calendar_permission(&state, user_id).await?;
    let limit = params.limit.unwrap_or(50).min(200);

    let rows: Vec<EventRow> = if let Some(st) = &params.status {
        sqlx::query_as(
            "SELECT id, title, description, location, start_time, end_time, \
             attendees, status, calendar_name, parse_confidence, source_input, created_at \
             FROM calendar_events WHERE user_id = $1 AND status = $2 \
             ORDER BY start_time DESC LIMIT $3",
        )
        .bind(user_id)
        .bind(st)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as(
            "SELECT id, title, description, location, start_time, end_time, \
             attendees, status, calendar_name, parse_confidence, source_input, created_at \
             FROM calendar_events WHERE user_id = $1 \
             ORDER BY start_time DESC LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    };

    Ok(Json(
        rows.into_iter().map(EventRow::into_response).collect(),
    ))
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
    .fetch_one(&state.db)
    .await?;

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
    let perms = check_calendar_permission(&state, user_id).await?;
    if perms.l1_require_confirmation {
        return Err(AppError::Forbidden(
            "Calendar deletes require an explicit confirmation flow before execution.".into(),
        ));
    }

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
