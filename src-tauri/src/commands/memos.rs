use crate::storage::{Memo, MemoFilter, UpdateResult};
use crate::{memo_tags, run_storage, storage_backend, AppState};
use serde::Deserialize;

#[tauri::command]
pub async fn get_memos(
    state: tauri::State<'_, AppState>,
    filter: Option<MemoFilter>,
) -> Result<Vec<Memo>, String> {
    let filter = filter.unwrap_or_default();
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::get_memos(storage, &filter)
    })
    .await
}

#[tauri::command]
pub async fn create_memo(
    state: tauri::State<'_, AppState>,
    title: String,
    body: String,
    tags: String,
) -> Result<Memo, String> {
    let auto_tags = memo_tags::infer(&title, &body);
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::create_memo(storage, &title, &body, &tags, &auto_tags)
    })
    .await
}

#[tauri::command]
pub async fn update_memo(
    state: tauri::State<'_, AppState>,
    id: i64,
    title: String,
    body: String,
    tags: String,
    expected_version: Option<i64>,
) -> Result<UpdateResult, String> {
    let auto_tags = memo_tags::infer(&title, &body);
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::update_memo(
            storage,
            id,
            &title,
            &body,
            &tags,
            &auto_tags,
            expected_version,
        )
    })
    .await
}

#[tauri::command]
pub async fn delete_memo(
    state: tauri::State<'_, AppState>,
    id: i64,
    archive: Option<bool>,
) -> Result<bool, String> {
    let archive = archive.unwrap_or(false);
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::delete_memo(storage, id, archive)
    })
    .await
}

#[tauri::command]
pub async fn toggle_memo_pin(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::toggle_memo_pin(storage, id)
    })
    .await
}

#[tauri::command]
pub async fn memo_count(state: tauri::State<'_, AppState>) -> Result<i64, String> {
    run_storage(state.storage.clone(), storage_backend::memo_count).await
}

#[derive(Deserialize)]
pub struct ReorderItem {
    id: i64,
    sort_order: i64,
}

#[tauri::command]
pub async fn reorder_memos(
    state: tauri::State<'_, AppState>,
    orders: Vec<ReorderItem>,
) -> Result<(), String> {
    let pairs = orders
        .into_iter()
        .map(|item| (item.id, item.sort_order))
        .collect::<Vec<_>>();
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::reorder_memos(storage, &pairs)
    })
    .await
}

#[tauri::command]
pub async fn archive_memo(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::archive_memo(storage, id)
    })
    .await
}

#[tauri::command]
pub async fn unarchive_memo(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::unarchive_memo(storage, id)
    })
    .await
}

#[tauri::command]
pub async fn get_archived_memos(
    state: tauri::State<'_, AppState>,
    filter: Option<MemoFilter>,
) -> Result<Vec<Memo>, String> {
    let filter = filter.unwrap_or_default();
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::get_archived_memos(storage, &filter)
    })
    .await
}

#[tauri::command]
pub async fn memo_archive_count(state: tauri::State<'_, AppState>) -> Result<i64, String> {
    run_storage(state.storage.clone(), storage_backend::memo_archive_count).await
}

#[tauri::command]
pub async fn permanent_delete_memo(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<bool, String> {
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::permanent_delete_memo(storage, id)
    })
    .await
}

#[tauri::command]
pub async fn purge_old_memo_archives(
    state: tauri::State<'_, AppState>,
    days: i64,
) -> Result<u64, String> {
    run_storage(state.storage.clone(), move |storage| {
        storage_backend::purge_old_memo_archives(storage, days)
    })
    .await
}

#[tauri::command]
pub async fn empty_memo_archive(state: tauri::State<'_, AppState>) -> Result<u64, String> {
    run_storage(state.storage.clone(), storage_backend::empty_memo_archive).await
}
