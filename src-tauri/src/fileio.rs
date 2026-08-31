// ── L2 File I/O ────────────────────────────────────────────────────────
//
// Read text files (.md, .txt, .pdf) from the user's filesystem.
// Constrained to the user's home directory for security.
// PDF text extraction is pure Rust so packaged builds do not depend on
// pdftotext, textutil, Python, or other host-installed tools.

use std::path::{Path, PathBuf};
use tauri::Manager;

const MAX_TEXT_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_PDF_FILE_BYTES: u64 = 64 * 1024 * 1024;

fn canonical_user_path(path: &str, home: &Path) -> Result<PathBuf, String> {
    let canonical = std::fs::canonicalize(path).map_err(|e| format!("文件不存在: {e}"))?;
    let canonical_home =
        std::fs::canonicalize(home).map_err(|e| format!("无法读取用户目录: {e}"))?;
    if !canonical.starts_with(&canonical_home) {
        return Err("安全限制：只能访问用户目录下的文件".into());
    }
    Ok(canonical)
}

fn ensure_file_size(path: &Path, max_bytes: u64) -> Result<(), String> {
    let metadata = std::fs::metadata(path).map_err(|e| format!("无法读取文件信息: {e}"))?;
    if metadata.len() > max_bytes {
        return Err(format!("文件过大：最大支持 {} MB", max_bytes / 1024 / 1024));
    }
    Ok(())
}

/// Tauri command: read a text file.
/// Supports .md, .txt, .pdf.  Paths must be under the user's home directory.
#[tauri::command]
pub fn read_file(app: tauri::AppHandle, path: String) -> Result<String, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|e| format!("无法确定用户目录: {e}"))?;
    let canonical = canonical_user_path(&path, &home)?;

    let ext = canonical
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "txt" | "md" | "json" | "csv" | "log" | "yaml" | "yml" | "toml" | "rs" | "ts" | "tsx"
        | "js" | "jsx" | "html" | "css" | "py" | "sh" => {
            ensure_file_size(&canonical, MAX_TEXT_FILE_BYTES)?;
            std::fs::read_to_string(&canonical).map_err(|e| format!("读取失败: {e}"))
        }
        "pdf" => {
            ensure_file_size(&canonical, MAX_PDF_FILE_BYTES)?;
            read_pdf(&canonical)
        }
        other => Err(format!("不支持的文件类型: .{other}")),
    }
}

/// List files in a directory (non-recursive, for browsing).
#[tauri::command]
pub fn list_directory(app: tauri::AppHandle, path: String) -> Result<String, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|e| format!("无法确定用户目录: {e}"))?;
    let canonical =
        canonical_user_path(&path, &home).map_err(|e| e.replace("文件不存在", "目录不存在"))?;
    if !canonical.is_dir() {
        return Err("不是目录".into());
    }

    let mut entries: Vec<String> = Vec::new();
    let rd = std::fs::read_dir(&canonical).map_err(|e| format!("读取目录失败: {e}"))?;
    for entry in rd {
        let entry = entry.map_err(|e| format!("遍历目录失败: {e}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let ft = entry
            .file_type()
            .map_err(|e| format!("无法获取文件类型: {e}"))?;
        let prefix = if ft.is_dir() { "📁" } else { "📄" };
        if !name.starts_with('.') {
            entries.push(format!("{prefix} {name}"));
        }
    }
    entries.sort();
    Ok(format!(
        "{}\n---\n{}",
        canonical.display(),
        entries.join("\n")
    ))
}

/// Extract embedded text from a PDF using a bundled pure-Rust parser.
fn read_pdf(path: &Path) -> Result<String, String> {
    let text = pdf_extract::extract_text(path).map_err(|e| format!("PDF文本提取失败: {e}"))?;
    if text.trim().is_empty() {
        return Err("PDF中没有可提取的文本；扫描件需要OCR后再读取".into());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_path_rejects_files_outside_home() {
        let temp = std::env::temp_dir().join(format!("lingshu-fileio-{}", std::process::id()));
        let home = temp.join("home");
        let outside = temp.join("outside.txt");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(&outside, "outside").unwrap();

        assert!(canonical_user_path(outside.to_str().unwrap(), &home).is_err());

        std::fs::remove_file(outside).unwrap();
        std::fs::remove_dir(home).unwrap();
        std::fs::remove_dir(temp).unwrap();
    }

    #[test]
    fn user_path_accepts_files_inside_home() {
        let temp =
            std::env::temp_dir().join(format!("lingshu-fileio-inside-{}", std::process::id()));
        let home = temp.join("home");
        let inside = home.join("note.txt");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(&inside, "inside").unwrap();

        assert_eq!(
            canonical_user_path(inside.to_str().unwrap(), &home).unwrap(),
            std::fs::canonicalize(&inside).unwrap()
        );

        std::fs::remove_file(inside).unwrap();
        std::fs::remove_dir(home).unwrap();
        std::fs::remove_dir(temp).unwrap();
    }

    #[test]
    fn pure_rust_pdf_reader_extracts_embedded_text() {
        use pdf_extract::{dictionary, Document, Object, Stream};

        let path =
            std::env::temp_dir().join(format!("lingshu-pdf-extract-{}.pdf", std::process::id()));
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
            b"BT /F1 12 Tf 72 720 Td (Hello LingShu PDF) Tj ET".to_vec(),
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

        let extracted = read_pdf(&path).unwrap();
        assert!(extracted.contains("Hello LingShu PDF"));

        std::fs::remove_file(path).unwrap();
    }
}
