use std::path::{Path, PathBuf};

const MAX_TEXT_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_PDF_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_RESULT_CHARS: usize = 8_000;

fn user_home_dir() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
        .or_else(|| {
            let drive = std::env::var_os("HOMEDRIVE")?;
            let path = std::env::var_os("HOMEPATH")?;
            let mut home = PathBuf::from(drive);
            home.push(path);
            Some(home)
        })
        .ok_or_else(|| "无法确定用户主目录".to_string())
}

fn canonical_user_path(path: &str, home: &Path) -> Result<PathBuf, String> {
    let canonical =
        std::fs::canonicalize(path).map_err(|e| format!("目标不存在或无法访问: {e}"))?;
    let canonical_home =
        std::fs::canonicalize(home).map_err(|e| format!("无法读取用户主目录: {e}"))?;
    if !canonical.starts_with(&canonical_home) {
        return Err("安全限制：只能访问用户主目录下的文件".into());
    }
    Ok(canonical)
}

fn ensure_file(path: &Path, max_bytes: u64) -> Result<(), String> {
    let metadata = std::fs::metadata(path).map_err(|e| format!("无法读取文件信息: {e}"))?;
    if !metadata.is_file() {
        return Err("目标不是文件".into());
    }
    if metadata.len() > max_bytes {
        return Err(format!("文件过大：最大支持 {} MB", max_bytes / 1024 / 1024));
    }
    Ok(())
}

fn truncate_for_model(content: &str) -> String {
    let char_count = content.chars().count();
    if char_count <= MAX_RESULT_CHARS {
        return content.to_string();
    }
    let prefix: String = content.chars().take(MAX_RESULT_CHARS).collect();
    format!("{prefix}...\n\n(内容已截断，共 {char_count} 字符)")
}

fn read_file_under(path: &str, home: &Path) -> Result<String, String> {
    let canonical = canonical_user_path(path, home)?;
    let extension = canonical
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_lowercase();

    let content = match extension.as_str() {
        "txt" | "md" | "json" | "csv" | "log" | "yaml" | "yml" | "toml" | "rs" | "ts" | "tsx"
        | "js" | "jsx" | "html" | "css" | "py" | "sh" => {
            ensure_file(&canonical, MAX_TEXT_FILE_BYTES)?;
            std::fs::read_to_string(&canonical).map_err(|e| format!("读取失败: {e}"))?
        }
        "pdf" => {
            ensure_file(&canonical, MAX_PDF_FILE_BYTES)?;
            let text = pdf_extract::extract_text(&canonical)
                .map_err(|e| format!("PDF文本提取失败: {e}"))?;
            if text.trim().is_empty() {
                return Err("PDF中没有可提取的文本；扫描件需要OCR后再读取".into());
            }
            text
        }
        _ => {
            return Err(format!(
                "不支持的文件类型: .{extension}（支持 .md .txt .pdf .json .csv 等）"
            ))
        }
    };

    Ok(format!(
        "[文件] {}\n---\n{}",
        canonical.display(),
        truncate_for_model(&content)
    ))
}

pub fn read_file(path: &str) -> Result<String, String> {
    read_file_under(path, &user_home_dir()?)
}

fn list_directory_under(path: &str, home: &Path) -> Result<String, String> {
    let canonical = canonical_user_path(path, home)?;
    if !canonical.is_dir() {
        return Err("目标不是目录".into());
    }

    let mut entries = Vec::new();
    let directory = std::fs::read_dir(&canonical).map_err(|e| format!("读取目录失败: {e}"))?;
    for entry in directory {
        let entry = entry.map_err(|e| format!("遍历目录失败: {e}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let file_type = entry
            .file_type()
            .map_err(|e| format!("无法获取文件类型: {e}"))?;
        entries.push(format!(
            "{} {name}",
            if file_type.is_dir() { "📁" } else { "📄" }
        ));
    }
    entries.sort();

    Ok(format!(
        "{}\n---\n{}",
        canonical.display(),
        entries.join("\n")
    ))
}

pub fn list_directory(path: &str) -> Result<String, String> {
    list_directory_under(path, &user_home_dir()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "lingshu-server-file-tools-{label}-{}",
            std::process::id()
        ))
    }

    #[test]
    fn rejects_paths_outside_the_canonical_home() {
        let root = test_root("outside");
        let home = root.join("home");
        let outside = root.join("outside.txt");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(&outside, "outside").unwrap();

        let result = read_file_under(outside.to_str().unwrap(), &home);
        assert!(result.unwrap_err().contains("只能访问用户主目录"));

        std::fs::remove_file(outside).unwrap();
        std::fs::remove_dir(home).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn truncates_unicode_on_character_boundaries() {
        let content = "灵".repeat(MAX_RESULT_CHARS + 3);
        let truncated = truncate_for_model(&content);
        assert!(truncated.starts_with(&"灵".repeat(MAX_RESULT_CHARS)));
        assert!(truncated.contains("共 8003 字符"));
    }

    #[test]
    fn extracts_text_from_a_real_pdf() {
        use pdf_extract::{dictionary, Document, Object, Stream};

        let root = test_root("pdf");
        let home = root.join("home");
        let path = home.join("sample.pdf");
        std::fs::create_dir_all(&home).unwrap();

        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let font_id = document.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Courier",
        });
        let resources_id = document.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });
        let content_id = document.add_object(Stream::new(
            dictionary! {},
            b"BT /F1 12 Tf 72 720 Td (Hello LingShu Sidecar PDF) Tj ET".to_vec(),
        ));
        let page_id = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
        });
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
                "Resources" => resources_id,
                "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            }),
        );
        let catalog_id = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        document.trailer.set("Root", catalog_id);
        document.save(&path).unwrap();

        let result = read_file_under(path.to_str().unwrap(), &home).unwrap();
        assert!(result.contains("Hello LingShu Sidecar PDF"));

        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(home).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
