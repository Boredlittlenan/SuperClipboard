mod classifier;
mod clipboard;
mod storage;

use clipboard::ClipboardMonitor;
use log::info;
use std::sync::Arc;
use storage::{ClipboardEntry, QueryFilter, Storage};
use tauri::Manager;

/// Tauri-managed application state
pub struct AppState {
    storage: Arc<Storage>,
    monitor: std::sync::Mutex<ClipboardMonitor>,
}

// ─── Tauri Commands ──────────────────────────────────────────────────

#[tauri::command]
fn get_entries(
    state: tauri::State<'_, AppState>,
    filter: Option<QueryFilter>,
) -> Result<Vec<ClipboardEntry>, String> {
    state
        .storage
        .query(&filter.unwrap_or_default())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_entry(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    state.storage.delete(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn toggle_pin(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    state.storage.toggle_pin(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_stats(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let total = state.storage.count(None).map_err(|e| e.to_string())?;
    let text = state.storage.count(Some("text")).map_err(|e| e.to_string())?;
    let link = state.storage.count(Some("link")).map_err(|e| e.to_string())?;
    let image = state.storage.count(Some("image")).map_err(|e| e.to_string())?;
    let code = state.storage.count(Some("code")).map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "total": total,
        "text": text,
        "link": link,
        "image": image,
        "code": code,
    }))
}

#[tauri::command]
fn clear_unpinned(state: tauri::State<'_, AppState>) -> Result<u64, String> {
    state.storage.clear_unpinned().map_err(|e| e.to_string())
}

/// Copy a stored entry back to the system clipboard
#[tauri::command]
fn copy_to_clipboard(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    let entry = state
        .storage
        .get_entry_by_id(id)
        .map_err(|e| e.to_string())?;

    if let Some(entry) = entry {
        let mut clip = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        match entry.category {
            classifier::Category::Image => {
                use base64::Engine;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(&entry.content)
                    .map_err(|e| e.to_string())?;
                let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let img_data = arboard::ImageData {
                    width: w as usize,
                    height: h as usize,
                    bytes: rgba.into_raw().into(),
                };
                clip.set_image(img_data).map_err(|e| e.to_string())?;
            }
            _ => {
                clip.set_text(&entry.content).map_err(|e| e.to_string())?;
            }
        }
        Ok(true)
    } else {
        Ok(false)
    }
}

// ─── App Setup ───────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // Initialize storage in app data directory
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");
            std::fs::create_dir_all(&app_dir).expect("Failed to create app data directory");

            let db_path = app_dir.join("clipboard.db");
            info!("Database path: {:?}", db_path);

            let storage = Arc::new(Storage::new(&db_path).expect("Failed to initialize storage"));

            // Start clipboard monitor
            let mut monitor = ClipboardMonitor::new();
            monitor.start(app.handle().clone(), storage.clone());

            app.manage(AppState {
                storage,
                monitor: std::sync::Mutex::new(monitor),
            });

            // Register global shortcut: Ctrl+Shift+V to show/hide window
            use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
            let _ = app.global_shortcut().on_shortcut("Ctrl+Shift+V", |app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    if let Some(window) = app.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
            });

            // Set up system tray click handler
            let handle = app.handle().clone();
            if let Some(tray) = app.tray_by_id("main-tray") {
                tray.on_tray_icon_event(move |_tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(window) = handle.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_entries,
            delete_entry,
            toggle_pin,
            get_stats,
            clear_unpinned,
            copy_to_clipboard,
        ])
        .run(tauri::generate_context!())
        .expect("Error while running tauri application");
}
