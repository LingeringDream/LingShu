use axum::{extract::State, routing, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/permissions",
        routing::get(get_permissions).patch(update_permissions),
    )
}

// ── Types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PermissionSettings {
    /// L0: Chat + pet + memory display (always on)
    pub l0_enabled: bool,
    /// L1: Calendar read/write
    pub l1_calendar: bool,
    /// L1 confirmation: require user approval for each calendar write
    pub l1_require_confirmation: bool,
    /// L2: Open apps, URLs, run Shortcuts
    pub l2_automation: bool,
    /// L2 whitelist mode: only allowed apps/URLs
    pub l2_whitelist_only: bool,
    /// L3: Accessibility (keyboard input, AX tree read)
    pub l3_accessibility: bool,
    /// L4: Screen recording + autonomous click (disabled by default, not in MVP)
    pub l4_autonomous: bool,
}

impl Default for PermissionSettings {
    fn default() -> Self {
        Self {
            l0_enabled: true,
            l1_calendar: false,
            l1_require_confirmation: true,
            l2_automation: false,
            l2_whitelist_only: true,
            l3_accessibility: false,
            l4_autonomous: false,
        }
    }
}

pub async fn permissions_for_user(state: &AppState, user_id: Uuid) -> PermissionSettings {
    state
        .permissions
        .read()
        .await
        .get(&user_id)
        .cloned()
        .unwrap_or_default()
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PermissionPatch {
    pub l1_calendar: Option<bool>,
    pub l1_require_confirmation: Option<bool>,
    pub l2_automation: Option<bool>,
    pub l2_whitelist_only: Option<bool>,
    pub l3_accessibility: Option<bool>,
    pub l4_autonomous: Option<bool>,
}

// ── Handlers ───────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/permissions",
    responses((status = 200, body = PermissionSettings), (status = 401))
)]
async fn get_permissions(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<PermissionSettings>, AppError> {
    let user_id = auth::require_user(auth).await?;

    Ok(Json(permissions_for_user(&state, user_id).await))
}

#[utoipa::path(
    patch,
    path = "/api/v1/permissions",
    request_body = PermissionPatch,
    responses((status = 200, body = PermissionSettings), (status = 401))
)]
async fn update_permissions(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Json(patch): Json<PermissionPatch>,
) -> Result<Json<PermissionSettings>, AppError> {
    let user_id = auth::require_user(auth).await?;

    let mut all_settings = state.permissions.write().await;
    let settings = all_settings.entry(user_id).or_default();

    if let Some(v) = patch.l1_calendar {
        settings.l1_calendar = v;
    }
    if let Some(v) = patch.l1_require_confirmation {
        settings.l1_require_confirmation = v;
    }
    if let Some(v) = patch.l2_automation {
        settings.l2_automation = v;
    }
    if let Some(v) = patch.l2_whitelist_only {
        settings.l2_whitelist_only = v;
    }
    if let Some(v) = patch.l3_accessibility {
        settings.l3_accessibility = v;
    }
    if let Some(v) = patch.l4_autonomous {
        settings.l4_autonomous = v;
    }

    Ok(Json(settings.clone()))
}
