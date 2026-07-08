mod autostart;
mod backup;
mod classifier;
mod clipboard;
mod remote_storage;
mod storage;
mod storage_backend;
mod window_position;

use clipboard::ClipboardMonitor;
use log::info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use storage::{ClipboardEntry, Memo, MemoFilter, QueryFilter, RestoreSummary, Storage};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::Emitter;
use tauri::Manager;
use window_position::{WindowPoint, WindowPositionService, WindowSize};

/// Current application version
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// GitHub repository owner/name for update checks
const GITHUB_REPO: &str = "Boredlittlenan/SuperClipboard";
const DEFAULT_SHORTCUT: &str = "Alt+X";
const DEFAULT_SETTINGS_VERSION: &str = "2";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub download_url: String,
    pub has_update: bool,
    pub release_name: String,
    pub release_notes: String,
    pub published_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFileInfo {
    pub file_name: String,
    pub created_at: String,
    pub size_bytes: u64,
    pub display_path: String,
    pub app_version: String,
    pub backup_version: u32,
}

/// Tauri-managed application state
pub struct AppState {
    storage: Arc<Storage>,
    _monitor: std::sync::Mutex<ClipboardMonitor>,
    current_shortcut: std::sync::Mutex<String>,
    shortcut_recording: Arc<AtomicBool>,
}

fn tray_menu_labels(language: &str) -> (&'static str, &'static str) {
    match language {
        "zh-CN" => ("设置", "退出"),
        _ => ("Settings", "Quit"),
    }
}

fn tray_tooltip_label(language: &str) -> &'static str {
    match language {
        "zh-CN" => "超级剪贴板",
        _ => "SuperClipboard",
    }
}

fn normalize_locale(locale: &str) -> &'static str {
    if locale.to_ascii_lowercase().starts_with("zh") {
        "zh-CN"
    } else {
        "en"
    }
}

#[cfg(windows)]
fn detect_system_locale() -> &'static str {
    use windows::Win32::Globalization::GetUserDefaultLocaleName;

    let mut buffer = [0u16; 85];
    let len = unsafe { GetUserDefaultLocaleName(&mut buffer) };
    if len > 0 {
        let locale = String::from_utf16_lossy(&buffer[..(len as usize).saturating_sub(1)]);
        normalize_locale(&locale)
    } else {
        "en"
    }
}

#[cfg(not(windows))]
fn detect_system_locale() -> &'static str {
    std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .map(|locale| normalize_locale(&locale))
        .unwrap_or("en")
}

fn copy_file_if_missing(from: &Path, to: &Path) {
    if from.exists() && !to.exists() {
        let _ = std::fs::copy(from, to);
    }
}

fn migrate_legacy_app_data(app_dir: &Path) {
    let Some(parent) = app_dir.parent() else {
        return;
    };

    let candidates = [
        parent.join("com.superclipboard3.app"),
        parent.join("SuperClipboard3"),
        parent.join("superclipboard3"),
    ];
    let current_db = app_dir.join("clipboard.db");

    if current_db.exists() {
        return;
    }

    for legacy_dir in candidates {
        let legacy_db = legacy_dir.join("clipboard.db");
        if legacy_db.exists() {
            let _ = std::fs::create_dir_all(app_dir);
            copy_file_if_missing(&legacy_db, &current_db);
            copy_file_if_missing(
                &legacy_dir.join("clipboard.db-wal"),
                &app_dir.join("clipboard.db-wal"),
            );
            copy_file_if_missing(
                &legacy_dir.join("clipboard.db-shm"),
                &app_dir.join("clipboard.db-shm"),
            );
            info!(
                "Migrated legacy app data from {:?} to {:?}",
                legacy_dir, app_dir
            );
            break;
        }
    }
}

fn update_tray_menu(app: &tauri::AppHandle, language: &str) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let (settings_label, quit_label) = tray_menu_labels(language);
        let settings_item = MenuItemBuilder::with_id("settings", settings_label).build(app)?;
        let quit_item = MenuItemBuilder::with_id("quit", quit_label).build(app)?;
        let menu = MenuBuilder::new(app)
            .item(&settings_item)
            .separator()
            .item(&quit_item)
            .build()?;
        tray.set_menu(Some(menu))?;
        tray.set_tooltip(Some(tray_tooltip_label(language)))?;
    }
    Ok(())
}

// ─── Position Helpers ────────────────────────────────────────────────

fn set_default_setting_if_missing(storage: &Storage, key: &str, value: &str) {
    if matches!(storage.get_setting(key), Ok(None)) {
        let _ = storage.set_setting(key, value);
    }
}

fn initialize_first_run_defaults(storage: &Storage) {
    // Only run this for a newly-created database. Existing users keep their saved data/settings.
    set_default_setting_if_missing(storage, "shortcut", DEFAULT_SHORTCUT);
    set_default_setting_if_missing(storage, "theme_mode", "system");
    set_default_setting_if_missing(storage, "theme_accent", "default");
    set_default_setting_if_missing(storage, "always_on_top", "false");
    set_default_setting_if_missing(storage, "raw_preview", "false");
    set_default_setting_if_missing(storage, "auto_update", "true");
    set_default_setting_if_missing(storage, "memo_enabled", "false");
    set_default_setting_if_missing(storage, "archive_enabled", "false");
    set_default_setting_if_missing(storage, "autostart", "true");
    set_default_setting_if_missing(storage, "language", detect_system_locale());
    set_default_setting_if_missing(storage, "defaults_schema_version", DEFAULT_SETTINGS_VERSION);

    let _ = autostart::enable();
}

fn current_window_size(window: &tauri::WebviewWindow) -> WindowSize {
    let size = window
        .outer_size()
        .unwrap_or(tauri::PhysicalSize::new(420u32, 600u32));
    WindowSize {
        width: size.width as i32,
        height: size.height as i32,
    }
}

fn apply_window_position(window: &tauri::WebviewWindow, point: WindowPoint) {
    #[cfg(windows)]
    {
        if let Ok(hwnd) = window.hwnd() {
            if WindowPositionService::set_window_position_native(hwnd.0 as isize, point) {
                return;
            }
        }
    }

    let _ = window.set_position(tauri::PhysicalPosition::new(point.x, point.y));
}

#[cfg(windows)]
fn is_window_foreground(window: &tauri::WebviewWindow) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    let Ok(hwnd) = window.hwnd() else {
        return window.is_focused().unwrap_or(false);
    };

    let foreground = unsafe { GetForegroundWindow() };
    foreground.0 as isize == hwnd.0 as isize
}

#[cfg(not(windows))]
fn is_window_foreground(window: &tauri::WebviewWindow) -> bool {
    window.is_focused().unwrap_or(false)
}

#[cfg(windows)]
fn focus_window_native(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

    if let Ok(hwnd) = window.hwnd() {
        unsafe {
            let _ = SetForegroundWindow(HWND(hwnd.0));
        }
    }
}

#[cfg(not(windows))]
fn focus_window_native(_window: &tauri::WebviewWindow) {}

fn move_window_to_default_position(window: &tauri::WebviewWindow, source: &'static str) {
    let point = WindowPositionService::default_position(current_window_size(window));
    info!(
        "[position] source={}, reset=true, x={}, y={}",
        source, point.x, point.y
    );
    apply_window_position(window, point);
}

fn show_window(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    source: &'static str,
    reset_to_default_position: bool,
) {
    if reset_to_default_position {
        move_window_to_default_position(window, source);
    }

    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    focus_window_native(window);

    if reset_to_default_position {
        move_window_to_default_position(window, source);
    }

    let _ = app.emit("window-shown", source);
}

fn toggle_window(app: &tauri::AppHandle, window: &tauri::WebviewWindow, source: &'static str) {
    if window.is_visible().unwrap_or(false) && is_window_foreground(window) {
        let _ = window.hide();
    } else {
        show_window(app, window, source, false);
    }
}

fn backups_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?
        .join("backups");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create backup directory: {}", e))?;
    Ok(dir)
}

fn backup_file_info(path: PathBuf) -> Result<BackupFileInfo, String> {
    let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    let backup_metadata = backup::read_backup_metadata(&path)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Invalid backup file name".to_string())?
        .to_string();

    Ok(BackupFileInfo {
        file_name,
        created_at: backup_metadata.created_at,
        size_bytes: metadata.len(),
        display_path: path.display().to_string(),
        app_version: backup_metadata.app_version,
        backup_version: backup_metadata.backup_version,
    })
}

fn resolve_backup_path(app: &tauri::AppHandle, file_name: &str) -> Result<PathBuf, String> {
    if file_name.contains(['/', '\\']) {
        return Err("Invalid backup file name".to_string());
    }

    let path = backups_dir(app)?.join(file_name);
    if !backup::is_supported_backup_path(&path) {
        return Err("Backup file must be a .scbackup file".to_string());
    }
    if !path.exists() {
        return Err("Backup file not found".to_string());
    }
    Ok(path)
}

// ─── Shortcut Helpers ────────────────────────────────────────────────

fn normalize_shortcut(shortcut: &str) -> String {
    shortcut
        .split('+')
        .filter_map(|part| {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                return None;
            }

            let normalized = match trimmed.to_ascii_lowercase().as_str() {
                "control" | "ctrl" => "Ctrl".to_string(),
                "meta" | "super" | "win" | "windows" => "Super".to_string(),
                "alt" | "option" => "Alt".to_string(),
                "shift" => "Shift".to_string(),
                key if key.len() == 1 => key.to_ascii_uppercase(),
                _ => trimmed.to_string(),
            };
            Some(normalized)
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// Register a global shortcut that toggles the main window visibility.
fn register_toggle_shortcut(
    app: &tauri::AppHandle,
    shortcut: &str,
    shortcut_recording: Arc<AtomicBool>,
) -> Result<(), tauri_plugin_global_shortcut::Error> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
    let app = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                if shortcut_recording.load(Ordering::SeqCst) {
                    return;
                }
                let app = app.clone();
                let app_for_main = app.clone();
                let _ = app.run_on_main_thread(move || {
                    if let Some(window) = app_for_main.get_webview_window("main") {
                        toggle_window(&app_for_main, &window, "shortcut");
                    }
                });
            }
        })
}

// ─── Tauri Commands ──────────────────────────────────────────────────

#[tauri::command]
fn get_entries(
    state: tauri::State<'_, AppState>,
    filter: Option<QueryFilter>,
) -> Result<Vec<ClipboardEntry>, String> {
    let filter = filter.unwrap_or_default();
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::query_clipboard(&state.storage, &filter).map_err(|e| e.to_string());
    }
    state.storage.query(&filter).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_entry(
    state: tauri::State<'_, AppState>,
    id: i64,
    archive: Option<bool>,
) -> Result<bool, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::delete_clipboard(&state.storage, id, archive.unwrap_or(false))
            .map_err(|e| e.to_string());
    }
    if archive.unwrap_or(false) {
        state.storage.archive_entry(id).map_err(|e| e.to_string())
    } else {
        state.storage.delete(id).map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn toggle_pin(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::toggle_clipboard_pin(&state.storage, id).map_err(|e| e.to_string());
    }
    state.storage.toggle_pin(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_entry(
    state: tauri::State<'_, AppState>,
    id: i64,
    content: String,
) -> Result<bool, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::update_clipboard(&state.storage, id, &content)
            .map_err(|e| e.to_string());
    }
    state
        .storage
        .update_entry(id, &content)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_stats(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        let stats = remote_storage::stats(&state.storage).map_err(|e| e.to_string())?;
        return Ok(serde_json::json!({
            "total": stats.total,
            "text": stats.text,
            "link": stats.link,
            "image": stats.image,
            "code": stats.code,
            "email": stats.email,
            "file_path": stats.file_path,
            "dbSize": 0,
            "clipboardSize": stats.clipboard_size,
            "memoSize": stats.memo_size,
            "archive": stats.archive,
            "memoCount": stats.memo_count,
            "memoArchive": stats.memo_archive,
        }));
    }

    let total = state.storage.count(None).map_err(|e| e.to_string())?;
    let text = state
        .storage
        .count(Some("text"))
        .map_err(|e| e.to_string())?;
    let link = state
        .storage
        .count(Some("link"))
        .map_err(|e| e.to_string())?;
    let image = state
        .storage
        .count(Some("image"))
        .map_err(|e| e.to_string())?;
    let code = state
        .storage
        .count(Some("code"))
        .map_err(|e| e.to_string())?;
    let email = state
        .storage
        .count(Some("email"))
        .map_err(|e| e.to_string())?;
    let file_path = state
        .storage
        .count(Some("file_path"))
        .map_err(|e| e.to_string())?;
    let db_size = state.storage.db_size().map_err(|e| e.to_string())?;
    let archive = state.storage.archive_count().map_err(|e| e.to_string())?;
    let clipboard_size = state
        .storage
        .clipboard_storage_size()
        .map_err(|e| e.to_string())?;
    let memo_size = state
        .storage
        .memo_storage_size()
        .map_err(|e| e.to_string())?;
    let memo_count = state.storage.memo_count().map_err(|e| e.to_string())?;
    let memo_archive = state
        .storage
        .memo_archive_count()
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "total": total,
        "text": text,
        "link": link,
        "image": image,
        "code": code,
        "email": email,
        "file_path": file_path,
        "dbSize": db_size,
        "clipboardSize": clipboard_size,
        "memoSize": memo_size,
        "archive": archive,
        "memoCount": memo_count,
        "memoArchive": memo_archive,
    }))
}

#[tauri::command]
fn clear_unpinned(state: tauri::State<'_, AppState>, archive: Option<bool>) -> Result<u64, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::clear_clipboard_unpinned(&state.storage, archive.unwrap_or(false))
            .map_err(|e| e.to_string());
    }
    state
        .storage
        .clear_unpinned(archive.unwrap_or(false))
        .map_err(|e| e.to_string())
}

// ─── Archive Commands ──────────────────────────────────────────

#[tauri::command]
fn archive_entry(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::archive_clipboard(&state.storage, id).map_err(|e| e.to_string());
    }
    state.storage.archive_entry(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn unarchive_entry(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::unarchive_clipboard(&state.storage, id).map_err(|e| e.to_string());
    }
    state.storage.unarchive_entry(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_archived_entries(
    state: tauri::State<'_, AppState>,
    filter: Option<QueryFilter>,
) -> Result<Vec<ClipboardEntry>, String> {
    let filter = filter.unwrap_or_default();
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::query_archived_clipboard(&state.storage, &filter)
            .map_err(|e| e.to_string());
    }
    state
        .storage
        .query_archived(&filter)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn archive_count(state: tauri::State<'_, AppState>) -> Result<i64, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::clipboard_archive_count(&state.storage).map_err(|e| e.to_string());
    }
    state.storage.archive_count().map_err(|e| e.to_string())
}

#[tauri::command]
fn permanent_delete(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::permanent_delete_clipboard(&state.storage, id)
            .map_err(|e| e.to_string());
    }
    state
        .storage
        .permanent_delete(id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn purge_old_archives(state: tauri::State<'_, AppState>, days: i64) -> Result<u64, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::purge_old_clipboard_archives(&state.storage, days)
            .map_err(|e| e.to_string());
    }
    state
        .storage
        .purge_old_archives(days)
        .map_err(|e| e.to_string())
}

/// Copy a stored entry back to the system clipboard
#[tauri::command]
fn copy_to_clipboard(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    let entry = if remote_storage::is_remote_mode(&state.storage) {
        remote_storage::get_clipboard_by_id(&state.storage, id).map_err(|e| e.to_string())?
    } else {
        state
            .storage
            .get_entry_by_id(id)
            .map_err(|e| e.to_string())?
    };

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

/// Get a user setting value by key
#[tauri::command]
fn get_setting(state: tauri::State<'_, AppState>, key: String) -> Result<Option<String>, String> {
    state.storage.get_setting(&key).map_err(|e| e.to_string())
}

/// Set a user setting value
#[tauri::command]
fn set_setting(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), String> {
    state
        .storage
        .set_setting(&key, &value)
        .map_err(|e| e.to_string())?;
    if key == "language" {
        update_tray_menu(&app, &value).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Check if auto-start on boot is enabled
#[tauri::command]
fn get_autostart_enabled() -> bool {
    autostart::is_enabled()
}

/// Enable or disable auto-start on boot
#[tauri::command]
fn set_autostart_enabled(enabled: bool) -> Result<bool, String> {
    if enabled {
        autostart::enable()?;
    } else {
        autostart::disable()?;
    }
    Ok(enabled)
}

/// Get the current global shortcut string
#[tauri::command]
fn get_shortcut(state: tauri::State<'_, AppState>) -> String {
    state.current_shortcut.lock().unwrap().clone()
}

/// Update the global shortcut at runtime
#[tauri::command]
fn set_shortcut(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    new_shortcut: String,
) -> Result<String, String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let old_shortcut = state.current_shortcut.lock().unwrap().clone();
    let new_shortcut = normalize_shortcut(&new_shortcut);

    state.shortcut_recording.store(false, Ordering::SeqCst);
    let _ = app.global_shortcut().unregister(old_shortcut.as_str());

    if let Err(err) = register_toggle_shortcut(
        &app,
        new_shortcut.as_str(),
        state.shortcut_recording.clone(),
    ) {
        if !old_shortcut.is_empty() {
            let _ = register_toggle_shortcut(
                &app,
                old_shortcut.as_str(),
                state.shortcut_recording.clone(),
            );
        }
        return Err(format!("Failed to register shortcut: {}", err));
    }

    *state.current_shortcut.lock().unwrap() = new_shortcut.clone();
    state
        .storage
        .set_setting("shortcut", &new_shortcut)
        .map_err(|e| e.to_string())?;

    Ok(new_shortcut)
}

#[tauri::command]
fn set_shortcut_recording(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    recording: bool,
) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let shortcut = state.current_shortcut.lock().unwrap().clone();
    state.shortcut_recording.store(recording, Ordering::SeqCst);

    let _ = app.global_shortcut().unregister(shortcut.as_str());
    if !recording {
        register_toggle_shortcut(&app, shortcut.as_str(), state.shortcut_recording.clone())
            .map_err(|e| format!("Failed to restore shortcut: {}", e))?;
    }

    Ok(())
}

/// Set window always-on-top at runtime
#[tauri::command]
fn set_always_on_top(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_always_on_top(enabled)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn create_backup(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<BackupFileInfo, String> {
    let data = state
        .storage
        .export_backup_data(APP_VERSION)
        .map_err(|e| e.to_string())?;
    let file_name = format!(
        "SuperClipboard-backup-{}.scbackup",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let path = backups_dir(&app)?.join(file_name);
    backup::write_backup_data(&path, &data)?;
    backup_file_info(path)
}

#[tauri::command]
fn list_backups(app: tauri::AppHandle) -> Result<Vec<BackupFileInfo>, String> {
    let dir = backups_dir(&app)?;
    let mut backups = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| backup::is_supported_backup_path(path))
        .filter_map(|path| backup_file_info(path).ok())
        .collect::<Vec<_>>();

    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(backups)
}

#[tauri::command]
fn restore_backup(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    file_name: String,
) -> Result<RestoreSummary, String> {
    let path = resolve_backup_path(&app, &file_name)?;
    let data = backup::read_backup_data(&path)?;
    if data.app != "SuperClipboard" {
        return Err("This backup does not belong to SuperClipboard".to_string());
    }
    let summary = state
        .storage
        .restore_backup_data(&data)
        .map_err(|e| e.to_string())?;

    let restored_setting = |key: &str| {
        data.settings
            .iter()
            .find(|setting| setting.key == key)
            .map(|setting| setting.value.as_str())
    };

    if let Some(language) = restored_setting("language") {
        update_tray_menu(&app, language).map_err(|e| e.to_string())?;
    }

    if let Some(shortcut) = restored_setting("shortcut") {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        let old_shortcut = state.current_shortcut.lock().unwrap().clone();
        let new_shortcut = normalize_shortcut(shortcut);
        state.shortcut_recording.store(false, Ordering::SeqCst);
        let _ = app.global_shortcut().unregister(old_shortcut.as_str());
        if register_toggle_shortcut(
            &app,
            new_shortcut.as_str(),
            state.shortcut_recording.clone(),
        )
        .is_ok()
        {
            *state.current_shortcut.lock().unwrap() = new_shortcut;
        } else if !old_shortcut.is_empty() {
            let _ = register_toggle_shortcut(
                &app,
                old_shortcut.as_str(),
                state.shortcut_recording.clone(),
            );
        }
    }

    if let Some(always_on_top) = restored_setting("always_on_top") {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_always_on_top(always_on_top == "true");
        }
    }

    if let Some(autostart) = restored_setting("autostart") {
        let _ = if autostart == "true" {
            autostart::enable()
        } else {
            autostart::disable()
        };
    }

    Ok(summary)
}

#[tauri::command]
fn open_backup_folder(app: tauri::AppHandle) -> Result<(), String> {
    let dir = backups_dir(&app)?;
    open::that(dir).map_err(|e| format!("Failed to open backup folder: {}", e))
}

// ─── Paste Commands ─────────────────────────────────────────

#[tauri::command]
fn paste_to_active_window(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<bool, String> {
    // Copy content to clipboard first
    let entry = if remote_storage::is_remote_mode(&state.storage) {
        remote_storage::get_clipboard_by_id(&state.storage, id).map_err(|e| e.to_string())?
    } else {
        state
            .storage
            .get_entry_by_id(id)
            .map_err(|e| e.to_string())?
    };

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
    } else {
        return Ok(false);
    }

    // Hide the window
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    // Wait for focus to return to previous window, then simulate Ctrl+V
    std::thread::sleep(std::time::Duration::from_millis(150));

    #[cfg(windows)]
    {
        use enigo::{Enigo, Keyboard, Settings};
        let mut enigo =
            Enigo::new(&Settings::default()).map_err(|e| format!("Enigo init failed: {:?}", e))?;
        enigo
            .key(enigo::Key::Control, enigo::Direction::Press)
            .map_err(|e| format!("Key press failed: {:?}", e))?;
        enigo
            .key(enigo::Key::Unicode('v'), enigo::Direction::Click)
            .map_err(|e| format!("Key click failed: {:?}", e))?;
        enigo
            .key(enigo::Key::Control, enigo::Direction::Release)
            .map_err(|e| format!("Key release failed: {:?}", e))?;
    }

    Ok(true)
}

// ─── Memo Commands ──────────────────────────────────────────────

fn memo_body_has_image(body: &str) -> bool {
    body.contains("![image](data:image/")
        || body.contains("![image](http://")
        || body.contains("![image](https://")
        || body.contains("data:image/")
}

#[tauri::command]
fn infer_memo_tag_types(title: String, body: String) -> Vec<String> {
    let content = format!("{}\n{}", title, body);
    let mut tags = Vec::new();

    if memo_body_has_image(&body) {
        tags.push("image".to_string());
    }
    if classifier::contains_email(&content) {
        tags.push("email".to_string());
    }
    if classifier::contains_file_path(&content) {
        tags.push("path".to_string());
    }
    if classifier::contains_link(&content) {
        tags.push("link".to_string());
    }
    if classifier::contains_code(&content) {
        tags.push("code".to_string());
    }

    tags
}

#[tauri::command]
fn get_memos(
    state: tauri::State<'_, AppState>,
    filter: Option<MemoFilter>,
) -> Result<Vec<Memo>, String> {
    let filter = filter.unwrap_or_default();
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::query_memos(&state.storage, &filter).map_err(|e| e.to_string());
    }
    state.storage.get_memos(&filter).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_memo(
    state: tauri::State<'_, AppState>,
    title: String,
    body: String,
    tags: String,
) -> Result<Memo, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::create_memo(&state.storage, &title, &body, &tags)
            .map_err(|e| e.to_string());
    }
    state
        .storage
        .create_memo(&title, &body, &tags)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn update_memo(
    state: tauri::State<'_, AppState>,
    id: i64,
    title: String,
    body: String,
    tags: String,
) -> Result<bool, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::update_memo(&state.storage, id, &title, &body, &tags)
            .map_err(|e| e.to_string());
    }
    state
        .storage
        .update_memo(id, &title, &body, &tags)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_memo(
    state: tauri::State<'_, AppState>,
    id: i64,
    archive: Option<bool>,
) -> Result<bool, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::delete_memo(&state.storage, id, archive.unwrap_or(false))
            .map_err(|e| e.to_string());
    }
    state
        .storage
        .delete_memo(id, archive.unwrap_or(false))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn toggle_memo_pin(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::toggle_memo_pin(&state.storage, id).map_err(|e| e.to_string());
    }
    state.storage.toggle_memo_pin(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn memo_count(state: tauri::State<'_, AppState>) -> Result<i64, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::memo_count(&state.storage).map_err(|e| e.to_string());
    }
    state.storage.memo_count().map_err(|e| e.to_string())
}

#[derive(Deserialize)]
struct ReorderItem {
    id: i64,
    sort_order: i64,
}

#[tauri::command]
fn reorder_memos(
    state: tauri::State<'_, AppState>,
    orders: Vec<ReorderItem>,
) -> Result<(), String> {
    let pairs: Vec<(i64, i64)> = orders.iter().map(|r| (r.id, r.sort_order)).collect();
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::reorder_memos(&state.storage, &pairs).map_err(|e| e.to_string());
    }
    state
        .storage
        .reorder_memos(&pairs)
        .map_err(|e| e.to_string())
}

// ─── Memo Archive Commands ──────────────────────────────────────

#[tauri::command]
fn archive_memo(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::archive_memo(&state.storage, id).map_err(|e| e.to_string());
    }
    state.storage.archive_memo(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn unarchive_memo(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::unarchive_memo(&state.storage, id).map_err(|e| e.to_string());
    }
    state.storage.unarchive_memo(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_archived_memos(
    state: tauri::State<'_, AppState>,
    filter: Option<MemoFilter>,
) -> Result<Vec<Memo>, String> {
    let filter = filter.unwrap_or_default();
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::query_archived_memos(&state.storage, &filter)
            .map_err(|e| e.to_string());
    }
    state
        .storage
        .query_archived_memos(&filter)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn memo_archive_count(state: tauri::State<'_, AppState>) -> Result<i64, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::memo_archive_count(&state.storage).map_err(|e| e.to_string());
    }
    state
        .storage
        .memo_archive_count()
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn permanent_delete_memo(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::permanent_delete_memo(&state.storage, id)
            .map_err(|e| e.to_string());
    }
    state
        .storage
        .permanent_delete_memo(id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn purge_old_memo_archives(state: tauri::State<'_, AppState>, days: i64) -> Result<u64, String> {
    if remote_storage::is_remote_mode(&state.storage) {
        return remote_storage::purge_old_memo_archives(&state.storage, days)
            .map_err(|e| e.to_string());
    }
    state
        .storage
        .purge_old_memo_archives(days)
        .map_err(|e| e.to_string())
}

/// Open a URL in the system default browser
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| format!("Failed to open URL: {}", e))
}

#[tauri::command]
fn test_remote_storage(state: tauri::State<'_, AppState>) -> Result<String, String> {
    remote_storage::test_connection(&state.storage).map_err(|e| e.to_string())
}

#[tauri::command]
fn initialize_remote_storage(state: tauri::State<'_, AppState>) -> Result<(), String> {
    remote_storage::ensure_schema(&state.storage).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_storage_status(state: tauri::State<'_, AppState>) -> storage_backend::StorageStatusInfo {
    storage_backend::status(&state.storage)
}

/// Check for updates from GitHub Releases
#[tauri::command]
async fn check_update() -> Result<UpdateInfo, String> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );

    let client = reqwest::Client::builder()
        .user_agent("SuperClipboard")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let resp: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let tag = resp
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("v0.0.0");

    // Strip leading 'v' or 'V' if present
    let latest = tag
        .strip_prefix('v')
        .or_else(|| tag.strip_prefix('V'))
        .unwrap_or(tag);
    let current = APP_VERSION;

    let has_update = compare_versions(latest, current);

    let download_url = resp
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or(&format!(
            "https://github.com/{}/releases/latest",
            GITHUB_REPO
        ))
        .to_string();
    let release_name = resp
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(tag)
        .to_string();
    let published_at = resp
        .get("published_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let release_notes = resp
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("\n");

    Ok(UpdateInfo {
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        download_url,
        has_update,
        release_name,
        release_notes,
        published_at,
    })
}

/// Compare two semver strings: returns true if `latest` > `current`
fn compare_versions(latest: &str, current: &str) -> bool {
    let parse =
        |s: &str| -> Vec<u64> { s.split('.').filter_map(|p| p.parse::<u64>().ok()).collect() };
    let a = parse(latest);
    let b = parse(current);
    for i in 0..3 {
        let va = a.get(i).copied().unwrap_or(0);
        let vb = b.get(i).copied().unwrap_or(0);
        if va > vb {
            return true;
        }
        if va < vb {
            return false;
        }
    }
    false
}

// ─── App Setup ───────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                show_window(app, &window, "single-instance", false);
            }
        }))
        .setup(|app| {
            // Initialize storage in app data directory
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");
            migrate_legacy_app_data(&app_dir);
            std::fs::create_dir_all(&app_dir).expect("Failed to create app data directory");

            let db_path = app_dir.join("clipboard.db");
            info!("Database path: {:?}", db_path);
            let is_new_database = !db_path.exists();

            let storage = Arc::new(Storage::new(&db_path).expect("Failed to initialize storage"));
            if is_new_database {
                initialize_first_run_defaults(storage.as_ref());
            }

            // Start clipboard monitor
            let mut monitor = ClipboardMonitor::new();
            monitor.start(app.handle().clone(), storage.clone());

            // Read saved shortcut or use default
            let saved_shortcut = storage.get_setting("shortcut").ok().flatten();
            let shortcut = saved_shortcut.unwrap_or_else(|| DEFAULT_SHORTCUT.to_string());
            info!("Global shortcut: {}", shortcut);

            // Read always-on-top setting before moving storage
            let always_on_top = storage
                .get_setting("always_on_top")
                .ok()
                .flatten()
                .map(|v| v == "true")
                .unwrap_or(false);
            let saved_language = storage
                .get_setting("language")
                .ok()
                .flatten()
                .unwrap_or_else(|| detect_system_locale().to_string());

            app.manage(AppState {
                storage: storage.clone(),
                _monitor: std::sync::Mutex::new(monitor),
                current_shortcut: std::sync::Mutex::new(shortcut.clone()),
                shortcut_recording: Arc::new(AtomicBool::new(false)),
            });

            // Register global shortcut to show/hide window
            let shortcut_recording = app.state::<AppState>().shortcut_recording.clone();
            let _ = register_toggle_shortcut(app.handle(), shortcut.as_str(), shortcut_recording);

            // Apply always-on-top setting.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_always_on_top(always_on_top);
                show_window(app.handle(), &window, "startup", true);
            }

            // Set up system tray menu and click handler
            let handle = app.handle().clone();
            if let Some(tray) = app.tray_by_id("main-tray") {
                update_tray_menu(&handle, &saved_language)?;

                // Handle menu item clicks
                tray.on_menu_event(move |app_handle, event| match event.id().as_ref() {
                    "settings" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            show_window(app_handle, &window, "settings", true);
                            let _ = app_handle.emit("open-settings", ());
                        }
                    }
                    "quit" => {
                        app_handle.exit(0);
                    }
                    _ => {}
                });

                // Left-click: show/hide window
                let handle2 = handle.clone();
                tray.on_tray_icon_event(move |_tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(window) = handle2.get_webview_window("main") {
                            toggle_window(&handle2, &window, "tray");
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
            update_entry,
            get_stats,
            clear_unpinned,
            archive_entry,
            unarchive_entry,
            get_archived_entries,
            archive_count,
            permanent_delete,
            purge_old_archives,
            copy_to_clipboard,
            get_setting,
            set_setting,
            get_autostart_enabled,
            set_autostart_enabled,
            get_shortcut,
            set_shortcut,
            set_shortcut_recording,
            infer_memo_tag_types,
            get_memos,
            create_memo,
            update_memo,
            delete_memo,
            toggle_memo_pin,
            memo_count,
            reorder_memos,
            archive_memo,
            unarchive_memo,
            get_archived_memos,
            memo_archive_count,
            permanent_delete_memo,
            purge_old_memo_archives,
            set_always_on_top,
            create_backup,
            list_backups,
            restore_backup,
            open_backup_folder,
            paste_to_active_window,
            check_update,
            open_url,
            test_remote_storage,
            initialize_remote_storage,
            get_storage_status,
        ])
        .run(tauri::generate_context!())
        .expect("Error while running tauri application");
}
