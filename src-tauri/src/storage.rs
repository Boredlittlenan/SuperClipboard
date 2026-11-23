mod clipboard_archive;
mod clipboard_repository;
mod memo_repository;
use crate::classifier::{classify_text_tags, Category, CLASSIFICATION_RULES_VERSION};
use crate::memo_tags;
use crate::search_index::{clipboard_search_text, memo_search_text};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use thiserror::Error;

const CLIPBOARD_QUERY_LIMIT: i64 = 50;
const MEMO_QUERY_LIMIT: i64 = 100;
const MAX_QUERY_LIMIT: i64 = 500;
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Storage operation failed: {0}")]
    InvalidOperation(String),
}

/// Result of capturing clipboard content that may already exist in history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardInsertResult {
    Inserted,
    Promoted,
    ArchivedDuplicate(i64),
    Ignored,
}

impl ClipboardInsertResult {
    pub fn inserted(self) -> bool {
        matches!(self, Self::Inserted)
    }

    pub fn refreshes_list(self) -> bool {
        matches!(self, Self::Inserted | Self::Promoted)
    }

    pub fn archived_duplicate_id(self) -> Option<i64> {
        match self {
            Self::ArchivedDuplicate(id) => Some(id),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupData {
    pub app: String,
    pub backup_version: u32,
    pub app_version: String,
    pub created_at: DateTime<Utc>,
    pub clipboard_entries: Vec<ClipboardEntry>,
    pub memos: Vec<Memo>,
    pub settings: Vec<SettingEntry>,
}

/// Aggregate values used by the footer and category tabs. Keeping these in one
/// query prevents every refresh from scanning the local database repeatedly.
#[derive(Debug, Clone, Copy)]
pub struct StorageStats {
    pub total: i64,
    pub text: i64,
    pub link: i64,
    pub image: i64,
    pub code: i64,
    pub email: i64,
    pub file_path: i64,
    pub archive: i64,
    pub memo_count: i64,
    pub memo_archive: i64,
    pub clipboard_size: i64,
    pub memo_size: i64,
}

/// All inputs needed to create a merged image memo atomically.
pub struct MergeMemoRequest<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub tags: &'a str,
    pub auto_tags: &'a [String],
    pub source_ids: &'a [i64],
    pub delete_sources: bool,
    pub archive_sources: bool,
}

/// A single clipboard entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub id: i64,
    pub category: Category,
    #[serde(default)]
    pub category_tags: Vec<Category>,
    pub content_type: String, // "text", "image/png", etc.
    pub content: String,      // Text content or base64-encoded image data
    pub preview: String,      // Short preview text for UI display
    pub hash: String,         // SHA-256 hash for deduplication
    pub pinned: bool,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_content: Option<String>, // Content before first edit (null = never edited)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>, // Timestamp of last edit (null = never edited)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>, // Timestamp of archival (null = not archived)
    #[serde(default = "default_record_version")]
    pub version: i64,
}

/// Query filter for listing entries
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryFilter {
    pub category: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    #[serde(default)]
    pub include_auxiliary_tags: bool,
    #[serde(default)]
    pub summary: bool,
    #[serde(default)]
    pub favorites: bool,
}

/// A memo/sticky note entry (separate from clipboard)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memo {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub tags: String,
    #[serde(default)]
    pub auto_tags: Vec<String>,
    pub pinned: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
    #[serde(default = "default_record_version")]
    pub version: i64,
}

fn default_record_version() -> i64 {
    1
}

/// Query filter for listing memos
#[derive(Debug, Default, Deserialize)]
pub struct MemoFilter {
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    #[serde(default)]
    pub summary: bool,
    #[serde(default)]
    pub favorites: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    pub updated: bool,
    pub conflict: bool,
}

impl UpdateResult {
    pub fn updated(updated: bool) -> Self {
        Self {
            updated,
            conflict: false,
        }
    }

    pub fn conflict() -> Self {
        Self {
            updated: false,
            conflict: true,
        }
    }
}

fn category_from_str(value: &str) -> Category {
    match value {
        "link" => Category::Link,
        "image" => Category::Image,
        "code" => Category::Code,
        "email" => Category::Email,
        "file_path" => Category::FilePath,
        _ => Category::Text,
    }
}

fn normalize_category_tags(tags: Vec<Category>) -> Vec<Category> {
    let mut result = Vec::new();
    for category in tags {
        if !result.contains(&category) {
            result.push(category);
        }
    }
    if result.is_empty() {
        result.push(Category::Text);
    }
    result
}

fn category_tags_json(tags: &[Category]) -> Result<String, serde_json::Error> {
    serde_json::to_string(&normalize_category_tags(tags.to_vec()))
}

fn category_tags_from_json(fallback: Category, value: Option<String>) -> Vec<Category> {
    let parsed = value
        .and_then(|json| serde_json::from_str::<Vec<Category>>(&json).ok())
        .unwrap_or_default();
    if parsed.is_empty() {
        vec![fallback]
    } else {
        normalize_category_tags(parsed)
    }
}

fn category_match_condition(column: &str, include_auxiliary_tags: bool) -> String {
    if include_auxiliary_tags {
        format!("({column} = ? OR category_tags LIKE ?)")
    } else {
        format!("{column} = ?")
    }
}

fn category_tag_pattern(category: &str) -> String {
    format!("%\"{}\"%", category)
}

/// Map a SQLite row to a ClipboardEntry (shared by query and get_entry_by_id)
fn map_row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClipboardEntry> {
    let category_str: String = row.get(1)?;
    let fallback_category = category_from_str(&category_str);

    let pinned_int: i32 = row.get(6)?;
    let created_str: String = row.get(7)?;
    let original_content: Option<String> = row.get(8)?;
    let updated_at: Option<String> = row.get(9)?;
    let archived_at: Option<String> = row.get(10)?;
    let category_tags_json: Option<String> = row.get(11).ok();
    let category_tags = category_tags_from_json(fallback_category, category_tags_json);
    let category = category_tags.first().cloned().unwrap_or(Category::Text);

    Ok(ClipboardEntry {
        id: row.get(0)?,
        category,
        category_tags,
        content_type: row.get(2)?,
        content: row.get(3)?,
        preview: row.get(4)?,
        hash: row.get(5)?,
        pinned: pinned_int != 0,
        created_at: DateTime::parse_from_rfc3339(&created_str)
            .unwrap_or_else(|_| Utc::now().into())
            .with_timezone(&Utc),
        original_content,
        updated_at,
        archived_at,
        version: row.get(12)?,
    })
}

fn map_row_to_memo(row: &rusqlite::Row<'_>) -> rusqlite::Result<Memo> {
    let pinned_int: i32 = row.get(5)?;
    let auto_tags_json: String = row.get(4)?;
    Ok(Memo {
        id: row.get(0)?,
        title: row.get(1)?,
        body: row.get(2)?,
        tags: row.get(3)?,
        auto_tags: serde_json::from_str(&auto_tags_json).unwrap_or_default(),
        pinned: pinned_int != 0,
        sort_order: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        archived_at: row.get(9)?,
        version: row.get(10)?,
    })
}

fn normalize_limit(limit: Option<i64>, default: i64) -> i64 {
    limit.unwrap_or(default).clamp(1, MAX_QUERY_LIMIT)
}

fn normalize_offset(offset: Option<i64>) -> i64 {
    offset.unwrap_or(0).max(0)
}

fn search_tokens(search: &str) -> Vec<String> {
    search
        .split_whitespace()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .take(8)
        .map(ToOwned::to_owned)
        .collect()
}

fn append_token_search(
    sql: &mut String,
    param_values: &mut Vec<Box<dyn rusqlite::types::ToSql>>,
    search: &str,
    columns: &[&str],
) {
    for token in search_tokens(search) {
        let conditions = columns
            .iter()
            .map(|column| format!("{column} LIKE ?"))
            .collect::<Vec<_>>()
            .join(" OR ");
        sql.push_str(" AND (");
        sql.push_str(&conditions);
        sql.push(')');
        let pattern = format!("%{}%", token);
        for _ in columns {
            param_values.push(Box::new(pattern.clone()));
        }
    }
}

pub struct Storage {
    pub(crate) conn: Mutex<Connection>,
    pub(crate) workspace: std::sync::RwLock<u64>,
}

impl Storage {
    /// Open or create the database at the given path
    pub fn new(db_path: &Path) -> Result<Self, StorageError> {
        let conn = Connection::open(db_path)?;
        let storage = Self {
            conn: Mutex::new(conn),
            workspace: std::sync::RwLock::new(0),
        };
        storage.init_tables()?;
        Ok(storage)
    }

    fn init_tables(&self) -> SqlResult<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        crate::storage_migrations::run(&tx)?;
        tx.commit()
    }
    /// Compute SHA-256 hash of content for deduplication
    pub fn hash_content(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Compute SHA-256 hash of raw bytes for binary clipboard data.
    pub fn hash_bytes(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex::encode(hasher.finalize())
    }

    pub fn export_backup_data(&self, app_version: &str) -> Result<BackupData, StorageError> {
        let conn = self.conn.lock().unwrap();

        let mut entry_stmt = conn.prepare(
            "SELECT id, category, content_type, content, preview, hash, pinned, created_at, original_content, updated_at, archived_at, category_tags, content_version
             FROM clipboard_entries ORDER BY id ASC",
        )?;
        let clipboard_entries = entry_stmt
            .query_map([], map_row_to_entry)?
            .collect::<SqlResult<Vec<_>>>()?;

        let mut memo_stmt = conn.prepare(
            "SELECT id, title, body, tags, auto_tags, pinned, sort_order, created_at, updated_at, archived_at, content_version
             FROM memos ORDER BY id ASC",
        )?;
        let memos = memo_stmt
            .query_map([], map_row_to_memo)?
            .collect::<SqlResult<Vec<_>>>()?;

        let mut settings_stmt = conn.prepare("SELECT key, value FROM settings ORDER BY key ASC")?;
        let settings = settings_stmt
            .query_map([], |row| {
                Ok(SettingEntry {
                    key: row.get(0)?,
                    value: row.get(1)?,
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(BackupData {
            app: "SuperClipboard".to_string(),
            backup_version: 1,
            app_version: app_version.to_string(),
            created_at: Utc::now(),
            clipboard_entries,
            memos,
            settings,
        })
    }

    /// Get a setting value by key; returns None if not set
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let value = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .ok();
        Ok(value)
    }

    /// Read a group of settings while holding the database lock once.
    pub fn get_settings(&self, keys: &[String]) -> Result<HashMap<String, String>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut statement = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut values = HashMap::with_capacity(keys.len());
        for key in keys {
            if let Ok(value) = statement.query_row(params![key], |row| row.get::<_, String>(0)) {
                values.insert(key.clone(), value);
            }
        }
        Ok(values)
    }

    /// Insert or update a setting value
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Persist a group of settings atomically.
    pub fn set_settings(&self, values: &HashMap<String, String>) -> Result<(), StorageError> {
        let mut conn = self.conn.lock().unwrap();
        let transaction = conn.transaction()?;
        {
            let mut statement = transaction.prepare(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )?;
            for (key, value) in values {
                statement.execute(params![key, value])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
