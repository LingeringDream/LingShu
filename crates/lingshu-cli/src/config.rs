use anyhow::{Context, Result};
use fs2::FileExt;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

pub const APPLICATION: &str = "lingshu-cli";

#[derive(Clone)]
pub struct Paths {
    pub home: PathBuf,
    pub binaries: PathBuf,
}

impl Paths {
    pub fn load() -> Result<Self> {
        let home = match std::env::var_os("LINGSHU_HOME") {
            Some(value) => {
                anyhow::ensure!(!value.is_empty(), "LINGSHU_HOME cannot be empty");
                PathBuf::from(value)
            }
            None => default_home()?,
        };
        let binaries = executable_directory(&std::env::current_exe()?)?;
        let home = std::path::absolute(home)?;
        let mut ancestor = home.clone();
        let mut suffix = Vec::new();
        while !ancestor.exists() {
            suffix.push(
                ancestor
                    .file_name()
                    .context("invalid data directory")?
                    .to_owned(),
            );
            ancestor = ancestor
                .parent()
                .context("invalid data directory")?
                .to_owned();
        }
        let mut resolved = fs::canonicalize(&ancestor)?;
        for component in suffix.into_iter().rev() {
            resolved.push(component);
        }
        let user_home = user_home()?;
        let user_home = fs::canonicalize(&user_home).unwrap_or(user_home);
        anyhow::ensure!(
            resolved.parent().is_some() && resolved != user_home,
            "use a dedicated LingShu data directory, not your home or filesystem root"
        );
        let paths = Self { home, binaries };
        if let Some(root) = paths.install_root() {
            let root = fs::canonicalize(&root).unwrap_or(root);
            anyhow::ensure!(!resolved.starts_with(root), "data directory must be outside the CLI installation so uninstall can retain memories");
        }
        Ok(paths)
    }
    pub fn file(&self, name: &str) -> PathBuf {
        self.home.join(name)
    }
    pub fn binary(&self, name: &str) -> PathBuf {
        self.binaries.join(executable_name(name))
    }
    pub fn install_root(&self) -> Option<PathBuf> {
        // Windows may retain the `current` junction in GetModuleFileName;
        // Unix resolves the launcher symlink into releases/<version>.
        let parent = self.binaries.parent()?;
        [Some(parent), parent.parent()]
            .into_iter()
            .flatten()
            .find_map(|root| {
                let metadata = fs::read(root.join("install.json")).ok()?;
                let metadata: serde_json::Value = serde_json::from_slice(&metadata).ok()?;
                (metadata["application"] == APPLICATION).then(|| root.to_owned())
            })
    }
    pub fn launcher(&self) -> Result<PathBuf> {
        Ok(self
            .install_root()
            .map(|root| root.join("current").join(executable_name("lingshu")))
            .unwrap_or(std::env::current_exe()?))
    }
}

fn executable_directory(executable: &Path) -> Result<PathBuf> {
    // macOS current_exe may preserve the user's bin/lingshu symlink. Resolve
    // it before looking for sibling programs and the installation marker.
    #[cfg(unix)]
    let executable = fs::canonicalize(executable).context("resolve CLI executable")?;
    Ok(executable
        .parent()
        .context("executable has no parent")?
        .to_owned())
}

pub fn executable_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

pub fn user_home() -> Result<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .context("user home is unavailable")
}

fn default_home() -> Result<PathBuf> {
    if cfg!(windows) {
        return Ok(PathBuf::from(
            std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is unavailable")?,
        )
        .join("LingShu")
        .join("cli-data"));
    }
    if cfg!(target_os = "macos") {
        return Ok(user_home()?.join("Library/Application Support/LingShu/cli"));
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or(user_home()?.join(".local/share"));
    Ok(base.join("lingshu"))
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub application: String,
    pub port: u16,
    pub database_port: u16,
    pub database_url: Option<String>,
    pub jwt_secret: String,
    pub encryption_key: String,
}

impl Config {
    pub fn load(paths: &Paths) -> Result<Self> {
        private_directory(&paths.home)?;
        let _lock = lock(&paths.file("config.lock"), false)?;
        let path = paths.file("config.json");
        if path.exists() {
            let config: Self = serde_json::from_slice(&fs::read(&path)?)?;
            anyhow::ensure!(
                config.application == APPLICATION,
                "not a LingShu CLI data directory"
            );
            anyhow::ensure!(
                config.port != 0
                    && config.database_port != 0
                    && config.jwt_secret.len() >= 32
                    && config.encryption_key.len() == 64,
                "invalid CLI configuration"
            );
            return Ok(config);
        }
        let config = Self {
            application: APPLICATION.into(),
            port: 8080,
            database_port: 55433,
            database_url: std::env::var("LINGSHU_DATABASE_URL")
                .ok()
                .filter(|v| !v.is_empty()),
            jwt_secret: secret(),
            encryption_key: secret(),
        };
        atomic_write(&path, &serde_json::to_vec_pretty(&config)?)?;
        Ok(config)
    }
    pub fn save(&self, paths: &Paths) -> Result<()> {
        let _lock = lock(&paths.file("config.lock"), false)?;
        atomic_write(
            &paths.file("config.json"),
            &serde_json::to_vec_pretty(self)?,
        )
    }
}

pub fn secret() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn private_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub fn private_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(false).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options.open(path)?)
}

pub fn lock(path: &Path, nonblocking: bool) -> Result<File> {
    let file = private_file(path)?;
    if nonblocking {
        file.try_lock_exclusive()
            .context("another LingShu process owns this operation")?;
    } else {
        file.lock_exclusive()?;
    }
    Ok(file)
}

pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path.parent().context("file has no parent")?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".lingshu-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(data)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn sibling_programs_resolve_through_launcher_and_current_symlinks() {
        use std::os::unix::fs::symlink;
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("installation with spaces");
        let release = root.join("releases/version");
        let bin = temporary.path().join("bin");
        fs::create_dir_all(&release).unwrap();
        fs::create_dir_all(&bin).unwrap();
        fs::write(release.join("lingshu"), b"CLI").unwrap();
        fs::write(release.join("lingshu-server"), b"backend").unwrap();
        fs::write(
            root.join("install.json"),
            r#"{"application":"lingshu-cli"}"#,
        )
        .unwrap();
        symlink(&release, root.join("current")).unwrap();
        symlink(root.join("current/lingshu"), bin.join("lingshu")).unwrap();
        let paths = Paths {
            home: temporary.path().join("data"),
            binaries: executable_directory(&bin.join("lingshu")).unwrap(),
        };
        assert_eq!(paths.binaries, release);
        assert!(paths.binary("lingshu-server").is_file());
        assert_eq!(paths.launcher().unwrap(), root.join("current/lingshu"));
    }
    #[test]
    fn secrets_survive_restart_and_config_is_private() {
        let temp = tempfile::tempdir().unwrap();
        let paths = Paths {
            home: temp.path().join("data"),
            binaries: temp.path().into(),
        };
        let original = Config::load(&paths).unwrap();
        let next = Config::load(&paths).unwrap();
        assert_eq!(original.jwt_secret, next.jwt_secret);
        assert_ne!(original.jwt_secret, original.encryption_key);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(paths.file("config.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn stable_launcher_resolves_unix_release_and_windows_junction_layouts() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("installation with spaces");
        fs::create_dir_all(root.join("releases/1.6.2")).unwrap();
        fs::create_dir_all(root.join("current")).unwrap();
        fs::write(
            root.join("install.json"),
            r#"{"application":"lingshu-cli"}"#,
        )
        .unwrap();
        for binaries in [root.join("current"), root.join("releases/1.6.2")] {
            let paths = Paths {
                home: temporary.path().join("data"),
                binaries,
            };
            assert_eq!(paths.install_root(), Some(root.clone()));
            assert_eq!(
                paths.launcher().unwrap(),
                root.join("current").join(executable_name("lingshu"))
            );
        }
        fs::write(root.join("install.json"), "{}").unwrap();
        let paths = Paths {
            home: temporary.path().join("data"),
            binaries: root.join("current"),
        };
        assert!(paths.install_root().is_none());
    }
    #[test]
    fn lock_excludes_another_owner() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lock");
        let first = lock(&path, true).unwrap();
        assert!(lock(&path, true).is_err());
        drop(first);
        assert!(lock(&path, true).is_ok());
    }
}
