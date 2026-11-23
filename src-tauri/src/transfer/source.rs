use crate::classifier::Category;
use crate::storage::{BackupData, ClipboardEntry, Memo, Storage};
use chrono::{DateTime, NaiveDateTime, Utc};
use rusqlite::{Connection, OpenFlags};
use std::collections::HashSet;
use std::io::Read;
use std::path::Path;

const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;

pub fn read_file(path: &Path) -> Result<(String, BackupData), String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut header = [0; 16];
    (&file)
        .read_exact(&mut header)
        .map_err(|error| error.to_string())?;
    if &header == b"SQLite format 3\0" {
        return read_sqlite(path).map(|data| ("SQLite".into(), data));
    }
    if header.starts_with(b"PK\x03\x04") {
        return crate::backup::read_backup_data(path).map(|data| ("SCBackup".into(), data));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("Backup exceeds 512 MiB".into());
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    let backup = serde_json::from_slice(bytes)
        .map_err(|error| format!("Unrecognized SuperClipboard backup: {error}"))?;
    Ok(("JSON".into(), backup))
}

pub fn timestamp(value: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value)
        .or_else(|_| DateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f%#z"))
        .map(|time| time.with_timezone(&Utc))
        .or_else(|_| {
            NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f").map(|time| time.and_utc())
        })
        .map_err(|_| format!("Invalid timestamp: {value}"))
}

fn columns(conn: &Connection, table: &str) -> rusqlite::Result<HashSet<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let names = stmt.query_map([], |row| row.get(1))?.collect();
    names
}

fn optional(columns: &HashSet<String>, name: &'static str, fallback: &'static str) -> &'static str {
    if columns.contains(name) {
        name
    } else {
        fallback
    }
}

fn read_sqlite(path: &Path) -> Result<BackupData, String> {
    let mut conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("Cannot open source database: {e}"))?;
    conn.pragma_update(None, "trusted_schema", false)
        .map_err(|e| e.to_string())?;
    let transaction = conn.transaction().map_err(|e| e.to_string())?;
    let result =
        read_sqlite_snapshot(&transaction).map_err(|e| format!("Invalid source database: {e}"))?;
    transaction.commit().map_err(|e| e.to_string())?;
    Ok(result)
}

fn read_sqlite_snapshot(conn: &Connection) -> rusqlite::Result<BackupData> {
    let clip_columns = columns(conn, "clipboard_entries")?;
    let opt = |field, fallback| optional(&clip_columns, field, fallback);
    let mut statement = conn.prepare(&format!(
        "SELECT id, category, content_type, content, preview, hash, pinned, created_at, {}, {}, {}, {}
         FROM clipboard_entries ORDER BY id",
        opt("original_content", "NULL"), opt("updated_at", "NULL"), opt("archived_at", "NULL"), opt("category_tags", "'[]'"),
    ))?;
    let clipboard_entries = statement
        .query_map([], |row| {
            let raw: String = row.get(1)?;
            let category: Category = serde_json::from_value(serde_json::Value::String(raw))
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
            let created: String = row.get(7)?;
            Ok(ClipboardEntry {
                id: row.get(0)?,
                category,
                content_type: row.get(2)?,
                content: row.get(3)?,
                preview: row.get(4)?,
                hash: row.get(5)?,
                pinned: row.get(6)?,
                created_at: timestamp(&created).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        7,
                        rusqlite::types::Type::Text,
                        error.into(),
                    )
                })?,
                original_content: row.get(8)?,
                updated_at: row.get(9)?,
                archived_at: row.get(10)?,
                category_tags: decode_tags(row.get(11)?, 11)?,
                version: 1,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let memo_columns = columns(conn, "memos")?;
    let mut memos = Vec::new();
    if !memo_columns.is_empty() {
        let opt = |field, fallback| optional(&memo_columns, field, fallback);
        let mut stmt = conn.prepare(&format!(
            "SELECT id, title, body, tags, pinned, created_at, updated_at, {}, {}, {} FROM memos ORDER BY id",
            opt("sort_order", "id"), opt("archived_at", "NULL"), opt("auto_tags", "'[]'"),
        ))?;
        memos = stmt
            .query_map([], |row| {
                Ok(Memo {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    body: row.get(2)?,
                    tags: row.get(3)?,
                    pinned: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    sort_order: row.get(7)?,
                    archived_at: row.get(8)?,
                    auto_tags: decode_tags(row.get(9)?, 9)?,
                    version: 1,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
    }
    Ok(snapshot(clipboard_entries, memos, "SQLite"))
}

fn decode_tags<T: serde::de::DeserializeOwned>(
    value: Option<String>,
    index: usize,
) -> rusqlite::Result<Vec<T>> {
    match value.as_deref().filter(|value| !value.trim().is_empty()) {
        None => Ok(Vec::new()),
        Some(value) => serde_json::from_str(value).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                index,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        }),
    }
}

pub fn snapshot(
    clipboard_entries: Vec<ClipboardEntry>,
    memos: Vec<Memo>,
    version: &str,
) -> BackupData {
    BackupData {
        app: "SuperClipboard".into(),
        backup_version: 1,
        app_version: version.into(),
        created_at: Utc::now(),
        clipboard_entries,
        memos,
        settings: Vec::new(),
    }
}

pub fn export(storage: &Storage) -> Result<BackupData, String> {
    if crate::remote_storage::is_remote_mode(storage) {
        return crate::remote_storage::workspace_snapshot(storage, false)
            .map_err(|error| error.to_string());
    }
    let mut data = storage
        .export_backup_data(crate::APP_VERSION)
        .map_err(|error| error.to_string())?;
    data.settings.clear();
    Ok(data)
}
