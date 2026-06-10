// ── 灵枢 (LingShu) Tauri 2 Desktop Shell ─────────────────────────────
// Self-contained desktop app: launches the axum backend as a sidecar,
// opens main (control panel) + pet (floating avatar) windows, and
// kills the backend when the app exits.
//
// Build:  ./scripts/build-sidecar.sh
//         cd frontend && npm run tauri build
// → src-tauri/target/release/bundle/macos/LingShu.app

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_shell::process::CommandChild;
use tauri_plugin_shell::ShellExt;

#[path = "eventkit.rs"]
mod eventkit;

#[path = "automation.rs"]
mod automation;

/// Wrapper so we can shut down the sidecar when the app exits.
/// `Mutex` is required by `app.manage()` (`T: Sync`). The lock is only
/// ever contended at program exit (single `Drop` call), so `lock()` never
/// blocks in practice.
struct SidecarGuard(Mutex<Option<CommandChild>>);

impl Drop for SidecarGuard {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.0.lock() {
            if let Some(child) = guard.take() {
                if let Err(e) = child.kill() {
                    eprintln!("[lingshu-tauri] failed to kill sidecar: {e}");
                }
            }
        }
    }
}

/// Poll until the backend is listening on the given port or we time out.
async fn wait_for_backend(port: u16, timeout: std::time::Duration) -> bool {
    use tokio::net::TcpStream;
    let deadline = tokio::time::Instant::now() + timeout;
    let addr = format!("127.0.0.1:{port}");
    while tokio::time::Instant::now() < deadline {
        if TcpStream::connect(&addr).await.is_ok() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
    false
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            eventkit::request_calendar_access,
            eventkit::create_calendar_event,
            eventkit::update_calendar_event,
            eventkit::delete_calendar_event,
            automation::open_application,
            automation::open_url,
            automation::open_path,
        ])
        .setup(|app| {
            // ── Launch the backend sidecar (bundled .app) ──────────
            // In dev mode the sidecar binary may not exist — that's fine,
            // just run `cargo run -p lingshu-server` in another terminal.
            match app.shell().sidecar("lingshu-server") {
                Ok(sidecar) => match sidecar.spawn() {
                    Ok((mut rx, child)) => {
                        app.manage(SidecarGuard(Mutex::new(Some(child))));
                        tauri::async_runtime::spawn(async move {
                            while let Some(event) = rx.recv().await {
                                match event {
                                    tauri_plugin_shell::process::CommandEvent::Stdout(line) => {
                                        println!("[lingshu-server] {}", String::from_utf8_lossy(&line));
                                    }
                                    tauri_plugin_shell::process::CommandEvent::Stderr(line) => {
                                        eprintln!("[lingshu-server] {}", String::from_utf8_lossy(&line));
                                    }
                                    tauri_plugin_shell::process::CommandEvent::Terminated(status) => {
                                        println!("[lingshu-server] exited with {status:?}");
                                    }
                                    tauri_plugin_shell::process::CommandEvent::Error(err) => {
                                        eprintln!("[lingshu-server] error: {err}");
                                    }
                                    _ => {}
                                }
                            }
                        });
                        println!("[lingshu-tauri] sidecar launched");
                    }
                    Err(e) => {
                        eprintln!("[lingshu-tauri] failed to spawn sidecar: {e}");
                    }
                },
                Err(e) => {
                    eprintln!("[lingshu-tauri] sidecar not found (dev mode?): {e}");
                }
            }

            // Wait for the backend port before showing windows.
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let ready =
                    wait_for_backend(8080, std::time::Duration::from_secs(15)).await;
                if ready {
                    println!("[lingshu-tauri] backend ready, showing windows");
                } else {
                    eprintln!("[lingshu-tauri] backend did not become ready within timeout — start it manually with `cargo run -p lingshu-server`");
                }
                if let Some(main) = app_handle.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.set_focus();
                }
                if let Some(pet) = app_handle.get_webview_window("pet") {
                    let _ = pet.show();
                }
            });

            #[cfg(target_os = "macos")]
            {
                use tauri::ActivationPolicy;
                app.set_activation_policy(ActivationPolicy::Regular);
            }

            // Position the pet window near the bottom-right of the screen.
            if let Some(pet) = app.get_webview_window("pet") {
                if let Ok(Some(monitor)) = pet.primary_monitor() {
                    let size = monitor.size();
                    let scale = monitor.scale_factor();
                    let x = ((size.width as f64) / scale) - 220.0;
                    let y = ((size.height as f64) / scale) - 300.0;
                    let _ = pet.set_position(tauri::PhysicalPosition::new(x, y));
                }
            }

            // Focus the main window on start.
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.set_focus();
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running LingShu desktop app");
}
