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
    /// L2 whitelist: allowed app names (e.g. "Calculator") and URL prefixes
    /// (e.g. "https://github.com"). Enforced when `l2_whitelist_only` is on.
    /// Empty by default → nothing is allowed until the user adds entries.
    #[serde(default)]
    pub l2_whitelist: Vec<String>,
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
            l2_whitelist: Vec::new(),
        }
    }
}

impl PermissionSettings {
    /// Whether an L2 automation target may be acted on.
    ///
    /// Requires `l2_automation`. When `l2_whitelist_only` is on (the default),
    /// the target must match a whitelist entry. Use [`resolve_canonical_target`]
    /// to get the canonical OS-level name after this check passes.
    /// Default-deny: empty whitelist allows nothing.
    pub fn automation_allowed(&self, kind: &str, target: &str) -> bool {
        self.resolve_canonical_target(kind, target).is_some()
    }

    /// Return the canonical form of `target` if it's whitelisted.
    ///
    /// Apps: returns the whitelist entry (e.g. "Chrome" → "Google Chrome")
    /// so the Tauri side can pass the correct name to `open -a`.
    /// URLs/paths: returns the target unchanged.
    pub fn resolve_canonical_target(&self, kind: &str, target: &str) -> Option<String> {
        if !self.l2_automation {
            return None;
        }
        if !self.l2_whitelist_only {
            return Some(target.to_string());
        }
        let t = target.trim().to_lowercase();
        if t.is_empty() {
            return None;
        }
        self.l2_whitelist.iter().find_map(|entry| {
            let e = entry.trim();
            let el = e.to_lowercase();
            if el.is_empty() {
                return None;
            }
            let matched = match kind {
                "open_url" | "open_file" => t.starts_with(&el),
                _ => app_name_matches(&t, &el),
            };
            if matched {
                Some(e.to_string())
            } else {
                None
            }
        })
    }
}

/// Normalised app-name matching for L2 whitelist.
///
/// Strips `.app` suffix, then tries exact match, known macOS
/// localised-name aliases, and finally prefix abbreviation
/// (short whitelist entry matching a longer target, no reverse).
fn app_name_matches(target: &str, whitelist_entry: &str) -> bool {
    let t = target.strip_suffix(".app").unwrap_or(target);
    let w = whitelist_entry
        .strip_suffix(".app")
        .unwrap_or(whitelist_entry);

    // 1. Exact normalised match
    if t == w {
        return true;
    }

    // 2. macOS built-in app aliases (English ↔ Chinese, abbreviations)
    if let Some(aliases) = MACOS_APP_ALIASES.get(w) {
        if aliases.contains(&t) {
            return true;
        }
    }
    // Also check reverse: target may be the canonical name
    if let Some(aliases) = MACOS_APP_ALIASES.get(t) {
        if aliases.contains(&w) {
            return true;
        }
    }

    // 3. Abbreviation: short whitelist entry (2–5 chars) is allowed to
    //    match the start of a longer target. "Calc" → "Calculator" ✅
    //    but "Calculator" → "CalculatorEvil" ❌ (entry too long for an abbrev).
    // Only when the entry is an abbreviation (≤ 8 chars, shorter than target).
    if w.len() <= 8 && w.len() < t.len() && t.starts_with(w) {
        return true;
    }

    false
}

/// macOS built-in app name aliases. Only the most common localised
/// and abbreviated forms are included.
static MACOS_APP_ALIASES: std::sync::LazyLock<std::collections::HashMap<&str, Vec<&str>>> =
    std::sync::LazyLock::new(|| {
        let mut m = std::collections::HashMap::new();
        m.insert("calculator", vec!["计算器", "calc"]);
        m.insert("计算器", vec!["calculator", "calc"]);
        m.insert("safari", vec!["safari browser"]);
        m.insert("terminal", vec!["终端", "term"]);
        m.insert("终端", vec!["terminal", "term"]);
        m.insert(
            "system settings",
            vec!["settings", "系统设置", "preferences", "system preferences"],
        );
        m.insert("系统设置", vec!["system settings", "settings"]);
        m.insert("activity monitor", vec!["活动监视器"]);
        m.insert("活动监视器", vec!["activity monitor"]);
        m.insert("textedit", vec!["textedit.app", "文本编辑"]);
        m.insert("文本编辑", vec!["textedit"]);
        m.insert("finder", vec!["访达"]);
        m.insert("访达", vec!["finder"]);
        m.insert("notes", vec!["备忘录", "note"]);
        m.insert("备忘录", vec!["notes", "note"]);
        m.insert("reminders", vec!["提醒事项"]);
        m.insert("提醒事项", vec!["reminders"]);
        m.insert("calendar", vec!["日历", "ical"]);
        m.insert("日历", vec!["calendar", "ical"]);
        m.insert("mail", vec!["邮件", "apple mail"]);
        m.insert("邮件", vec!["mail", "apple mail"]);
        m.insert("photos", vec!["照片"]);
        m.insert("照片", vec!["photos"]);
        m.insert("music", vec!["音乐", "itunes"]);
        m.insert("音乐", vec!["music", "itunes"]);
        m.insert("messages", vec!["信息", "imessage"]);
        m.insert("信息", vec!["messages", "imessage"]);
        m.insert("app store", vec!["appstore"]);
        m.insert("appstore", vec!["app store"]);
        m.insert("visual studio code", vec!["code", "vscode", "vs code"]);
        m.insert("vscode", vec!["visual studio code", "code"]);
        m.insert("google chrome", vec!["chrome", "google chrome.app"]);
        m.insert("chrome", vec!["google chrome"]);
        m.insert("firefox", vec!["firefox.app"]);
        m.insert("obsidian", vec!["obsidian.app"]);
        m.insert("notion", vec!["notion.app"]);
        m
    });

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
    pub l2_whitelist: Option<Vec<String>>,
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

    // Warm the cache from DB first so a PATCH starts from the user's persisted
    // settings, not defaults — otherwise a cold-cache PATCH (e.g. first request
    // after a restart) would clobber fields the client didn't send.
    let _ = permissions_for_user(&state, user_id).await;

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
    if let Some(v) = patch.l2_whitelist {
        settings.l2_whitelist = v;
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
            l2_whitelist: vec!["Calculator".into(), "https://github.com".into()],
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
        assert_eq!(restored.l2_whitelist, orig.l2_whitelist);
    }

    #[test]
    fn permissions_json_missing_whitelist_defaults_empty() {
        // Old rows persisted before l2_whitelist existed must still deserialize
        // (serde(default) → empty vec) rather than failing.
        let json = serde_json::json!({
            "l0_enabled": true,
            "l1_calendar": false,
            "l1_require_confirmation": true,
            "l2_automation": false,
            "l2_whitelist_only": true,
            "l3_accessibility": false,
            "l4_autonomous": false
        });
        let p: PermissionSettings = serde_json::from_value(json).unwrap();
        assert!(p.l2_whitelist.is_empty());
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
            l2_whitelist: None,
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
            l2_whitelist: Some(vec!["Calculator".into()]),
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
        if let Some(v) = patch.l2_whitelist {
            settings.l2_whitelist = v;
        }

        assert!(settings.l1_calendar);
        assert!(!settings.l1_require_confirmation);
        assert!(settings.l2_automation);
        assert!(!settings.l2_whitelist_only);
        assert!(settings.l3_accessibility);
        assert!(!settings.l4_autonomous);
        assert_eq!(settings.l2_whitelist, vec!["Calculator".to_string()]);
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

    #[test]
    fn automation_allowed_enforces_l2_and_whitelist() {
        // Default: l2 off → nothing allowed.
        let mut p = PermissionSettings::default();
        assert!(!p.automation_allowed("open_app", "Calculator"));

        // l2 on, whitelist_only on, empty whitelist → still deny (default-deny).
        p.l2_automation = true;
        assert!(!p.automation_allowed("open_app", "Calculator"));

        // Whitelisted app matches exactly (case-insensitive); no loose prefix.
        p.l2_whitelist = vec!["Calculator".into(), "https://github.com".into()];
        assert!(p.automation_allowed("open_app", "calculator"));
        assert!(!p.automation_allowed("open_app", "CalculatorEvil"));

        // URL matches by prefix.
        assert!(p.automation_allowed("open_url", "https://github.com/anthropics"));
        assert!(!p.automation_allowed("open_url", "https://evil.example.com"));

        // whitelist_only off → anything allowed once l2 is on.
        p.l2_whitelist_only = false;
        assert!(p.automation_allowed("open_app", "AnythingGoes"));
    }

    #[test]
    fn app_name_strips_dot_app() {
        let p = PermissionSettings {
            l2_automation: true,
            l2_whitelist_only: true,
            l2_whitelist: vec!["Safari".into()],
            ..Default::default()
        };
        assert!(p.automation_allowed("open_app", "Safari.app"));
        assert!(p.automation_allowed("open_app", "safari"));
    }

    #[test]
    fn app_name_chinese_alias() {
        let p = PermissionSettings {
            l2_automation: true,
            l2_whitelist_only: true,
            l2_whitelist: vec!["Calculator".into()],
            ..Default::default()
        };
        // "计算器" is the macOS Chinese localised name for Calculator
        assert!(p.automation_allowed("open_app", "计算器"));
        // Reverse: whitelist "计算器" → allows "Calculator"
        let p2 = PermissionSettings {
            l2_automation: true,
            l2_whitelist_only: true,
            l2_whitelist: vec!["计算器".into()],
            ..Default::default()
        };
        assert!(p2.automation_allowed("open_app", "Calculator"));
    }

    #[test]
    fn app_name_abbreviation() {
        let p = PermissionSettings {
            l2_automation: true,
            l2_whitelist_only: true,
            l2_whitelist: vec!["Calc".into()],
            ..Default::default()
        };
        // "Calc" (4 chars ≤ 8) matches "Calculator" via abbreviation prefix
        assert!(p.automation_allowed("open_app", "Calculator"));
        // Exact self-match still works
        assert!(p.automation_allowed("open_app", "Calc"));
    }

    #[test]
    fn app_name_vscode_alias() {
        let p = PermissionSettings {
            l2_automation: true,
            l2_whitelist_only: true,
            l2_whitelist: vec!["Visual Studio Code".into()],
            ..Default::default()
        };
        assert!(p.automation_allowed("open_app", "Code"));
        assert!(p.automation_allowed("open_app", "vscode"));
    }
}
