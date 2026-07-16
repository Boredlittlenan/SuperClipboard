use crate::classifier;
use crate::clipboard;
use crate::storage::{ClipboardEntry, QueryFilter, UpdateResult};
use crate::{run_storage, storage_backend, AppState};
use tauri::Emitter;

const MAX_DROPPED_TEXT_BYTES: usize = 2 * 1024 * 1024;
const MAX_DROPPED_IMAGE_BYTES: usize = 20 * 1024 * 1024;
const MAX_DROPPED_IMAGE_RGBA_BYTES: usize = 64 * 1024 * 1024;

#[tauri::command]
pub async fn import_dropped_text(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    text: String,
) -> Result<bool, String> {
    if text.trim().is_empty() {
        return Err("Dropped text is empty.".to_string());
    }
    if text.len() > MAX_DROPPED_TEXT_BYTES {
        return Err("Dropped text is too large (maximum 2 MB).".to_string());
    }

    let mut system_clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
    system_clipboard
        .set_text(&text)
        .map_err(|error| error.to_string())?;

    let entry = clipboard::make_text_entry(text);
    let entry_to_store = entry.clone();
    let inserted = run_storage(state.storage.clone(), move |storage| {
        storage_backend::insert_entry(storage, &entry_to_store)
    })
    .await?;
    if inserted {
        let _ = app.emit("clipboard-changed", entry);
    }
    Ok(inserted)
}

#[tauri::command]
pub async fn import_dropped_image(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    data_url: String,
) -> Result<bool, String> {
    let (image_data, encoded_png) = decode_dropped_image(&data_url)?;
    let image_hash = image_hash_payload(&image_data);
    let entry = clipboard::make_image_entry(
        encoded_png,
        image_data.width,
        image_data.height,
        &image_hash,
    );

    let mut system_clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
    system_clipboard
        .set_image(image_data.clone())
        .map_err(|error| error.to_string())?;

    let entry_to_store = entry.clone();
    let inserted = run_storage(state.storage.clone(), move |storage| {
        storage_backend::insert_entry(storage, &entry_to_store)
    })
    .await?;
    if inserted {
        let _ = app.emit("clipboard-changed", entry);
    }
    Ok(inserted)
}

fn decode_dropped_image(data_url: &str) -> Result<(arboard::ImageData<'static>, String), String> {
    use base64::Engine;

    if data_url.len() > MAX_DROPPED_IMAGE_BYTES.saturating_mul(2) {
        return Err("Dropped image is too large (maximum 20 MB).".to_string());
    }

    let (header, encoded) = data_url
        .split_once(',')
        .ok_or_else(|| "Invalid dropped image data.".to_string())?;
    if !header.starts_with("data:image/") || !header.ends_with(";base64") {
        return Err("Dropped content is not a supported image.".to_string());
    }

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| format!("Invalid dropped image data: {error}"))?;
    if bytes.len() > MAX_DROPPED_IMAGE_BYTES {
        return Err("Dropped image is too large (maximum 20 MB).".to_string());
    }

    let image = image::load_from_memory(&bytes)
        .map_err(|error| format!("Could not decode dropped image: {error}"))?;
    let rgba = image.to_rgba8();
    if rgba.len() > MAX_DROPPED_IMAGE_RGBA_BYTES {
        return Err("Dropped image dimensions are too large.".to_string());
    }
    let (width, height) = rgba.dimensions();

    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|error| format!("Could not normalize dropped image: {error}"))?;

    Ok((
        arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: rgba.into_raw().into(),
        },
        base64::engine::general_purpose::STANDARD.encode(png),
    ))
}

fn image_hash_payload(image: &arboard::ImageData<'_>) -> String {
    let mut bytes = Vec::with_capacity(image.bytes.len() + 32);
    bytes.extend_from_slice(b"image/png-raw");
    bytes.extend_from_slice(&(image.width as u64).to_le_bytes());
    bytes.extend_from_slice(&(image.height as u64).to_le_bytes());
    bytes.extend_from_slice(&image.bytes);
    crate::storage::Storage::hash_bytes(&bytes)
}

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
pub async fn export_clipboard_image(
    state: tauri::State<'_, AppState>,
    id: i64,
    path: String,
) -> Result<(), String> {
    let entry = run_storage(state.storage.clone(), move |storage| {
        storage_backend::get_entry_by_id(storage, id)
    })
    .await?
    .ok_or_else(|| "Clipboard entry was not found.".to_string())?;

    if entry.category != classifier::Category::Image {
        return Err("Only image clipboard entries can be exported.".to_string());
    }

    save_image_as_png(&entry.content, &path)
}

fn save_image_as_png(content: &str, path: &str) -> Result<(), String> {
    use base64::Engine;
    use std::path::PathBuf;

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(content)
        .map_err(|error| format!("Invalid image data: {error}"))?;
    let image = image::load_from_memory(&bytes)
        .map_err(|error| format!("Could not decode clipboard image: {error}"))?;

    let mut output_path = PathBuf::from(path);
    if output_path.extension().is_none() {
        output_path.set_extension("png");
    }
    image
        .save_with_format(output_path, image::ImageFormat::Png)
        .map_err(|error| format!("Could not save image: {error}"))
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
