use std::path::{Path, PathBuf};

use tauri::Manager;

const MAX_ICS_BYTES: usize = 256 * 1024;

fn sanitize_calendar_file_name(file_name: &str) -> String {
    let stem = file_name
        .trim()
        .trim_end_matches(".ics")
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || matches!(ch, '-' | '_' | ' ') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches(|ch: char| ch == '_' || ch.is_whitespace())
        .to_string();
    let stem = if stem.is_empty() {
        "lingshu-calendar".to_string()
    } else {
        stem
    };
    format!("{stem}.ics")
}

fn validate_calendar_content(content: &str) -> Result<(), String> {
    if content.len() > MAX_ICS_BYTES {
        return Err("日历文件过大".into());
    }
    if !content.starts_with("BEGIN:VCALENDAR\r\n") || !content.trim_end().ends_with("END:VCALENDAR")
    {
        return Err("日历文件格式无效".into());
    }
    Ok(())
}

fn available_download_path(download_dir: &Path, file_name: &str) -> PathBuf {
    let preferred = download_dir.join(file_name);
    if !preferred.exists() {
        return preferred;
    }

    let stem = file_name.trim_end_matches(".ics");
    for suffix in 1..=999 {
        let candidate = download_dir.join(format!("{stem}-{suffix}.ics"));
        if !candidate.exists() {
            return candidate;
        }
    }

    download_dir.join(format!("{stem}-{}.ics", std::process::id()))
}

/// Save a frontend-generated RFC 5545 calendar file into the current user's
/// Downloads directory. Content and filename are constrained so this command
/// cannot be used as an arbitrary filesystem write bridge.
#[tauri::command]
pub fn export_calendar_file(
    app: tauri::AppHandle,
    file_name: String,
    content: String,
) -> Result<String, String> {
    validate_calendar_content(&content)?;
    let download_dir = app
        .path()
        .download_dir()
        .map_err(|e| format!("无法定位下载目录：{e}"))?;
    std::fs::create_dir_all(&download_dir).map_err(|e| format!("无法创建下载目录：{e}"))?;

    let safe_name = sanitize_calendar_file_name(&file_name);
    let path = available_download_path(&download_dir, &safe_name);
    std::fs::write(&path, content.as_bytes()).map_err(|e| format!("日历文件写入失败：{e}"))?;
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_file_name_cannot_escape_downloads() {
        assert_eq!(
            sanitize_calendar_file_name("../../校赛/答辩.ics"),
            "校赛_答辩.ics"
        );
        assert_eq!(sanitize_calendar_file_name(".ics"), "lingshu-calendar.ics");
    }

    #[test]
    fn calendar_content_requires_a_calendar_envelope() {
        assert!(validate_calendar_content("not a calendar").is_err());
        assert!(
            validate_calendar_content("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nEND:VCALENDAR\r\n")
                .is_ok()
        );
    }

    #[test]
    fn download_path_avoids_overwriting_existing_files() {
        let temp = std::env::temp_dir().join(format!(
            "lingshu-calendar-export-test-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        std::fs::create_dir_all(&temp).unwrap();
        let original = temp.join("答辩.ics");
        std::fs::write(&original, b"existing").unwrap();

        assert_eq!(
            available_download_path(&temp, "答辩.ics"),
            temp.join("答辩-1.ics")
        );

        std::fs::remove_file(original).unwrap();
        std::fs::remove_dir(temp).unwrap();
    }
}
