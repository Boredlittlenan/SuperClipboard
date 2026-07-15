use crate::classifier;
use crate::storage::{ClipboardEntry, QueryFilter, UpdateResult};
use crate::{run_storage, storage_backend, AppState};

#[tauri::command]
pub async fn get_entries(
    state: tauri::State<'_, AppState>,
    filter: Option<QueryFilter>,
) -> Result<Vec<ClipboardEntry>, String> {
    let filter = filter.unwrap_or_default();
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::get_entries(storage, &filter)
    })
    .await
}

#[tauri::command]
pub async fn get_entry_content(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<Option<String>, String> {
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::get_entry_by_id(storage, id).map(|entry| entry.map(|item| item.content))
    })
    .await
}

#[tauri::command]
pub async fn delete_entry(
    state: tauri::State<'_, AppState>,
    id: i64,
    archive: Option<bool>,
) -> Result<bool, String> {
    let archive = archive.unwrap_or(false);
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::delete_entry(storage, id, archive)
    })
    .await
}

#[tauri::command]
pub async fn toggle_pin(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::toggle_pin(storage, id)
    })
    .await
}

#[tauri::command]
pub async fn update_entry(
    state: tauri::State<'_, AppState>,
    id: i64,
    content: String,
    expected_version: Option<i64>,
) -> Result<UpdateResult, String> {
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::update_entry(storage, id, &content, expected_version)
    })
    .await
}

#[tauri::command]
pub async fn get_stats(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    run_storage(state.storage.clone(), storage_backend::get_stats).await
}

#[tauri::command]
pub async fn clear_unpinned(
    state: tauri::State<'_, AppState>,
    archive: Option<bool>,
) -> Result<u64, String> {
    let archive = archive.unwrap_or(false);
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::clear_unpinned(storage, archive)
    })
    .await
}

#[tauri::command]
pub async fn archive_entry(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::archive_entry(storage, id)
    })
    .await
}

#[tauri::command]
pub async fn unarchive_entry(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::unarchive_entry(storage, id)
    })
    .await
}

#[tauri::command]
pub async fn get_archived_entries(
    state: tauri::State<'_, AppState>,
    filter: Option<QueryFilter>,
) -> Result<Vec<ClipboardEntry>, String> {
    let filter = filter.unwrap_or_default();
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::get_archived_entries(storage, &filter)
    })
    .await
}

#[tauri::command]
pub async fn archive_count(state: tauri::State<'_, AppState>) -> Result<i64, String> {
    run_storage(state.storage.clone(), storage_backend::archive_count).await
}

#[tauri::command]
pub async fn permanent_delete(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::permanent_delete(storage, id)
    })
    .await
}

#[tauri::command]
pub async fn purge_old_archives(
    state: tauri::State<'_, AppState>,
    days: i64,
) -> Result<u64, String> {
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::purge_old_archives(storage, days)
    })
    .await
}

#[tauri::command]
pub async fn copy_to_clipboard(
    state: tauri::State<'_, AppState>,
    id: i64,
    use_original: Option<bool>,
) -> Result<bool, String> {
    let entry = run_storage(state.storage.clone(), move |storage| {
        storage_backend::get_entry_by_id(storage, id)
    })
    .await?;

    if let Some(entry) = entry {
        let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
        match entry.category {
            classifier::Category::Image => {
                use base64::Engine;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(&entry.content)
                    .map_err(|error| error.to_string())?;
                let image = image::load_from_memory(&bytes).map_err(|error| error.to_string())?;
                let rgba = image.to_rgba8();
                let (width, height) = rgba.dimensions();
                clipboard
                    .set_image(arboard::ImageData {
                        width: width as usize,
                        height: height as usize,
                        bytes: rgba.into_raw().into(),
                    })
                    .map_err(|error| error.to_string())?;
            }
            _ => {
                let text = if use_original.unwrap_or(false) {
                    entry.original_content.as_deref().unwrap_or(&entry.content)
                } else {
                    &entry.content
                };
                clipboard
                    .set_text(text)
                    .map_err(|error| error.to_string())?;
            }
        }
        Ok(true)
    } else {
        Ok(false)
    }
}
