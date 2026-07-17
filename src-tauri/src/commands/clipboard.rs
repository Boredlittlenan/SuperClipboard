use crate::classifier;
use crate::clipboard;
use crate::storage::{ClipboardEntry, QueryFilter, UpdateResult};
use crate::{memo_tags, run_storage, storage_backend, AppState};
use serde::Serialize;
use tauri::Emitter;

const MAX_DROPPED_TEXT_BYTES: usize = 2 * 1024 * 1024;
const MAX_DROPPED_IMAGE_BYTES: usize = 20 * 1024 * 1024;
const MAX_DROPPED_IMAGE_RGBA_BYTES: usize = 64 * 1024 * 1024;
const MAX_MERGE_ITEMS: usize = 20;
const MAX_BATCH_DELETE_ITEMS: usize = 500;
const MAX_MERGED_TEXT_BYTES: usize = 4 * 1024 * 1024;
const MAX_MERGED_IMAGE_BYTES: usize = 50 * 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeEntriesResult {
    kind: String,
    created: bool,
    deleted_originals: u64,
}

#[derive(Debug, PartialEq, Eq)]
enum MergePayload {
    Text(String),
    ImageMemo(String),
}

fn build_merge_payload(entries: &[ClipboardEntry]) -> Result<MergePayload, String> {
    let first = entries
        .first()
        .ok_or_else(|| "Select at least two clipboard entries.".to_string())?;
    if entries.len() < 2 {
        return Err("Select at least two clipboard entries.".to_string());
    }
    if entries.iter().any(|entry| entry.archived_at.is_some()) {
        return Err("Archived entries cannot be merged.".to_string());
    }
    if entries.iter().any(|entry| entry.category != first.category) {
        return Err("Only clipboard entries of the same type can be merged.".to_string());
    }

    if first.category == classifier::Category::Image {
        let total_bytes = entries
            .iter()
            .map(|entry| entry.content.len())
            .sum::<usize>();
        if total_bytes > MAX_MERGED_IMAGE_BYTES {
            return Err("The selected images are too large to merge (maximum 50 MB).".to_string());
        }
        let body = entries
            .iter()
            .map(|entry| {
                format!(
                    "![image](data:{};base64,{})",
                    entry.content_type, entry.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        return Ok(MergePayload::ImageMemo(body));
    }

    let total_bytes = entries
        .iter()
        .map(|entry| entry.content.len())
        .sum::<usize>()
        .saturating_add(entries.len().saturating_sub(1));
    if total_bytes > MAX_MERGED_TEXT_BYTES {
        return Err("The merged text is too large (maximum 4 MB).".to_string());
    }
    let content = entries
        .iter()
        .map(|entry| entry.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if content.trim().is_empty() {
        return Err("The merged text is empty.".to_string());
    }
    Ok(MergePayload::Text(content))
}

#[tauri::command]
pub async fn merge_entries(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    ids: Vec<i64>,
    memo_title: String,
    delete_originals: Option<bool>,
    archive_originals: Option<bool>,
) -> Result<MergeEntriesResult, String> {
    let delete_originals = delete_originals.unwrap_or(false);
    let archive_originals = archive_originals.unwrap_or(false);
    let mut unique_ids = Vec::with_capacity(ids.len());
    for id in ids {
        if !unique_ids.contains(&id) {
            unique_ids.push(id);
        }
    }
    if !(2..=MAX_MERGE_ITEMS).contains(&unique_ids.len()) {
        return Err(format!(
            "Select between 2 and {MAX_MERGE_ITEMS} clipboard entries."
        ));
    }

    let (result, created_entry) = run_storage(state.storage.clone(), move |storage| {
        let entries = storage_backend::get_entries_by_ids(storage, &unique_ids)?;
        if entries.len() != unique_ids.len() {
            return Err("Some selected clipboard entries are no longer available.".to_string());
        }

        let (mut result, created_entry) = match build_merge_payload(&entries)? {
            MergePayload::Text(content) => {
                let entry = clipboard::make_text_entry(content);
                let created = storage_backend::insert_entry(storage, &entry)?;
                (
                    MergeEntriesResult {
                        kind: "clipboard".to_string(),
                        created,
                        deleted_originals: 0,
                    },
                    created.then_some(entry),
                )
            }
            MergePayload::ImageMemo(body) => {
                let title = memo_title.trim();
                if title.is_empty() {
                    return Err("A title is required for the merged image memo.".to_string());
                }
                let auto_tags = memo_tags::infer(title, &body);
                storage_backend::create_memo(storage, title, &body, "", &auto_tags)?;
                (
                    MergeEntriesResult {
                        kind: "memo".to_string(),
                        created: true,
                        deleted_originals: 0,
                    },
                    None,
                )
            }
        };
        if delete_originals {
            result.deleted_originals =
                storage_backend::delete_entries(storage, &unique_ids, archive_originals)?;
        }
        Ok((result, created_entry))
    })
    .await?;

    if let Some(entry) = created_entry {
        let _ = app.emit("clipboard-changed", entry);
    }
    Ok(result)
}

#[tauri::command]
pub async fn delete_entries(
    state: tauri::State<'_, AppState>,
    ids: Vec<i64>,
    archive: Option<bool>,
) -> Result<u64, String> {
    let mut unique_ids = Vec::with_capacity(ids.len());
    for id in ids {
        if !unique_ids.contains(&id) {
            unique_ids.push(id);
        }
    }
    if unique_ids.is_empty() {
        return Err("Select at least one clipboard entry.".to_string());
    }
    if unique_ids.len() > MAX_BATCH_DELETE_ITEMS {
        return Err(format!(
            "Up to {MAX_BATCH_DELETE_ITEMS} clipboard entries can be deleted at once."
        ));
    }
    let archive = archive.unwrap_or(false);
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::delete_entries(storage, &unique_ids, archive)
    })
    .await
}

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
pub async fn get_stats(
    state: tauri::State<'_, AppState>,
    include_auxiliary_tags: Option<bool>,
) -> Result<serde_json::Value, String> {
    let include_auxiliary_tags = include_auxiliary_tags.unwrap_or(false);
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::get_stats(storage, include_auxiliary_tags)
    })
    .await
}

#[tauri::command]
pub async fn reclassify_clipboard_entries(
    state: tauri::State<'_, AppState>,
) -> Result<u64, String> {
    run_storage(
        state.storage.clone(),
        storage_backend::reclassify_clipboard_entries,
    )
    .await
}

#[tauri::command]
pub async fn get_classification_status(
    state: tauri::State<'_, AppState>,
) -> Result<storage_backend::ClassificationStatusInfo, String> {
    run_storage(
        state.storage.clone(),
        storage_backend::classification_status,
    )
    .await
}

#[tauri::command]
pub async fn clear_unpinned(
    state: tauri::State<'_, AppState>,
    archive: Option<bool>,
    category: Option<String>,
    include_auxiliary_tags: Option<bool>,
) -> Result<u64, String> {
    let archive = archive.unwrap_or(false);
    let include_auxiliary_tags = include_auxiliary_tags.unwrap_or(false);
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::clear_unpinned(
            storage,
            archive,
            category.as_deref(),
            include_auxiliary_tags,
        )
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
pub async fn empty_archive(state: tauri::State<'_, AppState>) -> Result<u64, String> {
    run_storage(state.storage.clone(), storage_backend::empty_archive).await
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn entry(id: i64, category: classifier::Category, content: &str) -> ClipboardEntry {
        ClipboardEntry {
            id,
            category: category.clone(),
            category_tags: vec![category.clone()],
            content_type: if category == classifier::Category::Image {
                "image/png".to_string()
            } else {
                "text/plain".to_string()
            },
            content: content.to_string(),
            preview: content.to_string(),
            hash: id.to_string(),
            pinned: false,
            created_at: Utc::now(),
            original_content: None,
            updated_at: None,
            archived_at: None,
            version: 1,
        }
    }

    #[test]
    fn merged_text_keeps_the_requested_display_order() {
        let entries = vec![
            entry(2, classifier::Category::Text, "second"),
            entry(1, classifier::Category::Text, "first"),
        ];
        assert_eq!(
            build_merge_payload(&entries).unwrap(),
            MergePayload::Text("second\nfirst".to_string())
        );
    }

    #[test]
    fn different_primary_categories_cannot_be_merged() {
        let entries = vec![
            entry(1, classifier::Category::Text, "text"),
            entry(2, classifier::Category::Link, "https://example.com"),
        ];
        assert!(build_merge_payload(&entries).is_err());
    }

    #[test]
    fn images_are_serialized_as_an_editable_memo_body() {
        let entries = vec![
            entry(1, classifier::Category::Image, "aaa"),
            entry(2, classifier::Category::Image, "bbb"),
        ];
        assert_eq!(
            build_merge_payload(&entries).unwrap(),
            MergePayload::ImageMemo(
                "![image](data:image/png;base64,aaa)\n![image](data:image/png;base64,bbb)"
                    .to_string()
            )
        );
    }
}
