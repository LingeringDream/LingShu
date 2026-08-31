// ── L2 Automation Bridge (Phase 4) ─────────────────────────────────────
//
// Tauri commands that perform whitelisted L2 actions on the user's behalf:
// open an application, a URL, or a file / folder.
//
// SECURITY: the authoritative permission + whitelist check happens server-side
// (lingshu-server `execute_tool_call` → `PermissionSettings::automation_allowed`).
// These commands only execute actions the backend already approved and streamed
// down to the frontend. URL and path opening use Tauri's cross-platform opener;
// application-by-name remains a macOS adapter until Windows UI automation lands.

use tauri_plugin_opener::OpenerExt;

/// Open a macOS application by name (e.g. "Calculator", "Safari").
/// Tries multiple name variants: bare, with `.app`, and lookup via `mdfind`.
#[tauri::command]
pub fn open_application(name: String) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("empty app name".into());
    }

    // 1. Try as-is: `open -a "Google Chrome"`
    if run_open_status(&["-a", name]) == 0 {
        return Ok(());
    }

    // 2. Try with `.app` suffix: `open -a "Google Chrome.app"`
    let with_dot_app = format!("{name}.app");
    if run_open_status(&["-a", &with_dot_app]) == 0 {
        return Ok(());
    }

    // 3. Search /Applications and ~/Applications via `mdfind`
    if let Ok(found) = find_app_path(name) {
        if run_open_status(&[&found]) == 0 {
            return Ok(());
        }
    }

    Err(format!(
        "找不到应用程序「{name}」。请确认 App 名称拼写正确，或将 App 拖入 /Applications 目录。"
    ))
}

#[cfg(target_os = "macos")]
fn run_open_status(args: &[&str]) -> i32 {
    std::process::Command::new("/usr/bin/open")
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.code().unwrap_or(-1))
        .unwrap_or(-1)
}

#[cfg(target_os = "macos")]
fn find_app_path(name: &str) -> Result<String, ()> {
    // 1. Spotlight search — single query string
    let query = format!("kMDItemDisplayName == '{name}'");
    if let Ok(output) = std::process::Command::new("/usr/bin/mdfind")
        .arg(&query)
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
            return Ok(line.to_string());
        }
    }

    // 2. Check common locations
    for dir in &["/Applications", "/System/Applications"] {
        // Try name.app and name
        for candidate in &[format!("{name}.app"), name.to_string()] {
            let path = format!("{dir}/{candidate}");
            if std::path::Path::new(&path).exists() {
                return Ok(path);
            }
        }
    }

    Err(())
}

/// Open a URL in the user's default browser. Re-checks the scheme as defence in
/// depth (the backend only ever emits http/https).
#[tauri::command]
pub fn open_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(format!("refusing to open non-http(s) URL: {url}"));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("failed to open URL: {e}"))
}

/// Open a local file or folder with its default application.
#[tauri::command]
pub fn open_path(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let canonical =
        std::fs::canonicalize(&path).map_err(|e| format!("path does not exist: {e}"))?;
    app.opener()
        .open_path(canonical.display().to_string(), None::<&str>)
        .map_err(|e| format!("failed to open path: {e}"))
}

#[cfg(not(target_os = "macos"))]
fn run_open_status(_args: &[&str]) -> i32 {
    -1
}

#[cfg(not(target_os = "macos"))]
fn find_app_path(_name: &str) -> Result<String, ()> {
    Err(())
}
