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

/// Load permissions for a user: in-memory → DB → default.
pub async fn permissions_for_user(state: &AppState, user_id: Uuid) -> PermissionSettings {
    // 1. In-memory cache
    {
        let map = state.permissions.read().await;
        if let Some(perms) = map.get(&user_id) {
            return perms.clone();
        }
    }

    // 2. Database (migration 0022)
    if let Some(perms) = load_permissions_from_db(state, user_id).await {
        let mut map = state.permissions.write().await;
        map.insert(user_id, perms.clone());
        return perms;
    }

    PermissionSettings::default()
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

// ── DB helpers ─────────────────────────────────────────────────────

async fn load_permissions_from_db(state: &AppState, user_id: Uuid) -> Option<PermissionSettings> {
    let row: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT permissions FROM users WHERE id = $1 AND deleted_at IS NULL")
            .bind(user_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();
    row.and_then(|v| serde_json::from_value(v).ok())
}

async fn save_permissions_to_db(state: &AppState, user_id: Uuid, perms: &PermissionSettings) {
    let json = serde_json::to_value(perms).unwrap_or_default();
    let _ = sqlx::query(
        "UPDATE users SET permissions = $1, updated_at = now() WHERE id = $2 AND deleted_at IS NULL",
    )
    .bind(&json)
    .bind(user_id)
    .execute(&state.db)
    .await;
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

    // Persist so permissions survive restarts
    save_permissions_to_db(&state, user_id, settings).await;

    Ok(Json(settings.clone()))
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_permissions_have_l0_only() {
        let p = PermissionSettings::default();
        assert!(p.l0_enabled);
        assert!(!p.l1_calendar);
        assert!(p.l1_require_confirmation);
        assert!(!p.l2_automation);
        assert!(p.l2_whitelist_only);
        assert!(!p.l3_accessibility);
        assert!(!p.l4_autonomous);
    }

    #[test]
    fn permissions_json_round_trip() {
        let orig = PermissionSettings {
            l0_enabled: true,
            l1_calendar: true,
            l1_require_confirmation: false,
            l2_automation: true,
            l2_whitelist_only: false,
            l3_accessibility: true,
            l4_autonomous: false,
        };
        let json = serde_json::to_value(&orig).unwrap();
        let restored: PermissionSettings = serde_json::from_value(json).unwrap();
        assert_eq!(restored.l0_enabled, orig.l0_enabled);
        assert_eq!(restored.l1_calendar, orig.l1_calendar);
        assert_eq!(
            restored.l1_require_confirmation,
            orig.l1_require_confirmation
        );
        assert_eq!(restored.l2_automation, orig.l2_automation);
        assert_eq!(restored.l2_whitelist_only, orig.l2_whitelist_only);
        assert_eq!(restored.l3_accessibility, orig.l3_accessibility);
        assert_eq!(restored.l4_autonomous, orig.l4_autonomous);
    }

    #[test]
    fn permission_patch_partial_update() {
        let mut settings = PermissionSettings::default();
        // Apply a patch with only l1_calendar
        let patch = PermissionPatch {
            l1_calendar: Some(true),
            l1_require_confirmation: None,
            l2_automation: None,
            l2_whitelist_only: None,
            l3_accessibility: None,
            l4_autonomous: None,
        };
        if let Some(v) = patch.l1_calendar {
            settings.l1_calendar = v;
        }
        if let Some(v) = patch.l1_require_confirmation {
            settings.l1_require_confirmation = v;
        }
        // Only l1_calendar should change; l1_require_confirmation stays default
        assert!(settings.l1_calendar);
        assert!(settings.l1_require_confirmation); // unchanged default
        assert!(!settings.l2_automation); // unchanged default
    }

    #[test]
    fn permission_patch_all_fields() {
        let mut settings = PermissionSettings::default();
        let patch = PermissionPatch {
            l1_calendar: Some(true),
            l1_require_confirmation: Some(false),
            l2_automation: Some(true),
            l2_whitelist_only: Some(false),
            l3_accessibility: Some(true),
            l4_autonomous: Some(false),
        };
        // Apply all fields
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

        assert!(settings.l1_calendar);
        assert!(!settings.l1_require_confirmation);
        assert!(settings.l2_automation);
        assert!(!settings.l2_whitelist_only);
        assert!(settings.l3_accessibility);
        assert!(!settings.l4_autonomous);
    }

    #[test]
    fn permission_settings_db_json_null_is_default() {
        // When the DB returns SQL NULL for the permissions column,
        // serde_json::from_value(Value::Null) should fail → fallback to defaults.
        let result: Result<PermissionSettings, _> = serde_json::from_value(serde_json::Value::Null);
        assert!(result.is_err());
    }

    #[test]
    fn l0_enabled_is_always_true_in_default() {
        // L0 should never be disabled — it guards basic chat+pet functionality
        let p = PermissionSettings::default();
        assert!(p.l0_enabled);
    }
}
