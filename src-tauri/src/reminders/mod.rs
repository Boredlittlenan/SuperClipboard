mod clock;
mod platform;
mod store;

use crate::{run_storage, storage_backend, workspace_config, AppState};
use clock::Repeat;
use std::path::{Path, PathBuf};
use std::time::Duration;
use store::Reminder;
use tauri::Emitter;

pub fn start(app: tauri::AppHandle, path: PathBuf) {
    std::thread::spawn(move || {
        if matches!(store::needs_schedule(&path), Ok(true)) {
            if let Err(error) = platform::ensure_schedule(&path) {
                log::warn!("Could not register reminder scheduler: {error}");
            }
        }
        let mut offset = clock::zone_offset();
        loop {
            let new_offset = clock::zone_offset();
            if new_offset != offset && store::recalculate(&path).is_ok() {
                offset = new_offset;
            }
            match store::tick(&path, platform::notify) {
                Ok(true) => {
                    let _ = app.emit("reminders-changed", ());
                }
                Ok(false) => {}
                Err(error) => log::warn!("Reminder worker: {error}"),
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    });
}

pub fn headless(path: &Path) -> Result<(), String> {
    if path.is_file() {
        store::tick(path, platform::notify)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn list_reminders(state: tauri::State<'_, AppState>) -> Result<Vec<Reminder>, String> {
    let path = state.reminder_path.clone();
    run_storage(state.storage.clone(), move |storage| {
        store::list(&path, &workspace_config::id(storage)?)
    })
    .await
}

#[tauri::command]
pub async fn save_reminder(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    memo_id: i64,
    at: String,
    repeat: Repeat,
) -> Result<(), String> {
    let path = state.reminder_path.clone();
    run_storage(state.storage.clone(), move |storage| {
        let memo = storage_backend::get_memo(storage, memo_id)?
            .filter(|memo| memo.archived_at.is_none())
            .ok_or("The memo is unavailable")?;
        let title = if memo.title.trim().is_empty() {
            "Memo reminder"
        } else {
            &memo.title
        };
        platform::ensure_schedule(&path)?;
        store::save(
            &path,
            &workspace_config::id(storage)?,
            memo_id,
            title,
            &at,
            repeat,
        )
    })
    .await?;
    let _ = app.emit("reminders-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn finish_reminder(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    memo_id: i64,
    completed: bool,
) -> Result<(), String> {
    let path = state.reminder_path.clone();
    run_storage(state.storage.clone(), move |storage| {
        store::finish(
            &path,
            &workspace_config::id(storage)?,
            memo_id,
            if completed { "completed" } else { "cancelled" },
        )
    })
    .await?;
    let _ = app.emit("reminders-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn test_reminder_notification() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| platform::notify("SuperClipboard reminder test"))
        .await
        .map_err(|e| e.to_string())?
}

pub use store::{finish, refresh_title};
