use crate::classifier::{classify_text_tags, Category, CLASSIFICATION_RULES_VERSION};
use crate::memo_tags;
use crate::search_index::{clipboard_search_text, memo_search_text};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result as SqlResult};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreSummary {
    pub clipboard_entries: usize,
    pub memos: usize,
    pub settings: usize,
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
        version: 1,
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
        version: 1,
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
    conn: Mutex<Connection>,
}

impl Storage {
    /// Open or create the database at the given path
    pub fn new(db_path: &Path) -> Result<Self, StorageError> {
        let conn = Connection::open(db_path)?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.init_tables()?;
        Ok(storage)
    }

    fn init_tables(&self) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        crate::storage_migrations::run(&conn)
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

    /// Recompute classification metadata for existing non-image clipboard entries.
    /// This is intentionally user-triggered and never runs during database startup.
    pub fn reclassify_clipboard_entries(&self) -> Result<u64, StorageError> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        let rows = {
            let mut statement = tx.prepare(
                "SELECT id, category, category_tags, content_type, content, preview, search_text
                 FROM clipboard_entries
                 WHERE content_type NOT LIKE 'image/%'",
            )?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                })?
                .collect::<SqlResult<Vec<_>>>()?;
            rows
        };

        let mut changed = 0_u64;
        for (
            id,
            stored_category,
            stored_category_tags,
            content_type,
            content,
            preview,
            stored_search_text,
        ) in rows
        {
            let categories = classify_text_tags(&content);
            let category = categories.first().cloned().unwrap_or(Category::Text);
            let category = category.to_string();
            let category_tags = category_tags_json(&categories)?;
            let search_text = clipboard_search_text(&content_type, &content, &preview, &categories);

            if stored_category == category
                && stored_category_tags == category_tags
                && stored_search_text == search_text
            {
                continue;
            }

            tx.execute(
                "UPDATE clipboard_entries
                 SET category = ?1, category_tags = ?2, search_text = ?3
                 WHERE id = ?4",
                params![category, category_tags, search_text, id],
            )?;
            changed += 1;
        }

        tx.execute(
            "INSERT INTO settings (key, value) VALUES ('classification_rules_applied_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![CLASSIFICATION_RULES_VERSION.to_string()],
        )?;

        tx.commit()?;
        Ok(changed)
    }

    /// Insert a new clipboard entry, returns Ok(true) if inserted, Ok(false) if duplicate
    pub fn insert(&self, entry: &ClipboardEntry) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        let category_tags = normalize_category_tags(entry.category_tags.clone());
        let category = category_tags.first().cloned().unwrap_or(Category::Text);
        let search_text = clipboard_search_text(
            &entry.content_type,
            &entry.content,
            &entry.preview,
            &category_tags,
        );
        let category_tags = category_tags_json(&category_tags)?;
        let result = conn.execute(
            "INSERT OR IGNORE INTO clipboard_entries 
             (category, category_tags, content_type, content, preview, search_text, hash, content_hash, pinned, created_at)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10
             WHERE NOT EXISTS (
                SELECT 1 FROM clipboard_entries WHERE hash = ?7 OR content_hash = ?8
             )",
            params![
                category.to_string(),
                category_tags,
                entry.content_type,
                entry.content,
                entry.preview,
                search_text,
                entry.hash,
                Self::hash_content(&entry.content),
                entry.pinned as i32,
                entry.created_at.to_rfc3339(),
            ],
        )?;
        Ok(result > 0)
    }

    /// Query entries with optional filters
    pub fn query(&self, filter: &QueryFilter) -> Result<Vec<ClipboardEntry>, StorageError> {
        let conn = self.conn.lock().unwrap();

        let mut sql = String::from("SELECT id, category, content_type, content, preview, hash, pinned, created_at, original_content, updated_at, archived_at, category_tags FROM clipboard_entries WHERE archived_at IS NULL");
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref cat) = filter.category {
            sql.push_str(" AND ");
            sql.push_str(&category_match_condition(
                "category",
                filter.include_auxiliary_tags,
            ));
            param_values.push(Box::new(cat.clone()));
            if filter.include_auxiliary_tags {
                param_values.push(Box::new(category_tag_pattern(cat)));
            }
        }

        if let Some(ref search) = filter.search {
            append_token_search(&mut sql, &mut param_values, search, &["search_text"]);
        }

        sql.push_str(" ORDER BY pinned DESC, created_at DESC");

        let limit = normalize_limit(filter.limit, CLIPBOARD_QUERY_LIMIT);
        sql.push_str(" LIMIT ?");
        param_values.push(Box::new(limit));

        let offset = normalize_offset(filter.offset);
        if offset > 0 {
            sql.push_str(" OFFSET ?");
            param_values.push(Box::new(offset));
        }

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let entries = stmt
            .query_map(params_refs.as_slice(), map_row_to_entry)?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(entries)
    }

    /// Get a single entry by ID
    pub fn get_entry_by_id(&self, id: i64) -> Result<Option<ClipboardEntry>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, category, content_type, content, preview, hash, pinned, created_at, original_content, updated_at, archived_at, category_tags
             FROM clipboard_entries WHERE id = ?1",
        )?;

        let entry = stmt.query_row(params![id], map_row_to_entry).ok();

        Ok(entry)
    }

    /// Fetch selected clipboard entries while preserving the caller's display order.
    pub fn get_entries_by_ids(&self, ids: &[i64]) -> Result<Vec<ClipboardEntry>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, category, content_type, content, preview, hash, pinned, created_at, original_content, updated_at, archived_at, category_tags
             FROM clipboard_entries WHERE id = ?1",
        )?;
        let mut entries = Vec::with_capacity(ids.len());

        for id in ids {
            match stmt.query_row(params![id], map_row_to_entry) {
                Ok(entry) => entries.push(entry),
                Err(rusqlite::Error::QueryReturnedNoRows) => {}
                Err(error) => return Err(StorageError::Database(error)),
            }
        }

        Ok(entries)
    }

    pub fn export_backup_data(&self, app_version: &str) -> Result<BackupData, StorageError> {
        let conn = self.conn.lock().unwrap();

        let mut entry_stmt = conn.prepare(
            "SELECT id, category, content_type, content, preview, hash, pinned, created_at, original_content, updated_at, archived_at, category_tags
             FROM clipboard_entries ORDER BY id ASC",
        )?;
        let clipboard_entries = entry_stmt
            .query_map([], map_row_to_entry)?
            .collect::<SqlResult<Vec<_>>>()?;

        let mut memo_stmt = conn.prepare(
            "SELECT id, title, body, tags, auto_tags, pinned, sort_order, created_at, updated_at, archived_at
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

    pub fn restore_backup_data(&self, backup: &BackupData) -> Result<RestoreSummary, StorageError> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;

        tx.execute("DELETE FROM clipboard_entries", [])?;
        tx.execute("DELETE FROM memos", [])?;
        tx.execute("DELETE FROM settings", [])?;

        for entry in &backup.clipboard_entries {
            let category_tags = normalize_category_tags(entry.category_tags.clone());
            let search_text = clipboard_search_text(
                &entry.content_type,
                &entry.content,
                &entry.preview,
                &category_tags,
            );
            tx.execute(
                "INSERT INTO clipboard_entries
                 (id, category, category_tags, content_type, content, preview, search_text, hash, content_hash, pinned, created_at, original_content, updated_at, archived_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    entry.id,
                    category_tags
                        .first()
                        .cloned()
                        .unwrap_or(entry.category.clone())
                        .to_string(),
                    category_tags_json(&category_tags)?,
                    entry.content_type,
                    entry.content,
                    entry.preview,
                    search_text,
                    entry.hash,
                    Self::hash_content(&entry.content),
                    entry.pinned as i32,
                    entry.created_at.to_rfc3339(),
                    entry.original_content,
                    entry.updated_at,
                    entry.archived_at,
                ],
            )?;
        }

        for memo in &backup.memos {
            let auto_tags = if memo.auto_tags.is_empty() {
                memo_tags::infer(&memo.title, &memo.body)
            } else {
                memo.auto_tags.clone()
            };
            let tags = memo_tags::manual_only(&memo.tags);
            let search_text = memo_search_text(&memo.title, &memo.body, &tags, &auto_tags);
            tx.execute(
                "INSERT INTO memos
                 (id, title, body, tags, auto_tags, search_text, pinned, sort_order, created_at, updated_at, archived_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    memo.id,
                    memo.title,
                    memo.body,
                    tags,
                    serde_json::to_string(&auto_tags)?,
                    search_text,
                    memo.pinned as i32,
                    memo.sort_order,
                    memo.created_at,
                    memo.updated_at,
                    memo.archived_at,
                ],
            )?;
        }

        for setting in &backup.settings {
            tx.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)",
                params![setting.key, setting.value],
            )?;
        }

        tx.execute(
            "INSERT INTO settings (key, value) VALUES ('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![crate::storage_migrations::SCHEMA_VERSION],
        )?;

        tx.execute(
            "UPDATE sqlite_sequence SET seq = COALESCE((SELECT MAX(id) FROM clipboard_entries), 0) WHERE name = 'clipboard_entries'",
            [],
        )?;
        tx.execute(
            "UPDATE sqlite_sequence SET seq = COALESCE((SELECT MAX(id) FROM memos), 0) WHERE name = 'memos'",
            [],
        )?;

        tx.commit()?;

        Ok(RestoreSummary {
            clipboard_entries: backup.clipboard_entries.len(),
            memos: backup.memos.len(),
            settings: backup.settings.len(),
        })
    }

    /// Delete an entry by ID
    pub fn delete(&self, id: i64) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM clipboard_entries WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    pub fn delete_entries(&self, ids: &[i64], archive: bool) -> Result<u64, StorageError> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        let mut affected = 0_u64;
        for id in ids {
            let rows = if archive {
                tx.execute(
                    "UPDATE clipboard_entries
                     SET archived_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     WHERE id = ?1 AND archived_at IS NULL",
                    params![id],
                )?
            } else {
                tx.execute("DELETE FROM clipboard_entries WHERE id = ?1", params![id])?
            };
            affected += rows as u64;
        }
        tx.commit()?;
        Ok(affected)
    }

    /// Toggle pinned status
    pub fn toggle_pin(&self, id: i64) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE clipboard_entries SET pinned = NOT pinned WHERE id = ?1",
            params![id],
        )?;
        // Return the new pinned state
        let pinned: bool = conn
            .query_row(
                "SELECT pinned FROM clipboard_entries WHERE id = ?1",
                params![id],
                |row| row.get::<_, i32>(0).map(|v| v != 0),
            )
            .unwrap_or(false);
        Ok(pinned)
    }

    /// Update a clipboard entry while preserving its first captured content.
    pub fn update_entry(&self, id: i64, new_content: &str) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        let current: Option<(String, Option<String>)> = conn
            .query_row(
                "SELECT content, original_content FROM clipboard_entries WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();
        let (current_content, existing_original) = match current {
            Some(current) => current,
            None => return Ok(false),
        };
        if current_content == new_content {
            return Ok(true);
        }
        let reverts_to_original = existing_original.as_deref() == Some(new_content);
        let original = if reverts_to_original {
            None
        } else {
            Some(existing_original.unwrap_or(current_content))
        };
        let preview = if new_content.len() > 200 {
            new_content.chars().take(200).collect::<String>()
        } else {
            new_content.to_string()
        };
        let categories = classify_text_tags(new_content);
        let category = categories.first().cloned().unwrap_or(Category::Text);
        let search_text = clipboard_search_text("text/plain", new_content, &preview, &categories);
        let category_tags = category_tags_json(&categories)?;
        let now = (!reverts_to_original).then(|| Utc::now().to_rfc3339());

        let rows = conn.execute(
            "UPDATE clipboard_entries SET category = ?1, category_tags = ?2, content = ?3, preview = ?4, search_text = ?5, content_hash = ?6, original_content = ?7, updated_at = ?8 WHERE id = ?9",
            params![category.to_string(), category_tags, new_content, preview, search_text, Self::hash_content(new_content), original, now, id],
        )?;
        Ok(rows > 0)
    }

    /// Get total count of entries, optionally filtered by category
    pub fn count(
        &self,
        category: Option<&str>,
        include_auxiliary_tags: bool,
    ) -> Result<i64, StorageError> {
        let conn = self.conn.lock().unwrap();
        let count = if let Some(cat) = category {
            conn.query_row(
                "SELECT COUNT(*) FROM clipboard_entries
                 WHERE (category = ?1 OR (?2 AND category_tags LIKE ?3)) AND archived_at IS NULL",
                params![cat, include_auxiliary_tags, category_tag_pattern(cat)],
                |row| row.get(0),
            )?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM clipboard_entries WHERE archived_at IS NULL",
                [],
                |row| row.get(0),
            )?
        };
        Ok(count)
    }

    /// Get database size in bytes (page_count * page_size)
    pub fn db_size(&self) -> Result<i64, StorageError> {
        let conn = self.conn.lock().unwrap();
        let page_count: i64 = conn.query_row("PRAGMA page_count", [], |row| row.get(0))?;
        let page_size: i64 = conn.query_row("PRAGMA page_size", [], |row| row.get(0))?;
        Ok(page_count * page_size)
    }

    /// Get clipboard entries storage size in bytes (sum of content field lengths, excluding archived)
    pub fn clipboard_storage_size(&self) -> Result<i64, StorageError> {
        let conn = self.conn.lock().unwrap();
        let size: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(LENGTH(content) + LENGTH(preview) + LENGTH(COALESCE(original_content, ''))), 0) FROM clipboard_entries WHERE archived_at IS NULL",
                [],
                |row| row.get(0),
            )?;
        Ok(size)
    }

    /// Get memos storage size in bytes (sum of title + body + tags field lengths, excluding archived)
    pub fn memo_storage_size(&self) -> Result<i64, StorageError> {
        let conn = self.conn.lock().unwrap();
        let size: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(LENGTH(title) + LENGTH(body) + LENGTH(tags)), 0) FROM memos WHERE archived_at IS NULL",
                [],
                |row| row.get(0),
            )?;
        Ok(size)
    }

    /// Clear non-pinned entries, optionally limited to a category.
    pub fn clear_unpinned(
        &self,
        archive: bool,
        category: Option<&str>,
        include_auxiliary_tags: bool,
    ) -> Result<u64, StorageError> {
        let conn = self.conn.lock().unwrap();
        if let Some(category) = category {
            let tag_pattern = category_tag_pattern(category);
            let rows = if archive {
                conn.execute(
                    "UPDATE clipboard_entries
                     SET archived_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     WHERE pinned = 0 AND archived_at IS NULL
                       AND (category = ?1 OR (?2 AND category_tags LIKE ?3))",
                    params![category, include_auxiliary_tags, tag_pattern],
                )?
            } else {
                conn.execute(
                    "DELETE FROM clipboard_entries
                     WHERE pinned = 0 AND archived_at IS NULL
                       AND (category = ?1 OR (?2 AND category_tags LIKE ?3))",
                    params![category, include_auxiliary_tags, tag_pattern],
                )?
            };
            return Ok(rows as u64);
        }

        if archive {
            let rows = conn.execute(
                "UPDATE clipboard_entries SET archived_at = datetime('now') WHERE pinned = 0 AND archived_at IS NULL",
                [],
            )?;
            Ok(rows as u64)
        } else {
            let rows = conn.execute(
                "DELETE FROM clipboard_entries WHERE pinned = 0 AND archived_at IS NULL",
                [],
            )?;
            Ok(rows as u64)
        }
    }

    /// Archive a single entry by ID
    pub fn archive_entry(&self, id: i64) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE clipboard_entries SET archived_at = datetime('now') WHERE id = ?1 AND archived_at IS NULL",
            params![id],
        )?;
        Ok(rows > 0)
    }

    /// Unarchive (restore) a single entry by ID
    pub fn unarchive_entry(&self, id: i64) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE clipboard_entries SET archived_at = NULL WHERE id = ?1",
            params![id],
        )?;
        Ok(rows > 0)
    }

    /// Query archived entries
    pub fn query_archived(
        &self,
        filter: &QueryFilter,
    ) -> Result<Vec<ClipboardEntry>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut sql = String::from("SELECT id, category, content_type, content, preview, hash, pinned, created_at, original_content, updated_at, archived_at, category_tags FROM clipboard_entries WHERE archived_at IS NOT NULL");
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref cat) = filter.category {
            sql.push_str(" AND ");
            sql.push_str(&category_match_condition(
                "category",
                filter.include_auxiliary_tags,
            ));
            param_values.push(Box::new(cat.clone()));
            if filter.include_auxiliary_tags {
                param_values.push(Box::new(category_tag_pattern(cat)));
            }
        }

        if let Some(ref search) = filter.search {
            append_token_search(&mut sql, &mut param_values, search, &["search_text"]);
        }

        sql.push_str(" ORDER BY archived_at DESC");

        let limit = normalize_limit(filter.limit, CLIPBOARD_QUERY_LIMIT);
        sql.push_str(" LIMIT ?");
        param_values.push(Box::new(limit));

        let offset = normalize_offset(filter.offset);
        if offset > 0 {
            sql.push_str(" OFFSET ?");
            param_values.push(Box::new(offset));
        }

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let entries = stmt
            .query_map(params_refs.as_slice(), map_row_to_entry)?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(entries)
    }

    /// Count archived entries
    pub fn archive_count(&self) -> Result<i64, StorageError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM clipboard_entries WHERE archived_at IS NOT NULL",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Permanently delete entries archived more than `days` days ago
    pub fn purge_old_archives(&self, days: i64) -> Result<u64, StorageError> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "DELETE FROM clipboard_entries WHERE archived_at IS NOT NULL AND archived_at < datetime('now', '-' || ?1 || ' days')",
            params![days],
        )?;
        Ok(rows as u64)
    }

    /// Permanently delete all archived clipboard entries.
    pub fn empty_archive(&self) -> Result<u64, StorageError> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "DELETE FROM clipboard_entries WHERE archived_at IS NOT NULL",
            [],
        )?;
        Ok(rows as u64)
    }

    /// Permanently delete a single archived entry
    pub fn permanent_delete(&self, id: i64) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM clipboard_entries WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    // ─── Memo CRUD ──────────────────────────────────────────────────

    /// Query memos with optional search filter
    pub fn get_memos(&self, filter: &MemoFilter) -> Result<Vec<Memo>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut sql = String::from("SELECT id, title, body, tags, auto_tags, pinned, sort_order, created_at, updated_at, archived_at FROM memos WHERE archived_at IS NULL");
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref search) = filter.search {
            append_token_search(&mut sql, &mut param_values, search, &["search_text"]);
        }

        sql.push_str(" ORDER BY pinned DESC, sort_order DESC");

        let limit = normalize_limit(filter.limit, MEMO_QUERY_LIMIT);
        sql.push_str(" LIMIT ?");
        param_values.push(Box::new(limit));

        let offset = normalize_offset(filter.offset);
        if offset > 0 {
            sql.push_str(" OFFSET ?");
            param_values.push(Box::new(offset));
        }

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let memos = stmt
            .query_map(params_refs.as_slice(), map_row_to_memo)?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(memos)
    }

    /// Create a new memo, returns the created memo
    pub fn create_memo(
        &self,
        title: &str,
        body: &str,
        tags: &str,
        auto_tags: &[String],
    ) -> Result<Memo, StorageError> {
        let conn = self.conn.lock().unwrap();
        let tags = memo_tags::manual_only(tags);
        let search_text = memo_search_text(title, body, &tags, auto_tags);
        let auto_tags = serde_json::to_string(auto_tags)?;
        conn.execute(
            "INSERT INTO memos (title, body, tags, auto_tags, search_text, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM memos), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![title, body, tags, auto_tags, search_text],
        )?;
        let id = conn.last_insert_rowid();
        let memo = conn.query_row(
            "SELECT id, title, body, tags, auto_tags, pinned, sort_order, created_at, updated_at, archived_at FROM memos WHERE id = ?1",
            params![id],
            map_row_to_memo,
        )?;
        Ok(memo)
    }

    /// Update an existing memo (also refreshes updated_at)
    pub fn update_memo(
        &self,
        id: i64,
        title: &str,
        body: &str,
        tags: &str,
        auto_tags: &[String],
    ) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        let tags = memo_tags::manual_only(tags);
        let search_text = memo_search_text(title, body, &tags, auto_tags);
        let auto_tags = serde_json::to_string(auto_tags)?;
        let rows = conn.execute(
            "UPDATE memos SET title = ?1, body = ?2, tags = ?3, auto_tags = ?4, search_text = ?5, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?6",
            params![title, body, tags, auto_tags, search_text, id],
        )?;
        Ok(rows > 0)
    }

    /// Delete a memo by ID
    pub fn delete_memo(&self, id: i64, archive: bool) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        if archive {
            let rows = conn.execute(
                "UPDATE memos SET archived_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1 AND archived_at IS NULL",
                params![id],
            )?;
            Ok(rows > 0)
        } else {
            let rows = conn.execute("DELETE FROM memos WHERE id = ?1", params![id])?;
            Ok(rows > 0)
        }
    }

    /// Toggle memo pinned status
    pub fn toggle_memo_pin(&self, id: i64) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE memos SET pinned = NOT pinned WHERE id = ?1",
            params![id],
        )?;
        let pinned: bool = conn
            .query_row(
                "SELECT pinned FROM memos WHERE id = ?1",
                params![id],
                |row| row.get::<_, i32>(0).map(|v| v != 0),
            )
            .unwrap_or(false);
        Ok(pinned)
    }

    /// Get total memo count
    pub fn memo_count(&self) -> Result<i64, StorageError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM memos WHERE archived_at IS NULL",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Batch-update sort_order for multiple memos in a single transaction
    pub fn reorder_memos(&self, orders: &[(i64, i64)]) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        for (id, sort_order) in orders {
            tx.execute(
                "UPDATE memos SET sort_order = ?1 WHERE id = ?2",
                params![sort_order, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Archive a memo by ID
    pub fn archive_memo(&self, id: i64) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE memos SET archived_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1 AND archived_at IS NULL",
            params![id],
        )?;
        Ok(rows > 0)
    }

    /// Unarchive (restore) a memo by ID
    pub fn unarchive_memo(&self, id: i64) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE memos SET archived_at = NULL WHERE id = ?1",
            params![id],
        )?;
        Ok(rows > 0)
    }

    /// Query archived memos
    pub fn query_archived_memos(&self, filter: &MemoFilter) -> Result<Vec<Memo>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut sql = String::from("SELECT id, title, body, tags, auto_tags, pinned, sort_order, created_at, updated_at, archived_at FROM memos WHERE archived_at IS NOT NULL");
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref search) = filter.search {
            append_token_search(&mut sql, &mut param_values, search, &["search_text"]);
        }

        sql.push_str(" ORDER BY archived_at DESC");

        let limit = normalize_limit(filter.limit, MEMO_QUERY_LIMIT);
        sql.push_str(" LIMIT ?");
        param_values.push(Box::new(limit));

        let offset = normalize_offset(filter.offset);
        if offset > 0 {
            sql.push_str(" OFFSET ?");
            param_values.push(Box::new(offset));
        }

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let memos = stmt
            .query_map(params_refs.as_slice(), map_row_to_memo)?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(memos)
    }

    /// Count archived memos
    pub fn memo_archive_count(&self) -> Result<i64, StorageError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM memos WHERE archived_at IS NOT NULL",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Permanently delete memos archived more than `days` days ago
    pub fn purge_old_memo_archives(&self, days: i64) -> Result<u64, StorageError> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "DELETE FROM memos WHERE archived_at IS NOT NULL AND archived_at < datetime('now', '-' || ?1 || ' days')",
            params![days],
        )?;
        Ok(rows as u64)
    }

    /// Permanently delete all archived memos.
    pub fn empty_memo_archive(&self) -> Result<u64, StorageError> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM memos WHERE archived_at IS NOT NULL", [])?;
        Ok(rows as u64)
    }

    /// Permanently delete a single memo (archived or not)
    pub fn permanent_delete_memo(&self, id: i64) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM memos WHERE id = ?1", params![id])?;
        Ok(rows > 0)
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
mod tests {
    use super::*;
    use uuid::Uuid;

    fn temp_db_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("superclipboard-storage-{}.db", Uuid::new_v4()))
    }

    #[test]
    fn settings_batch_round_trips_atomically() {
        let db_path = temp_db_path();
        let storage = Storage::new(&db_path).unwrap();
        let values = HashMap::from([
            ("theme_mode".to_string(), "dark".to_string()),
            ("memo_enabled".to_string(), "true".to_string()),
        ]);

        storage.set_settings(&values).unwrap();
        storage.set_setting("language", "zh-CN").unwrap();
        storage
            .set_settings(&HashMap::from([
                ("storage_mode".to_string(), "remote".to_string()),
                ("remote_db_ready".to_string(), "true".to_string()),
            ]))
            .unwrap();
        let loaded = storage
            .get_settings(&[
                "theme_mode".to_string(),
                "memo_enabled".to_string(),
                "missing".to_string(),
            ])
            .unwrap();

        assert_eq!(loaded.get("theme_mode").map(String::as_str), Some("dark"));
        assert_eq!(loaded.get("memo_enabled").map(String::as_str), Some("true"));
        assert!(!loaded.contains_key("missing"));
        assert_eq!(
            storage.get_setting("language").unwrap().as_deref(),
            Some("zh-CN"),
        );

        drop(storage);
        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn auxiliary_categories_only_participate_when_enabled() {
        let db_path = temp_db_path();
        let storage = Storage::new(&db_path).unwrap();
        let content = "https://example.com contact@example.com";
        let entry = ClipboardEntry {
            id: 0,
            category: Category::Link,
            category_tags: vec![Category::Link, Category::Email],
            content_type: "text/plain".to_string(),
            content: content.to_string(),
            preview: content.to_string(),
            hash: Storage::hash_content(content),
            pinned: false,
            created_at: Utc::now(),
            original_content: None,
            updated_at: None,
            archived_at: None,
            version: 1,
        };
        assert!(storage.insert(&entry).unwrap());

        let primary_only = QueryFilter {
            category: Some("email".to_string()),
            ..Default::default()
        };
        let include_auxiliary = QueryFilter {
            category: Some("email".to_string()),
            include_auxiliary_tags: true,
            ..Default::default()
        };

        assert!(storage.query(&primary_only).unwrap().is_empty());
        assert_eq!(storage.query(&include_auxiliary).unwrap().len(), 1);
        assert_eq!(storage.count(Some("email"), false).unwrap(), 0);
        assert_eq!(storage.count(Some("email"), true).unwrap(), 1);
        assert_eq!(storage.count(Some("link"), false).unwrap(), 1);

        drop(storage);
        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn clear_unpinned_category_respects_auxiliary_tags_and_pins() {
        let db_path = temp_db_path();
        let storage = Storage::new(&db_path).unwrap();
        let insert = |content: &str, category: Category, category_tags: Vec<Category>, pinned| {
            storage
                .insert(&ClipboardEntry {
                    id: 0,
                    category,
                    category_tags,
                    content_type: "text/plain".to_string(),
                    content: content.to_string(),
                    preview: content.to_string(),
                    hash: Storage::hash_content(content),
                    pinned,
                    created_at: Utc::now(),
                    original_content: None,
                    updated_at: None,
                    archived_at: None,
                    version: 1,
                })
                .unwrap()
        };

        assert!(insert(
            "https://example.com contact@example.com",
            Category::Link,
            vec![Category::Link, Category::Email],
            false,
        ));
        assert!(insert(
            "primary@example.com",
            Category::Email,
            vec![Category::Email],
            false,
        ));
        assert!(insert(
            "pinned@example.com",
            Category::Email,
            vec![Category::Email],
            true,
        ));
        assert!(insert(
            "plain text",
            Category::Text,
            vec![Category::Text],
            false,
        ));

        assert_eq!(
            storage.clear_unpinned(false, Some("email"), false).unwrap(),
            1
        );
        assert_eq!(storage.count(Some("email"), false).unwrap(), 1);
        assert_eq!(storage.count(Some("link"), false).unwrap(), 1);

        assert_eq!(
            storage.clear_unpinned(false, Some("email"), true).unwrap(),
            1
        );
        assert_eq!(storage.count(Some("email"), true).unwrap(), 1);
        assert_eq!(storage.count(Some("link"), false).unwrap(), 0);
        assert_eq!(storage.count(Some("text"), false).unwrap(), 1);

        drop(storage);
        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn clear_unpinned_category_archives_only_matching_entries() {
        let db_path = temp_db_path();
        let storage = Storage::new(&db_path).unwrap();
        for (content, category, category_tags) in [
            (
                "https://example.com contact@example.com",
                Category::Link,
                vec![Category::Link, Category::Email],
            ),
            ("plain text", Category::Text, vec![Category::Text]),
        ] {
            assert!(storage
                .insert(&ClipboardEntry {
                    id: 0,
                    category,
                    category_tags,
                    content_type: "text/plain".to_string(),
                    content: content.to_string(),
                    preview: content.to_string(),
                    hash: Storage::hash_content(content),
                    pinned: false,
                    created_at: Utc::now(),
                    original_content: None,
                    updated_at: None,
                    archived_at: None,
                    version: 1,
                })
                .unwrap());
        }

        assert_eq!(
            storage.clear_unpinned(true, Some("email"), true).unwrap(),
            1
        );
        assert_eq!(storage.count(Some("link"), false).unwrap(), 0);
        assert_eq!(storage.count(Some("text"), false).unwrap(), 1);
        assert_eq!(storage.archive_count().unwrap(), 1);

        drop(storage);
        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn clear_all_unpinned_keeps_recycle_bin_entries() {
        let db_path = temp_db_path();
        let storage = Storage::new(&db_path).unwrap();
        for (content, pinned) in [
            ("active clipboard", false),
            ("archived clipboard", false),
            ("pinned clipboard", true),
        ] {
            assert!(storage
                .insert(&ClipboardEntry {
                    id: 0,
                    category: Category::Text,
                    category_tags: vec![Category::Text],
                    content_type: "text/plain".to_string(),
                    content: content.to_string(),
                    preview: content.to_string(),
                    hash: Storage::hash_content(content),
                    pinned,
                    created_at: Utc::now(),
                    original_content: None,
                    updated_at: None,
                    archived_at: None,
                    version: 1,
                })
                .unwrap());
        }

        let archived_id = storage
            .query(&QueryFilter::default())
            .unwrap()
            .into_iter()
            .find(|entry| entry.content == "archived clipboard")
            .unwrap()
            .id;
        assert!(storage.archive_entry(archived_id).unwrap());

        assert_eq!(storage.clear_unpinned(false, None, false).unwrap(), 1);
        assert_eq!(storage.count(None, false).unwrap(), 1);
        assert_eq!(storage.archive_count().unwrap(), 1);

        drop(storage);
        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn batch_delete_archives_and_removes_mixed_categories() {
        let db_path = temp_db_path();
        let storage = Storage::new(&db_path).unwrap();
        for (content, category) in [
            ("https://example.com", Category::Link),
            ("plain batch item", Category::Text),
        ] {
            let entry = ClipboardEntry {
                id: 0,
                category: category.clone(),
                category_tags: vec![category],
                content_type: "text/plain".to_string(),
                content: content.to_string(),
                preview: content.to_string(),
                hash: Storage::hash_content(content),
                pinned: false,
                created_at: Utc::now(),
                original_content: None,
                updated_at: None,
                archived_at: None,
                version: 1,
            };
            assert!(storage.insert(&entry).unwrap());
        }
        let ids = storage
            .query(&QueryFilter::default())
            .unwrap()
            .into_iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>();

        assert_eq!(storage.delete_entries(&ids, true).unwrap(), 2);
        assert_eq!(storage.count(None, false).unwrap(), 0);
        assert_eq!(storage.archive_count().unwrap(), 2);
        assert_eq!(storage.delete_entries(&ids, false).unwrap(), 2);
        assert_eq!(storage.archive_count().unwrap(), 0);

        drop(storage);
        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn empty_archives_permanently_remove_only_archived_records() {
        let db_path = temp_db_path();
        let storage = Storage::new(&db_path).unwrap();
        for content in ["active clipboard", "archived clipboard"] {
            assert!(storage
                .insert(&ClipboardEntry {
                    id: 0,
                    category: Category::Text,
                    category_tags: vec![Category::Text],
                    content_type: "text/plain".to_string(),
                    content: content.to_string(),
                    preview: content.to_string(),
                    hash: Storage::hash_content(content),
                    pinned: false,
                    created_at: Utc::now(),
                    original_content: None,
                    updated_at: None,
                    archived_at: None,
                    version: 1,
                })
                .unwrap());
        }
        let archived_entry_id = storage
            .query(&QueryFilter::default())
            .unwrap()
            .into_iter()
            .find(|entry| entry.content == "archived clipboard")
            .unwrap()
            .id;
        storage.archive_entry(archived_entry_id).unwrap();

        storage.create_memo("Active memo", "keep", "", &[]).unwrap();
        let archived_memo = storage
            .create_memo("Archived memo", "remove", "", &[])
            .unwrap();
        storage.archive_memo(archived_memo.id).unwrap();

        assert_eq!(storage.empty_archive().unwrap(), 1);
        assert_eq!(storage.empty_memo_archive().unwrap(), 1);
        assert_eq!(storage.archive_count().unwrap(), 0);
        assert_eq!(storage.memo_archive_count().unwrap(), 0);
        assert_eq!(storage.count(None, false).unwrap(), 1);
        assert_eq!(storage.memo_count().unwrap(), 1);

        drop(storage);
        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn backup_data_round_trips_between_databases() {
        let source_path = temp_db_path();
        let target_path = temp_db_path();
        let source = Storage::new(&source_path).unwrap();
        let target = Storage::new(&target_path).unwrap();

        let entry = ClipboardEntry {
            id: 0,
            category: Category::Text,
            category_tags: vec![Category::Text],
            content_type: "text".to_string(),
            content: "alpha beta release note".to_string(),
            preview: "alpha beta".to_string(),
            hash: Storage::hash_content("alpha beta release note"),
            pinned: true,
            created_at: Utc::now(),
            original_content: None,
            updated_at: None,
            archived_at: None,
            version: 1,
        };

        source.insert(&entry).unwrap();
        source
            .create_memo("Project note", "remember the backup package", "backup", &[])
            .unwrap();
        source.set_setting("language", "zh-CN").unwrap();

        let data = source.export_backup_data("2.3.2").unwrap();
        let summary = target.restore_backup_data(&data).unwrap();

        assert_eq!(summary.clipboard_entries, 1);
        assert_eq!(summary.memos, 1);
        assert!(summary.settings >= 1);
        assert_eq!(
            target
                .query(&QueryFilter {
                    search: Some("alpha release".to_string()),
                    ..Default::default()
                })
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            target
                .get_memos(&MemoFilter {
                    search: Some("backup package".to_string()),
                    ..Default::default()
                })
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            target.get_setting("language").unwrap(),
            Some("zh-CN".to_string())
        );

        std::fs::remove_file(source_path).ok();
        std::fs::remove_file(target_path).ok();
    }

    #[test]
    fn schema_migration_backfills_persisted_memo_auto_tags() {
        let db_path = temp_db_path();
        let storage = Storage::new(&db_path).unwrap();
        storage
            .create_memo("Contact", "user@example.com", "", &[])
            .unwrap();
        storage.set_setting("schema_version", "3").unwrap();
        drop(storage);

        let migrated = Storage::new(&db_path).unwrap();
        let memos = migrated.get_memos(&MemoFilter::default()).unwrap();
        assert_eq!(memos.len(), 1);
        assert_eq!(memos[0].auto_tags, vec!["email".to_string()]);
        assert_eq!(
            migrated.get_setting("schema_version").unwrap().as_deref(),
            Some(crate::storage_migrations::SCHEMA_VERSION),
        );

        drop(migrated);
        std::fs::remove_file(db_path).ok();
    }

    #[test]
    fn schema_migration_separates_localized_auto_tags_from_manual_tags() {
        let db_path = temp_db_path();
        let storage = Storage::new(&db_path).unwrap();
        let memo = storage
            .create_memo(
                "Contact",
                "user@example.com",
                "project",
                &["email".to_string()],
            )
            .unwrap();
        {
            let conn = storage.conn.lock().unwrap();
            conn.execute(
                "UPDATE memos SET tags = ?1 WHERE id = ?2",
                params!["project,EMAIL,邮箱", memo.id],
            )
            .unwrap();
        }
        storage.set_setting("schema_version", "5").unwrap();
        drop(storage);

        let migrated = Storage::new(&db_path).unwrap();
        let memos = migrated.get_memos(&MemoFilter::default()).unwrap();
        assert_eq!(memos[0].tags, "project");
        assert_eq!(memos[0].auto_tags, vec!["email".to_string()]);
        assert_eq!(
            migrated.get_setting("schema_version").unwrap().as_deref(),
            Some(crate::storage_migrations::SCHEMA_VERSION),
        );

        drop(migrated);
        std::fs::remove_file(db_path).ok();
    }

    #[test]
    fn schema_migration_normalizes_legacy_memo_timestamps_to_utc() {
        let db_path = temp_db_path();
        let storage = Storage::new(&db_path).unwrap();
        let memo = storage.create_memo("Time", "body", "", &[]).unwrap();
        {
            let conn = storage.conn.lock().unwrap();
            conn.execute(
                "UPDATE memos SET created_at = ?1, updated_at = ?2, archived_at = ?3 WHERE id = ?4",
                params![
                    "2026-07-16 00:45:00",
                    "2026-07-16 00:46:00",
                    "2026-07-16 00:47:00",
                    memo.id
                ],
            )
            .unwrap();
        }
        storage.set_setting("schema_version", "6").unwrap();
        drop(storage);

        let migrated = Storage::new(&db_path).unwrap();
        let memo = migrated
            .query_archived_memos(&MemoFilter::default())
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(memo.created_at, "2026-07-16T00:45:00Z");
        assert_eq!(memo.updated_at, "2026-07-16T00:46:00Z");
        assert_eq!(memo.archived_at.as_deref(), Some("2026-07-16T00:47:00Z"));

        drop(migrated);
        std::fs::remove_file(db_path).ok();
    }

    #[test]
    fn manual_reclassification_updates_only_classification_metadata() {
        let db_path = temp_db_path();
        let storage = Storage::new(&db_path).unwrap();
        let content = "E:/Code/SuperClipboard3/docs/index.html";
        let created_at = Utc::now();
        let original_content = Some("original path".to_string());
        let updated_at = Some("2026-07-17T01:02:03Z".to_string());
        storage
            .insert(&ClipboardEntry {
                id: 0,
                category: Category::Code,
                category_tags: vec![Category::Code, Category::Text],
                content_type: "text/plain".to_string(),
                content: content.to_string(),
                preview: content.to_string(),
                hash: Storage::hash_content(content),
                pinned: false,
                created_at,
                original_content: original_content.clone(),
                updated_at: updated_at.clone(),
                archived_at: None,
                version: 1,
            })
            .unwrap();
        let entry_id = storage.query(&QueryFilter::default()).unwrap()[0].id;
        {
            let conn = storage.conn.lock().unwrap();
            conn.execute(
                "UPDATE clipboard_entries
                 SET original_content = ?1, updated_at = ?2
                 WHERE id = ?3",
                params![original_content, updated_at, entry_id],
            )
            .unwrap();
        }

        assert_eq!(storage.reclassify_clipboard_entries().unwrap(), 1);
        assert_eq!(storage.reclassify_clipboard_entries().unwrap(), 0);
        assert_eq!(
            storage
                .get_setting("classification_rules_applied_version")
                .unwrap()
                .as_deref(),
            Some("1")
        );
        let entries = storage.query(&QueryFilter::default()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].category, Category::FilePath);
        assert_eq!(entries[0].category_tags, vec![Category::FilePath]);
        assert_eq!(entries[0].content, content);
        assert_eq!(entries[0].created_at, created_at);
        assert_eq!(entries[0].original_content, original_content);
        assert_eq!(entries[0].updated_at, updated_at);
        assert!(!entries[0].pinned);

        drop(storage);
        std::fs::remove_file(db_path).ok();
    }

    #[test]
    fn search_ignores_embedded_image_payloads() {
        let db_path = temp_db_path();
        let storage = Storage::new(&db_path).unwrap();
        let image_payload = "iVBORw0KGgoSEARCHPAYLOAD123456";
        let entry = ClipboardEntry {
            id: 0,
            category: Category::Image,
            category_tags: vec![Category::Image],
            content_type: "image/png".to_string(),
            content: image_payload.to_string(),
            preview: "[Image 640x480]".to_string(),
            hash: Storage::hash_content(image_payload),
            pinned: false,
            created_at: Utc::now(),
            original_content: None,
            updated_at: None,
            archived_at: None,
            version: 1,
        };
        storage.insert(&entry).unwrap();
        storage
            .create_memo(
                "Meeting note",
                "before image\n![image](data:image/png;base64,iVBORw0KGgoMEMOPAYLOAD123456)\nafter image",
                "project",
                &["image".to_string()],
            )
            .unwrap();

        assert!(storage
            .query(&QueryFilter {
                search: Some("SEARCHPAYLOAD".to_string()),
                ..Default::default()
            })
            .unwrap()
            .is_empty());
        assert_eq!(
            storage
                .query(&QueryFilter {
                    search: Some("图片 640x480".to_string()),
                    ..Default::default()
                })
                .unwrap()
                .len(),
            1
        );
        assert!(storage
            .get_memos(&MemoFilter {
                search: Some("MEMOPAYLOAD".to_string()),
                ..Default::default()
            })
            .unwrap()
            .is_empty());
        assert_eq!(
            storage
                .get_memos(&MemoFilter {
                    search: Some("before after project 图片".to_string()),
                    ..Default::default()
                })
                .unwrap()
                .len(),
            1
        );

        drop(storage);
        std::fs::remove_file(db_path).ok();
    }

    #[test]
    fn schema_migration_backfills_search_text() {
        let db_path = temp_db_path();
        let storage = Storage::new(&db_path).unwrap();
        let entry = ClipboardEntry {
            id: 0,
            category: Category::Text,
            category_tags: vec![Category::Link],
            content_type: "text/plain".to_string(),
            content: "https://migration.example.com".to_string(),
            preview: "https://migration.example.com".to_string(),
            hash: Storage::hash_content("https://migration.example.com"),
            pinned: false,
            created_at: Utc::now(),
            original_content: None,
            updated_at: None,
            archived_at: None,
            version: 1,
        };
        storage.insert(&entry).unwrap();
        storage
            .create_memo("Migration", "searchable history", "legacy", &[])
            .unwrap();
        {
            let conn = storage.conn.lock().unwrap();
            conn.execute("UPDATE clipboard_entries SET search_text = ''", [])
                .unwrap();
            conn.execute("UPDATE memos SET search_text = ''", [])
                .unwrap();
        }
        storage.set_setting("schema_version", "4").unwrap();
        drop(storage);

        let migrated = Storage::new(&db_path).unwrap();
        assert_eq!(
            migrated
                .query(&QueryFilter {
                    search: Some("migration.example.com 链接".to_string()),
                    ..Default::default()
                })
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            migrated
                .get_memos(&MemoFilter {
                    search: Some("searchable legacy".to_string()),
                    ..Default::default()
                })
                .unwrap()
                .len(),
            1
        );

        drop(migrated);
        std::fs::remove_file(db_path).ok();
    }

    #[test]
    fn update_entry_preserves_first_copied_content() {
        let db_path = temp_db_path();
        let storage = Storage::new(&db_path).unwrap();
        let entry = ClipboardEntry {
            id: 0,
            category: Category::Text,
            category_tags: vec![Category::Text],
            content_type: "text".to_string(),
            content: "before edit".to_string(),
            preview: "before edit".to_string(),
            hash: Storage::hash_content("before edit"),
            pinned: false,
            created_at: Utc::now(),
            original_content: None,
            updated_at: None,
            archived_at: None,
            version: 1,
        };

        storage.insert(&entry).unwrap();
        let id = storage.query(&QueryFilter::default()).unwrap()[0].id;
        assert!(storage.update_entry(id, "after edit").unwrap());

        assert!(storage.update_entry(id, "after another edit").unwrap());

        let updated = storage.get_entry_by_id(id).unwrap().unwrap();
        assert_eq!(updated.content, "after another edit");
        assert_eq!(updated.original_content.as_deref(), Some("before edit"));
        assert!(updated.updated_at.is_some());

        let copied_edited_content = ClipboardEntry {
            id: 0,
            category: Category::Text,
            category_tags: vec![Category::Text],
            content_type: "text/plain".to_string(),
            content: "after another edit".to_string(),
            preview: "after another edit".to_string(),
            hash: Storage::hash_content("after another edit"),
            pinned: false,
            created_at: Utc::now(),
            original_content: None,
            updated_at: None,
            archived_at: None,
            version: 1,
        };
        assert!(!storage.insert(&copied_edited_content).unwrap());

        std::fs::remove_file(db_path).ok();
    }

    #[test]
    fn update_entry_with_unchanged_content_keeps_metadata_empty() {
        let db_path = temp_db_path();
        let storage = Storage::new(&db_path).unwrap();
        let entry = ClipboardEntry {
            id: 0,
            category: Category::Text,
            category_tags: vec![Category::Text],
            content_type: "text/plain".to_string(),
            content: "unchanged".to_string(),
            preview: "unchanged".to_string(),
            hash: Storage::hash_content("unchanged"),
            pinned: false,
            created_at: Utc::now(),
            original_content: None,
            updated_at: None,
            archived_at: None,
            version: 1,
        };
        storage.insert(&entry).unwrap();
        let id = storage.query(&QueryFilter::default()).unwrap()[0].id;

        assert!(storage.update_entry(id, "unchanged").unwrap());
        let stored = storage.get_entry_by_id(id).unwrap().unwrap();
        assert_eq!(stored.content, "unchanged");
        assert!(stored.original_content.is_none());
        assert!(stored.updated_at.is_none());

        std::fs::remove_file(db_path).ok();
    }

    #[test]
    fn reverting_entry_to_original_clears_edit_metadata() {
        let db_path = temp_db_path();
        let storage = Storage::new(&db_path).unwrap();
        let entry = ClipboardEntry {
            id: 0,
            category: Category::Text,
            category_tags: vec![Category::Text],
            content_type: "text/plain".to_string(),
            content: "first copy".to_string(),
            preview: "first copy".to_string(),
            hash: Storage::hash_content("first copy"),
            pinned: false,
            created_at: Utc::now(),
            original_content: None,
            updated_at: None,
            archived_at: None,
            version: 1,
        };
        storage.insert(&entry).unwrap();
        let id = storage.query(&QueryFilter::default()).unwrap()[0].id;

        assert!(storage.update_entry(id, "temporary edit").unwrap());
        assert!(storage.update_entry(id, "first copy").unwrap());
        let stored = storage.get_entry_by_id(id).unwrap().unwrap();
        assert_eq!(stored.content, "first copy");
        assert!(stored.original_content.is_none());
        assert!(stored.updated_at.is_none());

        std::fs::remove_file(db_path).ok();
    }
}
