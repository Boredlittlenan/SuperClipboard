use crate::remote_storage;
use crate::storage::{ClipboardEntry, Memo, MemoFilter, QueryFilter, Storage, UpdateResult};
use serde::Serialize;

pub type BackendResult<T> = Result<T, String>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStatusInfo {
    pub mode: String,
    pub health: String,
    pub message: String,
}

fn remote(storage: &Storage) -> bool {
    remote_storage::is_remote_mode(storage)
}

pub fn get_entries(storage: &Storage, filter: &QueryFilter) -> BackendResult<Vec<ClipboardEntry>> {
    if remote(storage) {
        remote_storage::query_clipboard(storage, filter).map_err(|error| error.to_string())
    } else {
        storage.query(filter).map_err(|error| error.to_string())
    }
}

pub fn delete_entry(storage: &Storage, id: i64, archive: bool) -> BackendResult<bool> {
    if remote(storage) {
        remote_storage::delete_clipboard(storage, id, archive).map_err(|error| error.to_string())
    } else if archive {
        storage.archive_entry(id).map_err(|error| error.to_string())
    } else {
        storage.delete(id).map_err(|error| error.to_string())
    }
}

pub fn toggle_pin(storage: &Storage, id: i64) -> BackendResult<bool> {
    if remote(storage) {
        remote_storage::toggle_clipboard_pin(storage, id).map_err(|error| error.to_string())
    } else {
        storage.toggle_pin(id).map_err(|error| error.to_string())
    }
}

pub fn update_entry(
    storage: &Storage,
    id: i64,
    content: &str,
    expected_version: Option<i64>,
) -> BackendResult<UpdateResult> {
    if remote(storage) {
        remote_storage::update_clipboard(storage, id, content, expected_version)
            .map_err(|error| error.to_string())
    } else {
        storage
            .update_entry(id, content)
            .map(UpdateResult::updated)
            .map_err(|error| error.to_string())
    }
}

pub fn get_stats(storage: &Storage) -> BackendResult<serde_json::Value> {
    if remote(storage) {
        let stats = remote_storage::stats(storage).map_err(|error| error.to_string())?;
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

    Ok(serde_json::json!({
        "total": storage.count(None).map_err(|error| error.to_string())?,
        "text": storage.count(Some("text")).map_err(|error| error.to_string())?,
        "link": storage.count(Some("link")).map_err(|error| error.to_string())?,
        "image": storage.count(Some("image")).map_err(|error| error.to_string())?,
        "code": storage.count(Some("code")).map_err(|error| error.to_string())?,
        "email": storage.count(Some("email")).map_err(|error| error.to_string())?,
        "file_path": storage.count(Some("file_path")).map_err(|error| error.to_string())?,
        "dbSize": storage.db_size().map_err(|error| error.to_string())?,
        "clipboardSize": storage.clipboard_storage_size().map_err(|error| error.to_string())?,
        "memoSize": storage.memo_storage_size().map_err(|error| error.to_string())?,
        "archive": storage.archive_count().map_err(|error| error.to_string())?,
        "memoCount": storage.memo_count().map_err(|error| error.to_string())?,
        "memoArchive": storage.memo_archive_count().map_err(|error| error.to_string())?,
    }))
}

pub fn clear_unpinned(storage: &Storage, archive: bool) -> BackendResult<u64> {
    if remote(storage) {
        remote_storage::clear_clipboard_unpinned(storage, archive)
            .map_err(|error| error.to_string())
    } else {
        storage
            .clear_unpinned(archive)
            .map_err(|error| error.to_string())
    }
}

pub fn archive_entry(storage: &Storage, id: i64) -> BackendResult<bool> {
    if remote(storage) {
        remote_storage::archive_clipboard(storage, id).map_err(|error| error.to_string())
    } else {
        storage.archive_entry(id).map_err(|error| error.to_string())
    }
}

pub fn unarchive_entry(storage: &Storage, id: i64) -> BackendResult<bool> {
    if remote(storage) {
        remote_storage::unarchive_clipboard(storage, id).map_err(|error| error.to_string())
    } else {
        storage
            .unarchive_entry(id)
            .map_err(|error| error.to_string())
    }
}

pub fn get_archived_entries(
    storage: &Storage,
    filter: &QueryFilter,
) -> BackendResult<Vec<ClipboardEntry>> {
    if remote(storage) {
        remote_storage::query_archived_clipboard(storage, filter).map_err(|error| error.to_string())
    } else {
        storage
            .query_archived(filter)
            .map_err(|error| error.to_string())
    }
}

pub fn archive_count(storage: &Storage) -> BackendResult<i64> {
    if remote(storage) {
        remote_storage::clipboard_archive_count(storage).map_err(|error| error.to_string())
    } else {
        storage.archive_count().map_err(|error| error.to_string())
    }
}

pub fn permanent_delete(storage: &Storage, id: i64) -> BackendResult<bool> {
    if remote(storage) {
        remote_storage::permanent_delete_clipboard(storage, id).map_err(|error| error.to_string())
    } else {
        storage
            .permanent_delete(id)
            .map_err(|error| error.to_string())
    }
}

pub fn purge_old_archives(storage: &Storage, days: i64) -> BackendResult<u64> {
    if remote(storage) {
        remote_storage::purge_old_clipboard_archives(storage, days)
            .map_err(|error| error.to_string())
    } else {
        storage
            .purge_old_archives(days)
            .map_err(|error| error.to_string())
    }
}

pub fn get_entry_by_id(storage: &Storage, id: i64) -> BackendResult<Option<ClipboardEntry>> {
    if remote(storage) {
        remote_storage::get_clipboard_by_id(storage, id).map_err(|error| error.to_string())
    } else {
        storage
            .get_entry_by_id(id)
            .map_err(|error| error.to_string())
    }
}

pub fn get_memos(storage: &Storage, filter: &MemoFilter) -> BackendResult<Vec<Memo>> {
    if remote(storage) {
        remote_storage::query_memos(storage, filter).map_err(|error| error.to_string())
    } else {
        storage.get_memos(filter).map_err(|error| error.to_string())
    }
}

pub fn create_memo(
    storage: &Storage,
    title: &str,
    body: &str,
    tags: &str,
    auto_tags: &[String],
) -> BackendResult<Memo> {
    if remote(storage) {
        remote_storage::create_memo(storage, title, body, tags, auto_tags)
            .map_err(|error| error.to_string())
    } else {
        storage
            .create_memo(title, body, tags, auto_tags)
            .map_err(|error| error.to_string())
    }
}

pub fn update_memo(
    storage: &Storage,
    id: i64,
    title: &str,
    body: &str,
    tags: &str,
    auto_tags: &[String],
    expected_version: Option<i64>,
) -> BackendResult<UpdateResult> {
    if remote(storage) {
        remote_storage::update_memo(storage, id, title, body, tags, auto_tags, expected_version)
            .map_err(|error| error.to_string())
    } else {
        storage
            .update_memo(id, title, body, tags, auto_tags)
            .map(UpdateResult::updated)
            .map_err(|error| error.to_string())
    }
}

pub fn delete_memo(storage: &Storage, id: i64, archive: bool) -> BackendResult<bool> {
    if remote(storage) {
        remote_storage::delete_memo(storage, id, archive).map_err(|error| error.to_string())
    } else {
        storage
            .delete_memo(id, archive)
            .map_err(|error| error.to_string())
    }
}

pub fn toggle_memo_pin(storage: &Storage, id: i64) -> BackendResult<bool> {
    if remote(storage) {
        remote_storage::toggle_memo_pin(storage, id).map_err(|error| error.to_string())
    } else {
        storage
            .toggle_memo_pin(id)
            .map_err(|error| error.to_string())
    }
}

pub fn memo_count(storage: &Storage) -> BackendResult<i64> {
    if remote(storage) {
        remote_storage::memo_count(storage).map_err(|error| error.to_string())
    } else {
        storage.memo_count().map_err(|error| error.to_string())
    }
}

pub fn reorder_memos(storage: &Storage, orders: &[(i64, i64)]) -> BackendResult<()> {
    if remote(storage) {
        remote_storage::reorder_memos(storage, orders).map_err(|error| error.to_string())
    } else {
        storage
            .reorder_memos(orders)
            .map_err(|error| error.to_string())
    }
}

pub fn archive_memo(storage: &Storage, id: i64) -> BackendResult<bool> {
    if remote(storage) {
        remote_storage::archive_memo(storage, id).map_err(|error| error.to_string())
    } else {
        storage.archive_memo(id).map_err(|error| error.to_string())
    }
}

pub fn unarchive_memo(storage: &Storage, id: i64) -> BackendResult<bool> {
    if remote(storage) {
        remote_storage::unarchive_memo(storage, id).map_err(|error| error.to_string())
    } else {
        storage
            .unarchive_memo(id)
            .map_err(|error| error.to_string())
    }
}

pub fn get_archived_memos(storage: &Storage, filter: &MemoFilter) -> BackendResult<Vec<Memo>> {
    if remote(storage) {
        remote_storage::query_archived_memos(storage, filter).map_err(|error| error.to_string())
    } else {
        storage
            .query_archived_memos(filter)
            .map_err(|error| error.to_string())
    }
}

pub fn memo_archive_count(storage: &Storage) -> BackendResult<i64> {
    if remote(storage) {
        remote_storage::memo_archive_count(storage).map_err(|error| error.to_string())
    } else {
        storage
            .memo_archive_count()
            .map_err(|error| error.to_string())
    }
}

pub fn permanent_delete_memo(storage: &Storage, id: i64) -> BackendResult<bool> {
    if remote(storage) {
        remote_storage::permanent_delete_memo(storage, id).map_err(|error| error.to_string())
    } else {
        storage
            .permanent_delete_memo(id)
            .map_err(|error| error.to_string())
    }
}

pub fn purge_old_memo_archives(storage: &Storage, days: i64) -> BackendResult<u64> {
    if remote(storage) {
        remote_storage::purge_old_memo_archives(storage, days).map_err(|error| error.to_string())
    } else {
        storage
            .purge_old_memo_archives(days)
            .map_err(|error| error.to_string())
    }
}

pub fn status(storage: &Storage) -> StorageStatusInfo {
    let configured_mode = storage
        .get_setting("storage_mode")
        .ok()
        .flatten()
        .unwrap_or_else(|| "local".to_string());

    if configured_mode != "remote" {
        return StorageStatusInfo {
            mode: "local".to_string(),
            health: "local".to_string(),
            message: String::new(),
        };
    }

    let ready = matches!(
        storage.get_setting("remote_db_ready"),
        Ok(Some(value)) if value == "true"
    );
    if !ready {
        return StorageStatusInfo {
            mode: "remote".to_string(),
            health: "notReady".to_string(),
            message: String::new(),
        };
    }

    match remote_storage::test_connection(storage) {
        Ok(message) => StorageStatusInfo {
            mode: "remote".to_string(),
            health: "connected".to_string(),
            message,
        },
        Err(error) => StorageStatusInfo {
            mode: "remote".to_string(),
            health: "failed".to_string(),
            message: error.to_string(),
        },
    }
}
