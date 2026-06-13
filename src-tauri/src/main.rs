// ── 灵枢 (LingShu) Tauri 2 Desktop Shell ─────────────────────────────
// Self-contained desktop app: launches the axum backend as a sidecar,
// opens main (control panel) + pet (floating avatar) windows, and
// kills the backend when the app exits.
//
// Build:  ./scripts/build-sidecar.sh
//         cd frontend && npm run tauri build
// → src-tauri/target/release/bundle/macos/LingShu.app

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, sync::Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;
use tauri_plugin_shell::process::CommandChild;
use tauri_plugin_shell::ShellExt;

const BACKEND_PORT: u16 = 8080;
const SIDECAR_BINARY_NAME: &str = "lingshu-server";
const SIDECAR_EXIT_WITH_PARENT_ENV: &str = "LINGSHU_EXIT_WITH_PARENT";
const SIDECAR_EXIT_WITH_PARENT_VALUE: &str = "1";

#[path = "eventkit.rs"]
mod eventkit;

#[path = "automation.rs"]
mod automation;

#[path = "fileio.rs"]
mod fileio;

#[path = "screenreader.rs"]
mod screenreader;

/// Wrapper so we can shut down the sidecar when the app exits.
/// `Mutex` is required by `app.manage()` (`T: Sync`). The lock is only
/// ever contended at program exit (single `Drop` call), so `lock()` never
/// blocks in practice.
struct SidecarGuard(Mutex<Option<CommandChild>>);

const TRAY_SHOW_MAIN_ID: &str = "show-main";
const TRAY_QUIT_ID: &str = "quit";
const PET_WINDOW_POSITION_FILE: &str = "pet-window-position.json";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PetWindowPosition {
    x: i32,
    y: i32,
}

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

fn parse_lsof_pids(output: &str) -> Vec<u32> {
    output
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect()
}

fn command_looks_like_lingshu_sidecar(command: &str) -> bool {
    let command = command.trim();
    if command.is_empty() {
        return false;
    }

    command.split('/').any(|segment| {
        segment == SIDECAR_BINARY_NAME || segment.starts_with(&format!("{SIDECAR_BINARY_NAME} "))
    })
}

fn command_looks_like_bundled_lingshu_sidecar(command: &str) -> bool {
    command.contains(".app/Contents/MacOS/") && command_looks_like_lingshu_sidecar(command)
}

#[cfg(target_os = "macos")]
fn listening_pids_on_port(port: u16) -> Vec<u32> {
    let port_arg = format!("-iTCP:{port}");
    let output = std::process::Command::new("/usr/sbin/lsof")
        .args(["-nP", "-t", port_arg.as_str(), "-sTCP:LISTEN"])
        .output()
        .or_else(|_| {
            std::process::Command::new("lsof")
                .args(["-nP", "-t", port_arg.as_str(), "-sTCP:LISTEN"])
                .output()
        });

    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    parse_lsof_pids(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(target_os = "macos")]
fn command_for_pid(pid: u32) -> Option<String> {
    let output = std::process::Command::new("/bin/ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let command = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!command.is_empty()).then_some(command)
}

#[cfg(target_os = "macos")]
fn bundled_lingshu_sidecar_pids_on_port(port: u16) -> Vec<u32> {
    let current_pid = std::process::id();
    listening_pids_on_port(port)
        .into_iter()
        .filter(|pid| *pid != current_pid)
        .filter(|pid| {
            command_for_pid(*pid)
                .as_deref()
                .is_some_and(command_looks_like_bundled_lingshu_sidecar)
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn signal_pid(pid: u32, signal: &str) {
    let _ = std::process::Command::new("/bin/kill")
        .args([signal, &pid.to_string()])
        .status();
}

#[cfg(target_os = "macos")]
fn wait_until_port_released_from_pids(
    port: u16,
    pids: &[u32],
    timeout: std::time::Duration,
) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        let listening = listening_pids_on_port(port);
        if !pids.iter().any(|pid| listening.contains(pid)) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    false
}

#[cfg(target_os = "macos")]
fn terminate_stale_bundled_sidecars(port: u16) {
    let stale_pids = bundled_lingshu_sidecar_pids_on_port(port);
    if stale_pids.is_empty() {
        return;
    }

    eprintln!(
        "[lingshu-tauri] terminating stale bundled sidecar(s) on port {port}: {stale_pids:?}"
    );
    for pid in &stale_pids {
        signal_pid(*pid, "-TERM");
    }

    if wait_until_port_released_from_pids(port, &stale_pids, std::time::Duration::from_secs(2)) {
        return;
    }

    eprintln!("[lingshu-tauri] stale bundled sidecar(s) did not exit after TERM; sending KILL");
    for pid in stale_pids {
        signal_pid(pid, "-KILL");
    }
}

#[cfg(not(target_os = "macos"))]
fn terminate_stale_bundled_sidecars(_port: u16) {}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.unminimize();
        let _ = main.set_focus();
    }
}

fn install_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show_main = MenuItem::with_id(app, TRAY_SHOW_MAIN_ID, "打开控制台", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, TRAY_QUIT_ID, "退出灵枢", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_main, &quit])?;

    let mut tray = TrayIconBuilder::with_id("lingshu-status")
        .menu(&menu)
        .tooltip("灵枢 LingShu")
        .title("灵枢")
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_SHOW_MAIN_ID => show_main_window(app),
            TRAY_QUIT_ID => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.build(app)?;
    Ok(())
}

fn pet_window_position_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(PET_WINDOW_POSITION_FILE))
}

fn load_pet_window_position(app: &tauri::AppHandle) -> Option<PetWindowPosition> {
    let path = pet_window_position_path(app)?;
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn save_pet_window_position(app: &tauri::AppHandle, position: PetWindowPosition) {
    let Some(path) = pet_window_position_path(app) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(data) = serde_json::to_string(&position) {
        let _ = fs::write(path, data);
    }
}

fn default_pet_position(
    monitor_width: u32,
    monitor_height: u32,
    monitor_scale_factor: f64,
) -> PetWindowPosition {
    PetWindowPosition {
        x: (((monitor_width as f64) / monitor_scale_factor) - 220.0).round() as i32,
        y: (((monitor_height as f64) / monitor_scale_factor) - 300.0).round() as i32,
    }
}

fn pet_start_position(
    saved: Option<PetWindowPosition>,
    monitor_width: u32,
    monitor_height: u32,
    monitor_scale_factor: f64,
) -> PetWindowPosition {
    saved.unwrap_or_else(|| {
        default_pet_position(monitor_width, monitor_height, monitor_scale_factor)
    })
}

fn main() {
    tauri::Builder::default()
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            if window.label() == "pet" {
                if let tauri::WindowEvent::Moved(position) = event {
                    save_pet_window_position(
                        window.app_handle(),
                        PetWindowPosition {
                            x: position.x,
                            y: position.y,
                        },
                    );
                }
            }
        })
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
            screenreader::read_screen,
            screenreader::request_accessibility_permission,
            fileio::read_file,
            fileio::list_directory,
        ])
        .setup(|app| {
            install_tray(app)?;

            // ── Launch the backend sidecar (bundled .app) ──────────
            // In dev mode the sidecar binary may not exist — that's fine,
            // just run `cargo run -p lingshu-server` in another terminal.
            match app.shell().sidecar(SIDECAR_BINARY_NAME) {
                Ok(sidecar) => {
                    terminate_stale_bundled_sidecars(BACKEND_PORT);
                    // Have the sidecar self-terminate if this app dies, so an
                    // abrupt exit (Ctrl+C on `tauri dev`, a rebuild-triggered
                    // restart, or a crash) can't orphan it on :8080 and block
                    // the next launch.
                    let sidecar = sidecar.env(
                        SIDECAR_EXIT_WITH_PARENT_ENV,
                        SIDECAR_EXIT_WITH_PARENT_VALUE,
                    );
                    match sidecar.spawn() {
                        Ok((mut rx, child)) => {
                            app.manage(SidecarGuard(Mutex::new(Some(child))));
                            tauri::async_runtime::spawn(async move {
                                while let Some(event) = rx.recv().await {
                                    match event {
                                        tauri_plugin_shell::process::CommandEvent::Stdout(line) => {
                                            println!(
                                                "[lingshu-server] {}",
                                                String::from_utf8_lossy(&line)
                                            );
                                        }
                                        tauri_plugin_shell::process::CommandEvent::Stderr(line) => {
                                            eprintln!(
                                                "[lingshu-server] {}",
                                                String::from_utf8_lossy(&line)
                                            );
                                        }
                                        tauri_plugin_shell::process::CommandEvent::Terminated(
                                            status,
                                        ) => {
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
                    }
                }
                Err(e) => {
                    eprintln!("[lingshu-tauri] sidecar not found (dev mode?): {e}");
                }
            }

            // Wait for the backend port before showing windows.
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let ready =
                    wait_for_backend(BACKEND_PORT, std::time::Duration::from_secs(15)).await;
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
                // macOS: hide pet from Dock window menu & Cmd+Tab
                #[cfg(target_os = "macos")]
                unsafe {
                    let raw = pet.ns_window().expect("NSWindow");
                    let ns_window = raw as *mut objc2::runtime::AnyObject;
                    use objc2::msg_send;
                    // NSWindowCollectionBehaviorTransient     = 1 << 3
                    // NSWindowCollectionBehaviorIgnoresCycle = 1 << 5
                    let behavior: isize = msg_send![ns_window, collectionBehavior];
                    let _: () = msg_send![ns_window, setCollectionBehavior: behavior | (1 << 3) | (1 << 5)];
                }

                if let Ok(Some(monitor)) = pet.primary_monitor() {
                    let size = monitor.size();
                    let scale = monitor.scale_factor();
                    let position = pet_start_position(
                        load_pet_window_position(app.handle()),
                        size.width,
                        size.height,
                        scale,
                    );
                    let _ = pet.set_position(tauri::PhysicalPosition::new(position.x, position.y));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pet_start_position_prefers_saved_position() {
        let saved = PetWindowPosition { x: 320, y: 240 };

        assert_eq!(
            pet_start_position(Some(saved), 1920, 1080, 2.0),
            PetWindowPosition { x: 320, y: 240 }
        );
    }

    #[test]
    fn pet_start_position_defaults_to_bottom_right_without_saved_position() {
        assert_eq!(
            pet_start_position(None, 1920, 1080, 2.0),
            PetWindowPosition { x: 740, y: 240 }
        );
    }

    #[test]
    fn parse_lsof_pids_ignores_invalid_lines() {
        assert_eq!(parse_lsof_pids("62622\nnot-a-pid\n42\n"), vec![62622, 42]);
    }

    #[test]
    fn command_looks_like_lingshu_sidecar_matches_only_sidecar_binary() {
        assert!(command_looks_like_lingshu_sidecar(
            "/Applications/灵枢 LingShu.app/Contents/MacOS/lingshu-server"
        ));
        assert!(command_looks_like_lingshu_sidecar(
            "/Users/ymqz/projects/PA/target/debug/lingshu-server --port 8080"
        ));
        assert!(!command_looks_like_lingshu_sidecar(
            "/opt/homebrew/bin/postgres -D /tmp/db"
        ));
        assert!(!command_looks_like_lingshu_sidecar(
            "/Applications/灵枢 LingShu.app/Contents/MacOS/lingshu-tauri"
        ));
    }

    #[test]
    fn bundled_sidecar_match_is_limited_to_app_bundle_binary() {
        assert!(command_looks_like_bundled_lingshu_sidecar(
            "/Applications/灵枢 LingShu.app/Contents/MacOS/lingshu-server"
        ));
        assert!(!command_looks_like_bundled_lingshu_sidecar(
            "/Users/ymqz/projects/PA/target/debug/lingshu-server --port 8080"
        ));
    }

    #[test]
    fn sidecar_parent_watch_env_contract_is_stable() {
        assert_eq!(SIDECAR_EXIT_WITH_PARENT_ENV, "LINGSHU_EXIT_WITH_PARENT");
        assert_eq!(SIDECAR_EXIT_WITH_PARENT_VALUE, "1");
    }
}
