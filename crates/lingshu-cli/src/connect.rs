use crate::config::{self, Paths};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn connect(
    paths: &Paths,
    tool: &str,
    override_path: Option<PathBuf>,
    print: bool,
) -> Result<()> {
    let path = match override_path {
        Some(path) => std::path::absolute(path)?,
        None => default_path(tool)?,
    };
    let entry = json!({ "command": paths.launcher()?, "args": ["mcp"], "env": { "LINGSHU_HOME": paths.home } });
    if print {
        println!("{}", serde_json::to_string_pretty(&entry)?);
        return Ok(());
    }
    let parent = path.parent().context("config path has no parent")?;
    fs::create_dir_all(parent)?;
    let lock_path = parent.join(format!(
        ".{}.lingshu.lock",
        path.file_name()
            .context("config path has no filename")?
            .to_string_lossy()
    ));
    let _lock = config::lock(&lock_path, false)?;
    let original = if path.exists() {
        Some(fs::read_to_string(&path).context("read existing AI-tool config")?)
    } else {
        None
    };
    let updated = merge(
        tool,
        original
            .as_deref()
            .unwrap_or(if tool == "codex" { "" } else { "{}" }),
        &entry,
    )?;
    if let Some(original) = original {
        let backup = parent.join(format!(
            "{}.lingshu-backup-{}",
            path.file_name().unwrap().to_string_lossy(),
            uuid::Uuid::new_v4()
        ));
        config::atomic_write(&backup, original.as_bytes())?;
        println!("Backup: {}", backup.display());
    }
    config::atomic_write(&path, updated.as_bytes())?;
    println!(
        "Connected {tool}: {}\nRestart the AI tool to load LingShu MCP.",
        path.display()
    );
    Ok(())
}

fn default_path(tool: &str) -> Result<PathBuf> {
    let home = config::user_home()?;
    match tool {
        "cursor" => Ok(home.join(".cursor/mcp.json")),
        "codex" => Ok(std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or(home.join(".codex"))
            .join("config.toml")),
        "claude" if cfg!(target_os = "macos") => {
            Ok(home.join("Library/Application Support/Claude/claude_desktop_config.json"))
        }
        "claude" if cfg!(windows) => Ok(PathBuf::from(
            std::env::var_os("APPDATA").context("APPDATA is unavailable")?,
        )
        .join("Claude/claude_desktop_config.json")),
        "claude" => Ok(std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or(home.join(".config"))
            .join("Claude/claude_desktop_config.json")),
        _ => anyhow::bail!("supported tools: cursor, claude, codex"),
    }
}

fn merge(tool: &str, original: &str, entry: &Value) -> Result<String> {
    if tool == "codex" {
        use toml_edit::{value, Array, DocumentMut, Item, Table};
        let mut document: DocumentMut = original
            .parse()
            .context("invalid Codex TOML; config was not changed")?;
        if !document.contains_key("mcp_servers") {
            document["mcp_servers"] = Item::Table(Table::new());
        }
        let servers = document["mcp_servers"]
            .as_table_mut()
            .context("mcp_servers must be a TOML table")?;
        let mut server = Table::new();
        server["command"] = value(entry["command"].as_str().context("invalid command path")?);
        let mut args = Array::new();
        args.push("mcp");
        server["args"] = value(args);
        let mut environment = Table::new();
        environment["LINGSHU_HOME"] = value(
            entry["env"]["LINGSHU_HOME"]
                .as_str()
                .context("invalid data path")?,
        );
        server["env"] = Item::Table(environment);
        servers["lingshu-memory"] = Item::Table(server);
        return Ok(document.to_string());
    }
    let mut document: Value =
        serde_json::from_str(original).context("invalid JSON; config was not changed")?;
    let object = document
        .as_object_mut()
        .context("AI-tool config must be a JSON object")?;
    let servers = object
        .entry("mcpServers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .context("mcpServers must be an object; config was not changed")?;
    servers.insert("lingshu-memory".into(), entry.clone());
    Ok(serde_json::to_string_pretty(&document)? + "\n")
}

pub fn validate_purge(directory: &Path) -> Result<()> {
    let config: Value = serde_json::from_slice(&fs::read(directory.join("config.json"))?)?;
    anyhow::ensure!(
        config["application"] == config::APPLICATION,
        "refusing to purge an unrecognized data directory"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry() -> Value {
        json!({"command":"/a path/lingshu", "args":["mcp"],"env":{"LINGSHU_HOME":"/private data"}})
    }
    #[test]
    fn json_preserves_other_servers_and_rejects_broken_config() {
        let output: Value = serde_json::from_str(
            &merge(
                "cursor",
                r#"{"settings":42,"mcpServers":{"other":{"command":"other"}}}"#,
                &entry(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(output["settings"], 42);
        assert_eq!(output["mcpServers"]["other"]["command"], "other");
        assert!(merge("cursor", "broken", &entry()).is_err());
        assert!(merge("cursor", r#"{"mcpServers":[]}"#, &entry()).is_err());
    }
    #[test]
    fn codex_preserves_comments_model_and_other_servers() {
        let source = "# personal\nmodel = \"my-model\"\n[mcp_servers.other]\ncommand = \"other\"\n";
        let output = merge("codex", source, &entry()).unwrap();
        assert!(output.contains("# personal"));
        assert!(output.contains("my-model"));
        assert!(output.contains("[mcp_servers.other]"));
        let parsed: toml_edit::DocumentMut = output.parse().unwrap();
        assert_eq!(
            parsed["mcp_servers"]["lingshu-memory"]["command"].as_str(),
            Some("/a path/lingshu")
        );
        assert!(merge("codex", "mcp_servers = 42", &entry()).is_err());
    }
    #[test]
    fn malformed_file_is_not_overwritten() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.json");
        fs::write(&path, "invalid JSON").unwrap();
        let paths = Paths {
            home: temp.path().into(),
            binaries: temp.path().into(),
        };
        assert!(connect(&paths, "cursor", Some(path.clone()), false).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "invalid JSON");
    }
}
