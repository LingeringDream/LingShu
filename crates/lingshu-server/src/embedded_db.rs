//! Self-contained PostgreSQL runtime for the packaged desktop sidecar.
//!
//! The compressed Windows runtime is embedded into `lingshu-server.exe` at
//! build time. On first launch it is extracted and initialized under the
//! user's application-data directory, then reused on later launches.

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

        if !installation_dir.exists() {
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

fn application_data_root() -> anyhow::Result<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .unwrap_or(std::env::current_dir()?);
    Ok(base
        .join(Path::new("com.lingshu.desktop"))
        .join("embedded-postgresql"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pg_ctl_path_uses_platform_binary_name() {
        let path = pg_ctl_path(Path::new("runtime"));
        #[cfg(target_os = "windows")]
        assert_eq!(path, Path::new("runtime").join("bin").join("pg_ctl.exe"));
        #[cfg(not(target_os = "windows"))]
        assert_eq!(path, Path::new("runtime").join("bin").join("pg_ctl"));
    }
}
