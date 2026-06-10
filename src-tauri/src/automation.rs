// ── L2 Automation Bridge (Phase 4) ─────────────────────────────────────
//
// Tauri commands that perform whitelisted L2 actions on the user's behalf:
// open an application, a URL, or a file / folder.
//
// SECURITY: the authoritative permission + whitelist check happens server-side
// (lingshu-server `execute_tool_call` → `PermissionSettings::automation_allowed`).
// These commands only execute actions the backend already approved and streamed
// down to the frontend. They shell out to macOS `open(1)` via an absolute path.

/// Open a macOS application by name (e.g. "Calculator", "Safari").
#[tauri::command]
pub fn open_application(name: String) -> Result<(), String> {
    run_open(&["-a", name.as_str()])
}

/// Open a URL in the user's default browser. Re-checks the scheme as defence in
/// depth (the backend only ever emits http/https).
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(format!("refusing to open non-http(s) URL: {url}"));
    }
    run_open(&[url.as_str()])
}

/// Open a local file or folder with its default application.
#[tauri::command]
pub fn open_path(path: String) -> Result<(), String> {
    run_open(&[path.as_str()])
}

#[cfg(target_os = "macos")]
fn run_open(args: &[&str]) -> Result<(), String> {
    use std::process::Command;
    let status = Command::new("/usr/bin/open")
        .args(args)
        .status()
        .map_err(|e| format!("failed to launch `open`: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`open` exited with status {status}"))
    }
}

#[cfg(not(target_os = "macos"))]
fn run_open(_args: &[&str]) -> Result<(), String> {
    Err("L2 automation is only supported on macOS".into())
}
