// ── 灵枢 (LingShu) Tauri 2 Desktop Shell ─────────────────────────────
// This is the desktop entry point. It opens two windows:
//   "main" — the full control panel (memory, personality, calendar)
//   "pet"  — a small frameless transparent always-on-top floating avatar
//
// The axum backend runs independently on 127.0.0.1:8080.
// Frontend calls go through Vite proxy (dev) or directly (prod).
//
// Phase B: EventKit commands in eventkit.rs (macOS only).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

#[path = "eventkit.rs"]
mod eventkit;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            eventkit::request_calendar_access,
            eventkit::create_calendar_event,
            eventkit::update_calendar_event,
            eventkit::delete_calendar_event,
        ])
        .setup(|app| {
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
