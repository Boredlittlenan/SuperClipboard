mod autostart;
mod backup;
mod classifier;
mod clipboard;
mod commands;
mod memo_tags;
mod remote_storage;
mod search_index;
mod storage;
mod storage_backend;
mod update;
mod window_position;

use clipboard::ClipboardMonitor;
use log::{debug, info, warn};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use storage::{RestoreSummary, Storage};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::Emitter;
use tauri::Manager;
use window_position::{WindowPoint, WindowPositionService, WindowSize};

/// Current application version
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

const DEFAULT_SHORTCUT: &str = "Alt+X";
const DEFAULT_SETTINGS_VERSION: &str = "2";

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
    pub(crate) storage: Arc<Storage>,
    _monitor: std::sync::Mutex<ClipboardMonitor>,
    current_shortcut: std::sync::Mutex<String>,
    shortcut_recording: Arc<AtomicBool>,
    remote_listener: std::sync::Mutex<Option<RemoteListenerHandle>>,
}

struct RemoteListenerHandle {
    stop: Arc<AtomicBool>,
}

fn restart_remote_listener(app: &tauri::AppHandle, state: &AppState) {
    if let Ok(mut listener) = state.remote_listener.lock() {
        if let Some(previous) = listener.take() {
            previous.stop.store(true, Ordering::Relaxed);
        }

        if !remote_storage::is_remote_mode(&state.storage) {
            return;
        }

        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let storage = state.storage.clone();
        let app = app.clone();
        std::thread::spawn(move || {
            let mut retry_delay = Duration::from_secs(1);
            while !worker_stop.load(Ordering::Relaxed) {
                let result = remote_storage::listen_for_changes(
                    storage.as_ref(),
                    worker_stop.as_ref(),
                    |payload| {
                        if let Err(error) = app.emit("remote-storage-changed", payload.to_string())
                        {
                            warn!("Failed to emit remote storage change: {error}");
                        }
                    },
                );

                if worker_stop.load(Ordering::Relaxed) {
                    break;
                }
                if let Err(error) = result {
                    warn!("Remote storage listener disconnected: {error}");
                }
                std::thread::sleep(retry_delay);
                retry_delay = (retry_delay * 2).min(Duration::from_secs(15));
            }
        });
        *listener = Some(RemoteListenerHandle { stop });
    }
}

static REMOTE_TASK_LIMIT: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();

async fn run_remote<T, F>(storage: Arc<Storage>, operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&Storage) -> remote_storage::RemoteResult<T> + Send + 'static,
{
    let task_limit = REMOTE_TASK_LIMIT
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(4)))
        .clone();
    let permit = tokio::time::timeout(Duration::from_secs(2), task_limit.acquire_owned())
        .await
        .map_err(|_| "Remote storage is busy. Try again in a moment.".to_string())?
        .map_err(|_| "Remote storage task queue is unavailable.".to_string())?;
    let started_at = Instant::now();
    let task = tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        operation(storage.as_ref()).map_err(|err| err.to_string())
    });

    let result = tokio::time::timeout(Duration::from_secs(18), task)
        .await
        .map_err(|_| "Remote storage operation timed out.".to_string())?
        .map_err(|err| format!("Remote storage task failed: {err}"))?;
    let elapsed = started_at.elapsed();
    if elapsed > Duration::from_millis(750) {
        debug!("Remote storage operation completed in {elapsed:?}");
    }
    result
}

pub(crate) async fn run_storage<T, F>(storage: Arc<Storage>, operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&Storage) -> Result<T, String> + Send + 'static,
{
    if remote_storage::is_remote_mode(&storage) {
        let task_limit = REMOTE_TASK_LIMIT
            .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(4)))
            .clone();
        let permit = tokio::time::timeout(Duration::from_secs(2), task_limit.acquire_owned())
            .await
            .map_err(|_| "Remote storage is busy. Try again in a moment.".to_string())?
            .map_err(|_| "Remote storage task queue is unavailable.".to_string())?;
        let started_at = Instant::now();
        let task = tauri::async_runtime::spawn_blocking(move || {
            let _permit = permit;
            operation(storage.as_ref())
        });
        let result = tokio::time::timeout(Duration::from_secs(18), task)
            .await
            .map_err(|_| "Remote storage operation timed out.".to_string())?
            .map_err(|err| format!("Storage task failed: {err}"))?;
        let elapsed = started_at.elapsed();
        if elapsed > Duration::from_millis(750) {
            debug!("Remote storage operation completed in {elapsed:?}");
        }
        result
    } else {
        tauri::async_runtime::spawn_blocking(move || operation(storage.as_ref()))
            .await
            .map_err(|err| format!("Storage task failed: {err}"))?
    }
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
    set_default_setting_if_missing(storage, "modern_ui_enabled", "false");
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

/// Get a user setting value by key
#[tauri::command]
fn get_setting(state: tauri::State<'_, AppState>, key: String) -> Result<Option<String>, String> {
    state.storage.get_setting(&key).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_settings(
    state: tauri::State<'_, AppState>,
    keys: Vec<String>,
) -> Result<HashMap<String, String>, String> {
    state.storage.get_settings(&keys).map_err(|e| e.to_string())
}

fn setting_affects_remote_pool(key: &str) -> bool {
    key == "storage_mode"
        || (key.starts_with("remote_db_")
            && key != "remote_db_ready"
            && key != "remote_db_profiles")
}

fn setting_affects_remote_listener(key: &str) -> bool {
    setting_affects_remote_pool(key) || key == "remote_db_ready"
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
        if let Err(error) = update_tray_menu(&app, &value) {
            warn!("Failed to update tray language: {error}");
        }
    }
    if setting_affects_remote_pool(&key) {
        remote_storage::invalidate_pool();
    }
    if setting_affects_remote_listener(&key) {
        restart_remote_listener(&app, &state);
    }
    Ok(())
}

#[tauri::command]
fn set_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    values: HashMap<String, String>,
) -> Result<(), String> {
    state
        .storage
        .set_settings(&values)
        .map_err(|e| e.to_string())?;
    if let Some(language) = values.get("language") {
        if let Err(error) = update_tray_menu(&app, language) {
            warn!("Failed to update tray language: {error}");
        }
    }
    if values.keys().any(|key| setting_affects_remote_pool(key)) {
        remote_storage::invalidate_pool();
    }
    if values
        .keys()
        .any(|key| setting_affects_remote_listener(key))
    {
        restart_remote_listener(&app, &state);
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
async fn paste_to_active_window(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<bool, String> {
    // Copy content to clipboard first
    let entry = run_storage(state.storage.clone(), move |storage| {
        storage_backend::get_entry_by_id(storage, id)
    })
    .await?;

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

/// Open a URL in the system default browser
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| format!("Failed to open URL: {}", e))
}

#[tauri::command]
async fn test_remote_storage(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let storage = state.storage.clone();
    run_remote(storage, remote_storage::test_connection).await
}

#[tauri::command]
async fn initialize_remote_storage(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let storage = state.storage.clone();
    run_remote(storage, remote_storage::ensure_schema).await
}

#[tauri::command]
async fn get_storage_status(
    state: tauri::State<'_, AppState>,
) -> Result<storage_backend::StorageStatusInfo, String> {
    run_storage(state.storage.clone(), |storage| {
        Ok(storage_backend::status(storage))
    })
    .await
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

            // Remote writes can begin as soon as clipboard monitoring starts, so schema
            // migrations must finish before the monitor and notification listener run.
            if remote_storage::is_remote_mode(storage.as_ref())
                && !remote_storage::is_schema_current(storage.as_ref())
            {
                info!("Checking remote storage schema before startup");
                if let Err(error) = remote_storage::ensure_schema(storage.as_ref()) {
                    warn!("Failed to initialize remote storage schema: {error:?}");
                }
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
                remote_listener: std::sync::Mutex::new(None),
            });

            let state = app.state::<AppState>();
            restart_remote_listener(app.handle(), &state);

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
            commands::clipboard::get_entries,
            commands::clipboard::get_entry_content,
            commands::clipboard::delete_entry,
            commands::clipboard::toggle_pin,
            commands::clipboard::update_entry,
            commands::clipboard::get_stats,
            commands::clipboard::clear_unpinned,
            commands::clipboard::archive_entry,
            commands::clipboard::unarchive_entry,
            commands::clipboard::get_archived_entries,
            commands::clipboard::archive_count,
            commands::clipboard::permanent_delete,
            commands::clipboard::purge_old_archives,
            commands::clipboard::copy_to_clipboard,
            get_setting,
            get_settings,
            set_setting,
            set_settings,
            get_autostart_enabled,
            set_autostart_enabled,
            get_shortcut,
            set_shortcut,
            set_shortcut_recording,
            memo_tags::infer_memo_tag_types,
            commands::memos::get_memos,
            commands::memos::create_memo,
            commands::memos::update_memo,
            commands::memos::delete_memo,
            commands::memos::toggle_memo_pin,
            commands::memos::memo_count,
            commands::memos::reorder_memos,
            commands::memos::archive_memo,
            commands::memos::unarchive_memo,
            commands::memos::get_archived_memos,
            commands::memos::memo_archive_count,
            commands::memos::permanent_delete_memo,
            commands::memos::purge_old_memo_archives,
            set_always_on_top,
            create_backup,
            list_backups,
            restore_backup,
            open_backup_folder,
            paste_to_active_window,
            update::check_update,
            open_url,
            test_remote_storage,
            initialize_remote_storage,
            get_storage_status,
        ])
        .run(tauri::generate_context!())
        .expect("Error while running tauri application");
}
