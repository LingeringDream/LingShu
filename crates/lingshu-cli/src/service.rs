use crate::config::{self, Config, Paths};
use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{io::AsyncWriteExt, sync::watch};

#[derive(Clone, Serialize, Deserialize)]
struct Runtime {
    control_port: u16,
    key: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Status {
    pub running: bool,
    pub ready: bool,
    pub version: String,
    pub url: String,
    pub pid: u32,
}
#[derive(Clone)]
struct Control {
    key: String,
    status: Status,
    ready: Arc<AtomicBool>,
    stop: watch::Sender<bool>,
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()?)
}
fn runtime(paths: &Paths) -> Result<Option<Runtime>> {
    match fs::read(paths.file("runtime.json")) {
        Ok(bytes) => Ok(Some(
            serde_json::from_slice(&bytes).context("invalid runtime.json")?,
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
pub async fn status(paths: &Paths) -> Result<Option<Status>> {
    let Some(runtime) = runtime(paths)? else {
        return Ok(None);
    };
    let response = client()?
        .get(format!("http://127.0.0.1:{}/status", runtime.control_port))
        .bearer_auth(runtime.key)
        .send()
        .await;
    match response {
        Ok(response) if response.status().is_success() => Ok(Some(response.json().await?)),
        _ => Ok(None),
    }
}

pub async fn start(paths: &Paths, port: Option<u16>) -> Result<Status> {
    config::private_directory(&paths.home)?;
    let _operation = config::lock(&paths.file("operation.lock"), false)?;
    if let Some(status) = status(paths).await? {
        if let Some(port) = port {
            anyhow::ensure!(
                status.url == format!("http://127.0.0.1:{port}"),
                "service already uses {}; stop it before changing ports",
                status.url
            );
        }
        if status.ready {
            return Ok(status);
        }
    } else {
        let mut config = Config::load(paths)?;
        if let Some(port) = port {
            anyhow::ensure!(port != 0, "port must not be zero");
            config.port = port;
            config.save(paths)?;
        }
        anyhow::ensure!(
            paths.binary("lingshu-server").is_file(),
            "lingshu-server is missing beside the CLI; install the complete CLI bundle"
        );
        let mut log = config::private_file(&paths.file("service.log"))?;
        std::io::Seek::seek(&mut log, std::io::SeekFrom::End(0))?;
        let mut command = std::process::Command::new(std::env::current_exe()?);
        command
            .arg("daemon")
            .env("LINGSHU_HOME", &paths.home)
            .current_dir(&paths.home)
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                command.pre_exec(|| {
                    if libc::setsid() < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().context("launch LingShu supervisor")?;
        eprintln!("Starting LingShu; first database initialization may take two minutes…");
        for _ in 0..650 {
            if let Some(status) = status(paths).await? {
                if status.ready {
                    return Ok(status);
                }
            }
            if let Some(code) = child.try_wait()? {
                anyhow::bail!(
                    "LingShu exited ({code}); see {}",
                    paths.file("service.log").display()
                );
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        anyhow::bail!(
            "startup timed out; inspect {} and run `lingshu stop`",
            paths.file("service.log").display()
        );
    }
    for _ in 0..650 {
        if let Some(status) = status(paths).await? {
            if status.ready {
                return Ok(status);
            }
        } else {
            anyhow::bail!(
                "supervisor stopped during startup; see {}",
                paths.file("service.log").display()
            );
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    anyhow::bail!("startup timed out; run `lingshu logs`")
}

pub async fn stop(paths: &Paths) -> Result<()> {
    if !paths.home.exists() {
        return Ok(());
    }
    let _operation = config::lock(&paths.file("operation.lock"), false)?;
    if status(paths).await?.is_none() {
        // A stale PID/port is never used to terminate another process.
        let _lock = config::lock(&paths.file("daemon.lock"), true)?;
        let _ = fs::remove_file(paths.file("runtime.json"));
        return Ok(());
    }
    let runtime = runtime(paths)?.context("runtime disappeared")?;
    client()?
        .post(format!("http://127.0.0.1:{}/stop", runtime.control_port))
        .bearer_auth(runtime.key)
        .send()
        .await?
        .error_for_status()?;
    for _ in 0..225 {
        if config::lock(&paths.file("daemon.lock"), true).is_ok() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    anyhow::bail!(
        "graceful shutdown timed out; see {}",
        paths.file("service.log").display()
    )
}

fn authorized(headers: &HeaderMap, state: &Control) -> bool {
    headers
        .get("authorization")
        .and_then(|header| header.to_str().ok())
        == Some(format!("Bearer {}", state.key).as_str())
}
async fn control_status(
    State(state): State<Control>,
    headers: HeaderMap,
) -> Result<Json<Status>, StatusCode> {
    if !authorized(&headers, &state) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let mut status = state.status.clone();
    status.ready = state.ready.load(Ordering::Acquire);
    Ok(Json(status))
}
async fn control_stop(State(state): State<Control>, headers: HeaderMap) -> StatusCode {
    if !authorized(&headers, &state) {
        return StatusCode::UNAUTHORIZED;
    }
    let _ = state.stop.send(true);
    StatusCode::ACCEPTED
}

pub async fn daemon(paths: &Paths) -> Result<()> {
    let _owner = config::lock(&paths.file("daemon.lock"), true)?;
    let config = Config::load(paths)?;
    // Refuse occupied ports instead of adopting or stopping an unrelated server.
    let probe = std::net::TcpListener::bind(("127.0.0.1", config.port))
        .context("backend port is occupied; use `lingshu start --port PORT`")?;
    drop(probe);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let runtime = Runtime {
        control_port: listener.local_addr()?.port(),
        key: config::secret(),
    };
    let url = format!("http://127.0.0.1:{}", config.port);
    let (stop_tx, mut stop_rx) = watch::channel(false);
    let ready = Arc::new(AtomicBool::new(false));
    let state = Control {
        key: runtime.key.clone(),
        status: Status {
            running: true,
            ready: false,
            version: env!("CARGO_PKG_VERSION").into(),
            url: url.clone(),
            pid: std::process::id(),
        },
        ready: ready.clone(),
        stop: stop_tx.clone(),
    };
    let app = Router::new()
        .route("/status", get(control_status))
        .route("/stop", post(control_stop))
        .with_state(state);
    let mut http_stop = stop_rx.clone();
    let control = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                while !*http_stop.borrow() {
                    if http_stop.changed().await.is_err() {
                        break;
                    }
                }
            })
            .await
    });
    let result = async {
        config::atomic_write(&paths.file("runtime.json"), &serde_json::to_vec(&runtime)?)?;
        let mut command = tokio::process::Command::new(paths.binary("lingshu-server"));
        command
            .current_dir(&paths.home)
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .env("LINGSHU_EXIT_WITH_PARENT", "1")
            .env("SERVER_HOST", "127.0.0.1")
            .env("SERVER_PORT", config.port.to_string())
            .env("JWT_SECRET", &config.jwt_secret)
            .env("ENCRYPTION_KEY", &config.encryption_key)
            .env("REDIS_URL", "")
            .env("QDRANT_URL", "")
            .env_remove("LINGSHU_TEAM_SERVER")
            .env("LINGSHU_EMBEDDED_DB_ROOT", paths.file("postgresql"))
            .env("LINGSHU_EMBEDDED_DB_PORT", config.database_port.to_string());
        #[cfg(windows)]
        {
            // PostgreSQL runs from the extracted data directory, so its DLL
            // search path must include the C runtime shipped beside the CLI.
            let mut search = vec![paths.binaries.clone()];
            if let Some(path) = std::env::var_os("PATH") {
                search.extend(std::env::split_paths(&path));
            }
            command.env("PATH", std::env::join_paths(search)?);
        }
        if let Some(database_url) = &config.database_url {
            command
                .env("DATABASE_URL", database_url)
                .env("LINGSHU_EMBEDDED_DB", "0");
        } else {
            command
                .env("LINGSHU_EMBEDDED_DB", "1")
                .env_remove("DATABASE_URL");
        }
        if paths.binaries.join("web/index.html").is_file() {
            command.env("LINGSHU_WEB_DIR", paths.binaries.join("web"));
        } else {
            command.env_remove("LINGSHU_WEB_DIR");
        }
        let mut server = command.spawn().context("start bundled backend")?;
        let client = client()?;
        let startup = tokio::time::Instant::now();
        let outcome = loop {
            if let Some(code) = server.try_wait()? {
                break Err(anyhow::anyhow!("backend exited: {code}"));
            }
            if *stop_rx.borrow() {
                break Ok(());
            }
            if !ready.load(Ordering::Acquire) {
                if startup.elapsed() > Duration::from_secs(120) {
                    break Err(anyhow::anyhow!("backend readiness timed out"));
                }
                if let Ok(response) = client
                    .get(format!("{url}/api/v1/system/health"))
                    .send()
                    .await
                {
                    if response.status().is_success() {
                        ready.store(true, Ordering::Release);
                    }
                }
            }
            tokio::select! {
                _ = stop_rx.changed() => {},
                _ = tokio::time::sleep(Duration::from_millis(200)) => {},
            }
        };
        // Existing sidecar protocol also shuts down embedded PostgreSQL on Windows.
        if let Some(mut input) = server.stdin.take() {
            let _ = input.write_all(b"shutdown\n").await;
            let _ = input.shutdown().await;
        }
        if tokio::time::timeout(Duration::from_secs(30), server.wait())
            .await
            .is_err()
        {
            eprintln!("Backend did not stop gracefully; terminating the managed child.");
            let _ = server.kill().await;
        }
        outcome
    }
    .await;
    let _ = stop_tx.send(true);
    let _ = control.await;
    let _ = fs::remove_file(paths.file("runtime.json"));
    result
}
