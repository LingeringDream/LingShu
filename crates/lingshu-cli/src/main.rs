mod config;
mod connect;
mod service;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use config::Paths;
use std::{
    path::PathBuf,
    process::{Command, ExitCode, Stdio},
};

#[derive(Parser)]
#[command(
    name = "lingshu",
    version,
    about = "Local shared memory for your AI tools"
)]
struct Cli {
    /// Isolated data/config directory (also accepts LINGSHU_HOME).
    #[arg(long, global = true)]
    home: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}
#[derive(Clone, ValueEnum)]
enum Tool {
    Cursor,
    Claude,
    Codex,
}
#[derive(Subcommand)]
enum Commands {
    /// Start the background memory service; no Docker is required.
    Start {
        #[arg(long)]
        port: Option<u16>,
    },
    /// Gracefully stop the service and its database.
    Stop,
    /// Show managed service status (exit 1 when stopped).
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Configure an AI tool, preserving other servers and backing up its config.
    Connect {
        tool: Tool,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        print: bool,
    },
    /// Start the service if needed and open the memory console.
    Open {
        #[arg(long)]
        no_browser: bool,
    },
    /// Stdio MCP bridge; starts the managed backend when necessary.
    Mcp,
    /// Show recent service logs.
    Logs {
        #[arg(long, default_value_t = 50)]
        lines: usize,
    },
    /// Install a newer bundle, preserving data; --archive supports offline updates.
    Update {
        #[arg(long)]
        version: Option<String>,
        #[arg(long)]
        archive: Option<PathBuf>,
    },
    /// Remove CLI programs; retain memories unless --purge is explicitly supplied.
    Uninstall {
        #[arg(long)]
        purge: bool,
    },
    #[command(hide = true)]
    Daemon,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Cli::parse()).await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("lingshu: {error:#}");
            ExitCode::FAILURE
        }
    }
}
async fn run(cli: Cli) -> Result<ExitCode> {
    if let Some(home) = cli.home {
        std::env::set_var("LINGSHU_HOME", home);
    }
    let paths = Paths::load()?;
    match cli.command {
        Commands::Start { port } => {
            let status = service::start(&paths, port).await?;
            println!("LingShu ready: {}", status.url);
        }
        Commands::Stop => {
            service::stop(&paths).await?;
            println!("LingShu stopped; memories retained.");
        }
        Commands::Status { json } => {
            let status = service::status(&paths).await?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&status.clone().unwrap_or(service::Status {
                        running: false,
                        ready: false,
                        version: env!("CARGO_PKG_VERSION").into(),
                        url: String::new(),
                        pid: 0
                    }))?
                );
            } else if let Some(status) = &status {
                println!(
                    "LingShu {}: {} (v{}, supervisor {})",
                    if status.ready { "ready" } else { "starting" },
                    status.url,
                    status.version,
                    status.pid
                );
            } else {
                println!("LingShu is stopped.");
            }
            if !status.is_some_and(|status| status.ready) {
                return Ok(ExitCode::FAILURE);
            }
        }
        Commands::Connect {
            tool,
            config,
            print,
        } => {
            let name = match tool {
                Tool::Cursor => "cursor",
                Tool::Claude => "claude",
                Tool::Codex => "codex",
            };
            connect::connect(&paths, name, config, print)?;
        }
        Commands::Open { no_browser } => {
            let status = service::start(&paths, None).await?;
            anyhow::ensure!(
                paths.binaries.join("web/index.html").is_file(),
                "web console missing; install the complete CLI bundle"
            );
            println!("{}", status.url);
            if !no_browser {
                open_browser(&status.url)?;
            }
        }
        Commands::Mcp => {
            // Keep stdout exclusively for JSON-RPC, including cold-start messages.
            let url = match std::env::var("LINGSHU_BASE_URL")
                .ok()
                .filter(|url| !url.is_empty())
            {
                Some(url) => url,
                None => service::start(&paths, None).await?.url,
            };
            let status = tokio::process::Command::new(paths.binary("lingshu-mcp"))
                .env("LINGSHU_BASE_URL", url)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()
                .await
                .context("start MCP bridge")?;
            if !status.success() {
                return Ok(ExitCode::FAILURE);
            }
        }
        Commands::Logs { lines } => {
            let log = std::fs::read_to_string(paths.file("service.log"))
                .context("no service log; run `lingshu start` first")?;
            let collected: Vec<_> = log.lines().collect();
            for line in collected.iter().skip(collected.len().saturating_sub(lines)) {
                println!("{line}");
            }
        }
        Commands::Update { version, archive } => {
            let root = paths
                .install_root()
                .context("update requires an installed CLI bundle")?;
            let was_running = service::status(&paths).await?.is_some();
            // The installer validates downloads before stopping the old service.
            let mut command = installer(&paths);
            command.arg("--prefix").arg(&root);
            if let Some(version) = version {
                command.arg("--version").arg(version);
            }
            if let Some(archive) = archive {
                command.arg("--archive").arg(std::path::absolute(archive)?);
            }
            let result = command.env("LINGSHU_HOME", &paths.home).status()?;
            if was_running {
                let result = Command::new(
                    root.join("current")
                        .join(config::executable_name("lingshu")),
                )
                .arg("start")
                .env("LINGSHU_HOME", &paths.home)
                .status()?;
                anyhow::ensure!(
                    result.success(),
                    "updated, but service restart failed; run `lingshu logs`"
                );
            }
            anyhow::ensure!(
                result.success(),
                "update failed; previous installation and memories retained"
            );
        }

        Commands::Uninstall { purge } => {
            let root = paths
                .install_root()
                .context("uninstall requires an installed CLI bundle")?;
            if purge {
                connect::validate_purge(&paths.home)?;
            }
            service::stop(&paths).await?;
            let mut command = installer(&paths);
            command.arg("--prefix").arg(root).arg("--uninstall");
            if purge {
                command.arg("--purge-data").arg(&paths.home);
            }
            #[cfg(windows)]
            {
                let log_path = std::env::temp_dir()
                    .join(format!("lingshu-uninstall-{}.log", std::process::id()));
                let log = config::private_file(&log_path)?;
                command
                    .arg("--wait-pid")
                    .arg(std::process::id().to_string())
                    .stdin(Stdio::null())
                    .stdout(log.try_clone()?)
                    .stderr(log);
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000);
                command.spawn()?;
                println!(
                    "Uninstall scheduled after this process exits. Log: {}",
                    log_path.display()
                );
            }
            #[cfg(not(windows))]
            {
                anyhow::ensure!(command.status()?.success(), "uninstall failed");
            }
        }
        Commands::Daemon => service::daemon(&paths).await?,
    }
    Ok(ExitCode::SUCCESS)
}

fn installer(paths: &Paths) -> Command {
    if cfg!(windows) {
        let mut command = Command::new("powershell.exe");
        command
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(paths.binaries.join("install.ps1"));
        command
    } else {
        let mut command = Command::new("sh");
        command.arg(paths.binaries.join("install.sh"));
        command
    }
}
fn open_browser(url: &str) -> Result<()> {
    let mut command = if cfg!(windows) {
        let mut command = Command::new("cmd.exe");
        command.args(["/C", "start", "", url]);
        command
    } else {
        let mut command = Command::new(if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        });
        command.arg(url);
        command
    };
    anyhow::ensure!(
        command
            .status()
            .context("open browser (or use `lingshu open --no-browser`)")?
            .success(),
        "browser launch failed; open {url} manually"
    );
    Ok(())
}
