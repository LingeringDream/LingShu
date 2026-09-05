//! Self-contained PostgreSQL runtime for the packaged desktop sidecar.
//!
//! The compressed PostgreSQL runtime is embedded into the `lingshu-server`
//! binary at build time (feature `desktop-embedded-db`). On first launch it is
//! extracted and initialized under the user's application-data directory, then
//! reused on later launches — end users never install or configure a database.
//!
//! Supported on Windows, macOS and Linux; see [`application_data_base`] for
//! the per-platform location.

use anyhow::Context;
use postgresql_archive::{ExactVersion, VersionReq};
use postgresql_embedded::{PostgreSQL, Settings, Status};
use std::path::{Path, PathBuf};

const DATABASE_NAME: &str = "lingshu";
const DATABASE_PORT: u16 = 55_432;
const DATABASE_USER: &str = "postgres";
const DATABASE_PASSWORD: &str = "lingshu-desktop-local";
const POSTGRESQL_VERSION: &str = "=17.7.0";
const EMBEDDED_ARCHIVE: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/embedded-postgresql.jar"));

pub struct EmbeddedDatabase {
    postgresql: PostgreSQL,
}

impl EmbeddedDatabase {
    pub async fn start() -> anyhow::Result<Self> {
        let root = application_data_root()?;
        std::fs::create_dir_all(&root)
            .with_context(|| format!("create embedded database directory {}", root.display()))?;

        let version = VersionReq::parse(POSTGRESQL_VERSION)?;
        let exact_version = version
            .exact_version()
            .context("embedded PostgreSQL version must be exact")?;
        let installation_dir = root.join("runtime").join(exact_version.to_string());

        if !embedded_runtime_is_complete(&installation_dir) {
            if installation_dir.exists() {
                tracing::warn!(
                    path = %installation_dir.display(),
                    "Removing incomplete embedded PostgreSQL runtime before re-extracting"
                );
                std::fs::remove_dir_all(&installation_dir).with_context(|| {
                    format!(
                        "remove incomplete embedded PostgreSQL runtime {}",
                        installation_dir.display()
                    )
                })?;
            }
            tracing::info!(
                path = %installation_dir.display(),
                "Extracting bundled PostgreSQL runtime (first launch only)"
            );
            let archive = EMBEDDED_ARCHIVE.to_vec();
            postgresql_archive::extract(
                postgresql_archive::configuration::zonky::URL,
                &archive,
                &installation_dir,
            )
            .await
            .context("extract bundled PostgreSQL runtime")?;
        }

        let data_dir = root.join("data");
        remove_stale_postmaster_pid(&installation_dir, &data_dir)?;

        let settings = Settings {
            version,
            installation_dir,
            password_file: root.join("postgresql.password"),
            data_dir,
            host: "127.0.0.1".to_string(),
            port: DATABASE_PORT,
            username: DATABASE_USER.to_string(),
            password: DATABASE_PASSWORD.to_string(),
            temporary: false,
            timeout: Some(std::time::Duration::from_secs(30)),
            trust_installation_dir: true,
            ..Settings::default()
        };

        let mut postgresql = PostgreSQL::new(settings);
        postgresql
            .setup()
            .await
            .context("initialize embedded PostgreSQL")?;
        if postgresql.status() != Status::Started {
            postgresql
                .start()
                .await
                .context("start embedded PostgreSQL")?;
        }
        if !postgresql.database_exists(DATABASE_NAME).await? {
            postgresql
                .create_database(DATABASE_NAME)
                .await
                .context("create LingShu desktop database")?;
        }

        tracing::info!(port = DATABASE_PORT, "Embedded PostgreSQL ready");
        Ok(Self { postgresql })
    }

    pub fn database_url(&self) -> String {
        self.postgresql.settings().url(DATABASE_NAME)
    }

    pub async fn stop(&self) {
        if self.postgresql.status() == Status::Started {
            if let Err(error) = self.postgresql.stop().await {
                tracing::warn!(%error, "Failed to stop embedded PostgreSQL cleanly");
            }
        }
    }
}

fn pg_ctl_path(installation_dir: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    const PG_CTL: &str = "pg_ctl.exe";
    #[cfg(not(target_os = "windows"))]
    const PG_CTL: &str = "pg_ctl";

    installation_dir.join("bin").join(PG_CTL)
}

#[cfg(target_os = "windows")]
const EMBEDDED_BINARY_SUFFIX: &str = ".exe";
#[cfg(not(target_os = "windows"))]
const EMBEDDED_BINARY_SUFFIX: &str = "";

fn embedded_runtime_is_complete(installation_dir: &Path) -> bool {
    ["postgres", "pg_ctl", "initdb"].iter().all(|binary| {
        installation_dir
            .join("bin")
            .join(format!("{binary}{EMBEDDED_BINARY_SUFFIX}"))
            .is_file()
    })
}

/// `postgresql_embedded::PostgreSQL::status()` considers the database started
/// whenever `postmaster.pid` exists. Windows can leave that file behind after
/// a crash or forced shutdown even though the process is gone. Ask PostgreSQL's
/// own `pg_ctl status` before trusting the file and remove it only when pg_ctl
/// confirms that no server owns this data directory.
fn remove_stale_postmaster_pid(installation_dir: &Path, data_dir: &Path) -> anyhow::Result<()> {
    let pid_file = data_dir.join("postmaster.pid");
    if !pid_file.exists() {
        return Ok(());
    }

    let output = std::process::Command::new(pg_ctl_path(installation_dir))
        .args(["status", "-D"])
        .arg(data_dir)
        .output()
        .context("check embedded PostgreSQL process status")?;

    match output.status.code() {
        Some(0) => return Ok(()),
        // pg_ctl documents exit code 3 as "server is not running". Only this
        // result authorizes deleting the PID file; other failures may be a
        // permission or runtime problem and must leave user data untouched.
        Some(3) => {}
        code => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            anyhow::bail!("could not verify embedded PostgreSQL status (exit {code:?}): {stderr}");
        }
    }

    std::fs::remove_file(&pid_file)
        .with_context(|| format!("remove stale PostgreSQL pid file {}", pid_file.display()))?;
    tracing::warn!(path = %pid_file.display(), "Removed stale embedded PostgreSQL pid file");
    Ok(())
}

/// Root of the bundled PostgreSQL runtime and its data directory.
///
/// A packaged desktop app has no writable install directory and its working
/// directory is undefined (a double-clicked `.app` inherits `/`), so the
/// location must be derived from the platform's per-user data directory —
/// never from the current directory.
fn application_data_root() -> anyhow::Result<PathBuf> {
    Ok(application_data_base()?
        .join(Path::new("com.lingshu.desktop"))
        .join("embedded-postgresql"))
}

/// Per-user application-data directory for the current platform.
///
/// * Windows: `%APPDATA%` (falling back to `%LOCALAPPDATA%`)
/// * macOS:   `~/Library/Application Support`
/// * Linux/other: `$XDG_DATA_HOME`, else `~/.local/share`
#[cfg(target_os = "windows")]
fn application_data_base() -> anyhow::Result<PathBuf> {
    std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("LOCALAPPDATA"))
        .map(PathBuf::from)
        .context("neither APPDATA nor LOCALAPPDATA is set; cannot locate the embedded database directory")
}

#[cfg(target_os = "macos")]
fn application_data_base() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set; cannot locate the embedded database directory")?;
    Ok(home.join("Library").join("Application Support"))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn application_data_base() -> anyhow::Result<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set; cannot locate the embedded database directory")?;
    Ok(home.join(".local").join("share"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A packaged app is launched by double-clicking, so its working directory
    /// is "/". The embedded database must never be placed relative to it.
    #[test]
    fn application_data_root_is_not_cwd_relative() {
        let root = application_data_root().expect("application data root resolves");
        assert!(root.is_absolute(), "root must be absolute: {}", root.display());
        assert!(
            root.ends_with("com.lingshu.desktop/embedded-postgresql"),
            "unexpected root layout: {}",
            root.display()
        );
    }

    #[test]
    fn pg_ctl_path_uses_platform_binary_name() {
        let path = pg_ctl_path(Path::new("runtime"));
        #[cfg(target_os = "windows")]
        assert_eq!(path, Path::new("runtime").join("bin").join("pg_ctl.exe"));
        #[cfg(not(target_os = "windows"))]
        assert_eq!(path, Path::new("runtime").join("bin").join("pg_ctl"));
    }
}
