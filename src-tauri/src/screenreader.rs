// ── L3 Screen Reading (macOS Accessibility) ─────────────────────────────
//
// Reads text from the frontmost window via AppleScript + System Events.
// Requires Accessibility permission (System Settings → Privacy → Accessibility).

/// Tauri command: read visible text from the frontmost application window.
#[tauri::command]
pub fn read_screen() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        // AppleScript: get frontmost app name + selected text + window content
        let script = r#"
tell application "System Events"
    set frontApp to first application process whose frontmost is true
    set appName to name of frontApp
    set winTitle to ""

    try
        set winTitle to title of front window of frontApp
    end try

    -- Try to get selected text from the focused UI element
    set selectedText to ""
    try
        set focusedElem to focused UI element of front window of frontApp
        set selectedText to value of focusedElem
        if selectedText is missing value then set selectedText to ""
    end try

    -- If no selected text, try the entire value
    if selectedText is "" then
        try
            set selectedText to entire value of focused UI element of front window of frontApp
        end try
    end if

    return appName & "|||" & winTitle & "|||" & selectedText
end tell
"#;

        let output = std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()
            .map_err(|e| format!("无法运行 osascript: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("not allowed") || stderr.contains("assistive")
                || stderr.contains("-25211")
            {
                return Err(
                    "辅助功能权限未开启。请在 系统设置 → 隐私与安全性 → 辅助功能 中允许灵枢（或 Terminal / osascript）。"
                        .into(),
                );
            }
            return Err(format!("屏幕阅读失败: {stderr}"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            return Ok("(前台窗口无可读文本)".into());
        }

        // Parse: appName|||winTitle|||text
        let parts: Vec<&str> = stdout.splitn(3, "|||").collect();
        let app_name = parts.first().unwrap_or(&"");
        let win_title = parts.get(1).unwrap_or(&"");
        let text = parts.get(2).unwrap_or(&"");

        let mut result = String::new();
        if !app_name.is_empty() {
            result.push_str(&format!("[前台应用] {app_name}\n"));
        }
        if !win_title.is_empty() {
            result.push_str(&format!("[窗口标题] {win_title}\n"));
        }
        if !text.is_empty() {
            result.push_str(&format!("[内容]\n{text}"));
        } else {
            result.push_str("[内容]\n(该窗口无可读文本元素)");
        }

        Ok(result)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = ();
        Err("屏幕阅读仅在 macOS 上可用".into())
    }
}

/// Request Accessibility permission: open System Settings directly.
#[tauri::command]
pub fn request_accessibility_permission() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }
        if unsafe { AXIsProcessTrusted() } {
            return Ok(true);
        }
        // Open Accessibility settings pane
        let _ = std::process::Command::new("open")
            .args(["x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"])
            .status();
        Ok(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = ();
        Err("仅 macOS 支持".into())
    }
}
