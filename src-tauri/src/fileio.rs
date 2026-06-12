// ── L2 File I/O ────────────────────────────────────────────────────────
//
// Read text files (.md, .txt, .pdf) from the user's filesystem.
// Constrained to the user's home directory for security.
// PDF text extraction uses macOS `mdls` + `textutil` (no extra deps).

use std::path::{Path, PathBuf};

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

/// Tauri command: read a text file.
/// Supports .md, .txt, .pdf.  Paths must be under the user's home directory.
#[tauri::command]
pub fn read_file(path: String) -> Result<String, String> {
    let canonical = std::fs::canonicalize(&path).map_err(|e| format!("文件不存在: {e}"))?;

    // Security: only allow files under the user's home directory
    let home = home_dir().ok_or("无法确定用户目录")?;
    if !canonical.starts_with(&home) {
        return Err("安全限制：只能读取用户目录下的文件".into());
    }

    let ext = canonical
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "txt" | "md" | "json" | "csv" | "log" | "yaml" | "yml" | "toml" | "rs"
        | "ts" | "tsx" | "js" | "jsx" | "html" | "css" | "py" | "sh" => {
            std::fs::read_to_string(&canonical)
                .map_err(|e| format!("读取失败: {e}"))
        }
        "pdf" => read_pdf(&canonical),
        other => Err(format!("不支持的文件类型: .{other}")),
    }
}

/// List files in a directory (non-recursive, for browsing).
#[tauri::command]
pub fn list_directory(path: String) -> Result<String, String> {
    let canonical = std::fs::canonicalize(&path).map_err(|e| format!("目录不存在: {e}"))?;
    let home = home_dir().ok_or("无法确定用户目录")?;
    if !canonical.starts_with(&home) {
        return Err("安全限制：只能浏览用户目录".into());
    }
    if !canonical.is_dir() {
        return Err("不是目录".into());
    }

    let mut entries: Vec<String> = Vec::new();
    let rd = std::fs::read_dir(&canonical).map_err(|e| format!("读取目录失败: {e}"))?;
    for entry in rd {
        let entry = entry.map_err(|e| format!("遍历目录失败: {e}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let ft = entry.file_type().map_err(|e| format!("无法获取文件类型: {e}"))?;
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

/// Extract text from a PDF using macOS built-in tools.
#[cfg(target_os = "macos")]
fn read_pdf(path: &Path) -> Result<String, String> {
    // Use `mdls` to get page count, then `textutil` or `pdftotext` for extraction
    // Fallback: use python3 with PyPDF2 if available, or just return metadata
    let path_str = path.to_string_lossy().into_owned();

    // Try pdftotext first (from poppler, often pre-installed or via brew)
    if let Ok(output) = std::process::Command::new("pdftotext")
        .args(["-layout", path_str.as_str(), "-"])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).into_owned();
            if !text.trim().is_empty() {
                return Ok(text);
            }
        }
    }

    // Try textutil (macOS built-in, works for some PDFs)
    if let Ok(output) = std::process::Command::new("textutil")
        .args(["-convert", "txt", "-stdout", path_str.as_str()])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).into_owned();
            if !text.trim().is_empty() {
                return Ok(text);
            }
        }
    }

    // Try python3 with PyPDF2
    let py_script = format!(
        "import sys; sys.path.insert(0, ''); \
         from PyPDF2 import PdfReader; \
         r = PdfReader('{path_str}'); \
         print('\\n'.join(p.extract_text() or '' for p in r.pages))"
    );
    if let Ok(output) = std::process::Command::new("python3")
        .args(["-c", &py_script])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).into_owned();
            if !text.trim().is_empty() {
                return Ok(text);
            }
        }
    }

    // Last resort: return PDF metadata
    if let Ok(output) = std::process::Command::new("mdls")
        .args([&path_str])
        .output()
    {
        if output.status.success() {
            let meta = String::from_utf8_lossy(&output.stdout).into_owned();
            return Ok(format!("[PDF 元数据]\n{meta}\n\n(无法提取文本内容。安装 pdftotext: brew install poppler)"));
        }
    }

    Err("无法提取 PDF 文本。请安装 pdftotext（brew install poppler）或使用 PyPDF2。".into())
}

#[cfg(not(target_os = "macos"))]
fn read_pdf(_path: &Path) -> Result<String, String> {
    Err("PDF 阅读仅在 macOS 上可用".into())
}

