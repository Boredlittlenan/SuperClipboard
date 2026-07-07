use crate::classifier::Category;
use crate::storage::{ClipboardEntry, Memo, MemoFilter, QueryFilter, Storage};
use chrono::{DateTime, Utc};
use native_tls::TlsConnector;
use postgres::types::ToSql;
use postgres::{Client, NoTls, Row};
use postgres_native_tls::MakeTlsConnector;
use std::sync::{Mutex, OnceLock};
use thiserror::Error;
use uuid::Uuid;

const DEFAULT_PORT: &str = "5432";
const DEFAULT_SSL_MODE: &str = "prefer";

#[derive(Debug, Error)]
pub enum RemoteStorageError {
    #[error("Remote storage is not configured")]
    NotConfigured,
    #[error("Remote storage error: {0}")]
    Database(#[from] postgres::Error),
    #[error("TLS error: {0}")]
    Tls(#[from] native_tls::Error),
    #[error("Invalid remote storage URL: {0}")]
    InvalidUrl(String),
    #[error("Remote storage client cache is unavailable")]
    CacheUnavailable,
}

pub type RemoteResult<T> = Result<T, RemoteStorageError>;

#[derive(Debug, Clone)]
struct RemoteDbConfig {
    url: String,
    ssl_mode: String,
}

impl RemoteDbConfig {
    fn cache_key(&self) -> String {
        format!("{}|{}", self.ssl_mode, self.url)
    }
}

struct CachedClient {
    key: String,
    client: Client,
}

static CLIENT_CACHE: OnceLock<Mutex<Option<CachedClient>>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct RemoteStats {
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

pub fn is_remote_mode(storage: &Storage) -> bool {
    matches!(storage.get_setting("storage_mode"), Ok(Some(mode)) if mode == "remote")
        && matches!(storage.get_setting("remote_db_ready"), Ok(Some(ready)) if ready == "true")
        && remote_config(storage).is_ok()
}

fn setting(storage: &Storage, key: &str) -> Option<String> {
    storage
        .get_setting(key)
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn remote_config(storage: &Storage) -> RemoteResult<RemoteDbConfig> {
    let mode = setting(storage, "remote_db_connection_mode").unwrap_or_else(|| "url".into());
    let ssl_mode =
        setting(storage, "remote_db_ssl_mode").unwrap_or_else(|| DEFAULT_SSL_MODE.into());
    if mode == "manual" {
        let host = setting(storage, "remote_db_host").ok_or(RemoteStorageError::NotConfigured)?;
        let port = setting(storage, "remote_db_port").unwrap_or_else(|| DEFAULT_PORT.into());
        let database =
            setting(storage, "remote_db_database").ok_or(RemoteStorageError::NotConfigured)?;
        let username =
            setting(storage, "remote_db_username").ok_or(RemoteStorageError::NotConfigured)?;
        let password = setting(storage, "remote_db_password").unwrap_or_default();
        let mut url = format!(
            "postgresql://{}:{}@{}:{}/{}",
            urlencoding::encode(&username),
            urlencoding::encode(&password),
            host,
            port,
            database
        );
        url.push_str(&format!("?sslmode={}&connect_timeout=5", ssl_mode));
        return Ok(RemoteDbConfig { url, ssl_mode });
    }

    let mut url = setting(storage, "remote_db_url").ok_or(RemoteStorageError::NotConfigured)?;
    if !url.starts_with("postgres://") && !url.starts_with("postgresql://") {
        return Err(RemoteStorageError::InvalidUrl(
            "URL must start with postgres:// or postgresql://".to_string(),
        ));
    }
    if !url.contains("sslmode=") {
        let separator = if url.contains('?') { '&' } else { '?' };
        url.push(separator);
        url.push_str(&format!("sslmode={}", ssl_mode));
    }
    if !url.contains("connect_timeout=") {
        let separator = if url.contains('?') { '&' } else { '?' };
        url.push(separator);
        url.push_str("connect_timeout=5");
    }
    Ok(RemoteDbConfig { url, ssl_mode })
}

fn connect_config(config: &RemoteDbConfig) -> RemoteResult<Client> {
    match config.ssl_mode.as_str() {
        "disable" => Ok(Client::connect(&config.url, NoTls)?),
        "prefer" => {
            let connector = TlsConnector::builder().build()?;
            let connector = MakeTlsConnector::new(connector);
            match Client::connect(&config.url, connector) {
                Ok(client) => Ok(client),
                Err(_) => Ok(Client::connect(&config.url, NoTls)?),
            }
        }
        _ => {
            let connector = TlsConnector::builder().build()?;
            let connector = MakeTlsConnector::new(connector);
            Ok(Client::connect(&config.url, connector)?)
        }
    }
}

fn connect(storage: &Storage) -> RemoteResult<Client> {
    let config = remote_config(storage)?;
    connect_config(&config)
}

fn with_client<T>(
    storage: &Storage,
    mut action: impl FnMut(&mut Client) -> RemoteResult<T>,
) -> RemoteResult<T> {
    let config = remote_config(storage)?;
    let key = config.cache_key();
    let cache = CLIENT_CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache
        .lock()
        .map_err(|_| RemoteStorageError::CacheUnavailable)?;

    if !matches!(guard.as_ref(), Some(cached) if cached.key == key) {
        *guard = Some(CachedClient {
            key: key.clone(),
            client: connect_config(&config)?,
        });
    }

    let result = {
        let cached = guard.as_mut().ok_or(RemoteStorageError::CacheUnavailable)?;
        action(&mut cached.client)
    };

    match result {
        Ok(value) => Ok(value),
        Err(RemoteStorageError::Database(_)) => {
            *guard = Some(CachedClient {
                key,
                client: connect_config(&config)?,
            });
            let cached = guard.as_mut().ok_or(RemoteStorageError::CacheUnavailable)?;
            action(&mut cached.client)
        }
        Err(err) => Err(err),
    }
}

pub fn test_connection(storage: &Storage) -> RemoteResult<String> {
    with_client(storage, |client| {
        let version: String = client.query_one("SELECT version()", &[])?.get(0);
        Ok(version)
    })
}

pub fn ensure_schema(storage: &Storage) -> RemoteResult<()> {
    with_client(storage, |client| {
        client.batch_execute(
        "
        CREATE SCHEMA IF NOT EXISTS superclipboard;

        CREATE TABLE IF NOT EXISTS superclipboard.clipboard_entries (
            id BIGSERIAL PRIMARY KEY,
            uuid TEXT NOT NULL UNIQUE,
            category TEXT NOT NULL,
            content_type TEXT NOT NULL,
            content TEXT NOT NULL,
            preview TEXT NOT NULL DEFAULT '',
            hash TEXT NOT NULL UNIQUE,
            pinned BOOLEAN NOT NULL DEFAULT false,
            created_at TEXT NOT NULL,
            original_content TEXT,
            updated_at TEXT,
            archived_at TEXT,
            deleted_at TEXT,
            version BIGINT NOT NULL DEFAULT 1
        );

        CREATE INDEX IF NOT EXISTS idx_sc_clipboard_category ON superclipboard.clipboard_entries(category);
        CREATE INDEX IF NOT EXISTS idx_sc_clipboard_created_at ON superclipboard.clipboard_entries(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_sc_clipboard_archived_at ON superclipboard.clipboard_entries(archived_at);

        CREATE TABLE IF NOT EXISTS superclipboard.memos (
            id BIGSERIAL PRIMARY KEY,
            uuid TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL DEFAULT '',
            body TEXT NOT NULL DEFAULT '',
            tags TEXT NOT NULL DEFAULT '',
            pinned BOOLEAN NOT NULL DEFAULT false,
            sort_order BIGINT NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            archived_at TEXT,
            deleted_at TEXT,
            version BIGINT NOT NULL DEFAULT 1
        );

        CREATE INDEX IF NOT EXISTS idx_sc_memos_sort_order ON superclipboard.memos(sort_order DESC);
        CREATE INDEX IF NOT EXISTS idx_sc_memos_updated_at ON superclipboard.memos(updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_sc_memos_archived_at ON superclipboard.memos(archived_at);

        CREATE TABLE IF NOT EXISTS superclipboard.sync_events (
            event_id BIGSERIAL PRIMARY KEY,
            entity_type TEXT NOT NULL,
            entity_id BIGINT NOT NULL,
            operation TEXT NOT NULL,
            changed_at TEXT NOT NULL
        );

        CREATE OR REPLACE FUNCTION superclipboard.notify_change() RETURNS trigger AS $$
        DECLARE
            payload text;
            row_id bigint;
            operation text;
        BEGIN
            IF TG_OP = 'DELETE' THEN
                row_id := OLD.id;
                operation := 'delete';
            ELSE
                row_id := NEW.id;
                operation := lower(TG_OP);
            END IF;

            INSERT INTO superclipboard.sync_events(entity_type, entity_id, operation, changed_at)
            VALUES (TG_ARGV[0], row_id, operation, now()::text);

            payload := json_build_object(
                'entityType', TG_ARGV[0],
                'entityId', row_id,
                'operation', operation
            )::text;
            PERFORM pg_notify('superclipboard_changes', payload);
            RETURN COALESCE(NEW, OLD);
        END;
        $$ LANGUAGE plpgsql;

        DROP TRIGGER IF EXISTS sc_clipboard_notify ON superclipboard.clipboard_entries;
        CREATE TRIGGER sc_clipboard_notify
        AFTER INSERT OR UPDATE OR DELETE ON superclipboard.clipboard_entries
        FOR EACH ROW EXECUTE FUNCTION superclipboard.notify_change('clipboard');

        DROP TRIGGER IF EXISTS sc_memo_notify ON superclipboard.memos;
        CREATE TRIGGER sc_memo_notify
        AFTER INSERT OR UPDATE OR DELETE ON superclipboard.memos
        FOR EACH ROW EXECUTE FUNCTION superclipboard.notify_change('memo');
        ",
        )?;
        Ok(())
    })
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

fn parse_created_at(value: String) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&value)
        .unwrap_or_else(|_| Utc::now().into())
        .with_timezone(&Utc)
}

fn row_to_entry(row: &Row) -> ClipboardEntry {
    let category: String = row.get("category");
    let created_at: String = row.get("created_at");
    ClipboardEntry {
        id: row.get("id"),
        category: category_from_str(&category),
        content_type: row.get("content_type"),
        content: row.get("content"),
        preview: row.get("preview"),
        hash: row.get("hash"),
        pinned: row.get("pinned"),
        created_at: parse_created_at(created_at),
        original_content: row.get("original_content"),
        updated_at: row.get("updated_at"),
        archived_at: row.get("archived_at"),
    }
}

fn row_to_memo(row: &Row) -> Memo {
    Memo {
        id: row.get("id"),
        title: row.get("title"),
        body: row.get("body"),
        tags: row.get("tags"),
        pinned: row.get("pinned"),
        sort_order: row.get("sort_order"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        archived_at: row.get("archived_at"),
    }
}

pub fn insert_clipboard(storage: &Storage, entry: &ClipboardEntry) -> RemoteResult<bool> {
    let mut client = connect(storage)?;
    let uuid = Uuid::new_v4().to_string();
    let rows = client.execute(
        "INSERT INTO superclipboard.clipboard_entries
         (uuid, category, content_type, content, preview, hash, pinned, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT(hash) DO NOTHING",
        &[
            &uuid,
            &entry.category.to_string(),
            &entry.content_type,
            &entry.content,
            &entry.preview,
            &entry.hash,
            &entry.pinned,
            &entry.created_at.to_rfc3339(),
        ],
    )?;
    Ok(rows > 0)
}

pub fn query_clipboard(
    storage: &Storage,
    filter: &QueryFilter,
) -> RemoteResult<Vec<ClipboardEntry>> {
    let mut sql = String::from(
        "SELECT id, category, content_type, content, preview, hash, pinned, created_at, original_content, updated_at, archived_at
         FROM superclipboard.clipboard_entries WHERE archived_at IS NULL AND deleted_at IS NULL",
    );
    let mut values: Vec<Box<dyn ToSql + Sync>> = Vec::new();

    if let Some(category) = &filter.category {
        values.push(Box::new(category.clone()));
        sql.push_str(&format!(" AND category = ${}", values.len()));
    }
    if let Some(search) = &filter.search {
        let pattern = format!("%{}%", search);
        values.push(Box::new(pattern.clone()));
        sql.push_str(&format!(" AND (content LIKE ${}", values.len()));
        values.push(Box::new(pattern));
        sql.push_str(&format!(" OR preview LIKE ${})", values.len()));
    }

    sql.push_str(" ORDER BY pinned DESC, created_at DESC");
    let limit = filter.limit.unwrap_or(50).clamp(1, 500);
    values.push(Box::new(limit));
    sql.push_str(&format!(" LIMIT ${}", values.len()));
    let offset = filter.offset.unwrap_or(0).max(0);
    if offset > 0 {
        values.push(Box::new(offset));
        sql.push_str(&format!(" OFFSET ${}", values.len()));
    }
    let params: Vec<&(dyn ToSql + Sync)> = values.iter().map(|value| value.as_ref()).collect();
    with_client(storage, |client| {
        Ok(client
            .query(&sql, &params)?
            .iter()
            .map(row_to_entry)
            .collect())
    })
}

pub fn get_clipboard_by_id(storage: &Storage, id: i64) -> RemoteResult<Option<ClipboardEntry>> {
    with_client(storage, |client| {
        let row = client.query_opt(
            "SELECT id, category, content_type, content, preview, hash, pinned, created_at, original_content, updated_at, archived_at
             FROM superclipboard.clipboard_entries WHERE id = $1 AND deleted_at IS NULL",
            &[&id],
        )?;
        Ok(row.as_ref().map(row_to_entry))
    })
}

pub fn delete_clipboard(storage: &Storage, id: i64, archive: bool) -> RemoteResult<bool> {
    let mut client = connect(storage)?;
    let rows = if archive {
        client.execute(
            "UPDATE superclipboard.clipboard_entries SET archived_at = now()::text, version = version + 1 WHERE id = $1 AND archived_at IS NULL",
            &[&id],
        )?
    } else {
        client.execute(
            "UPDATE superclipboard.clipboard_entries SET deleted_at = now()::text, version = version + 1 WHERE id = $1",
            &[&id],
        )?
    };
    Ok(rows > 0)
}

pub fn toggle_clipboard_pin(storage: &Storage, id: i64) -> RemoteResult<bool> {
    let mut client = connect(storage)?;
    let row = client.query_one(
        "UPDATE superclipboard.clipboard_entries SET pinned = NOT pinned, version = version + 1 WHERE id = $1 RETURNING pinned",
        &[&id],
    )?;
    Ok(row.get("pinned"))
}

pub fn update_clipboard(storage: &Storage, id: i64, content: &str) -> RemoteResult<bool> {
    let mut client = connect(storage)?;
    let current = client.query_opt(
        "SELECT content, original_content FROM superclipboard.clipboard_entries WHERE id = $1",
        &[&id],
    )?;
    let Some(row) = current else {
        return Ok(false);
    };
    let current_content: String = row.get("content");
    let original_content: Option<String> = row.get("original_content");
    let original = original_content.unwrap_or(current_content);
    let preview = if content.chars().count() > 200 {
        content.chars().take(200).collect::<String>()
    } else {
        content.to_string()
    };
    let now = Utc::now().to_rfc3339();
    let rows = client.execute(
        "UPDATE superclipboard.clipboard_entries
         SET content = $1, preview = $2, original_content = $3, updated_at = $4, version = version + 1
         WHERE id = $5",
        &[&content, &preview, &original, &now, &id],
    )?;
    Ok(rows > 0)
}

pub fn stats(storage: &Storage) -> RemoteResult<RemoteStats> {
    with_client(storage, |client| {
        let row = client.query_one(
            "
        SELECT
            COUNT(*) FILTER (WHERE ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS total,
            COUNT(*) FILTER (WHERE ce.category = 'text' AND ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS text,
            COUNT(*) FILTER (WHERE ce.category = 'link' AND ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS link,
            COUNT(*) FILTER (WHERE ce.category = 'image' AND ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS image,
            COUNT(*) FILTER (WHERE ce.category = 'code' AND ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS code,
            COUNT(*) FILTER (WHERE ce.category = 'email' AND ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS email,
            COUNT(*) FILTER (WHERE ce.category = 'file_path' AND ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS file_path,
            COUNT(*) FILTER (WHERE ce.archived_at IS NOT NULL AND ce.deleted_at IS NULL)::bigint AS archive,
            COALESCE(SUM(LENGTH(ce.content) + LENGTH(ce.preview) + LENGTH(COALESCE(ce.original_content, '')))
                FILTER (WHERE ce.archived_at IS NULL AND ce.deleted_at IS NULL), 0)::bigint AS clipboard_size,
            (SELECT COUNT(*) FROM superclipboard.memos WHERE archived_at IS NULL AND deleted_at IS NULL)::bigint AS memo_count,
            (SELECT COUNT(*) FROM superclipboard.memos WHERE archived_at IS NOT NULL AND deleted_at IS NULL)::bigint AS memo_archive,
            (SELECT COALESCE(SUM(LENGTH(title) + LENGTH(body) + LENGTH(tags)), 0)::bigint
             FROM superclipboard.memos WHERE archived_at IS NULL AND deleted_at IS NULL) AS memo_size
        FROM superclipboard.clipboard_entries ce
        ",
            &[],
        )?;
        Ok(RemoteStats {
            total: row.get("total"),
            text: row.get("text"),
            link: row.get("link"),
            image: row.get("image"),
            code: row.get("code"),
            email: row.get("email"),
            file_path: row.get("file_path"),
            archive: row.get("archive"),
            memo_count: row.get("memo_count"),
            memo_archive: row.get("memo_archive"),
            clipboard_size: row.get("clipboard_size"),
            memo_size: row.get("memo_size"),
        })
    })
}

pub fn clear_clipboard_unpinned(storage: &Storage, archive: bool) -> RemoteResult<u64> {
    let mut client = connect(storage)?;
    let rows = if archive {
        client.execute(
            "UPDATE superclipboard.clipboard_entries SET archived_at = now()::text, version = version + 1
             WHERE pinned = false AND archived_at IS NULL AND deleted_at IS NULL",
            &[],
        )?
    } else {
        client.execute(
            "UPDATE superclipboard.clipboard_entries SET deleted_at = now()::text, version = version + 1
             WHERE pinned = false AND deleted_at IS NULL",
            &[],
        )?
    };
    Ok(rows)
}

pub fn query_archived_clipboard(
    storage: &Storage,
    filter: &QueryFilter,
) -> RemoteResult<Vec<ClipboardEntry>> {
    let mut sql = String::from(
        "SELECT id, category, content_type, content, preview, hash, pinned, created_at, original_content, updated_at, archived_at
         FROM superclipboard.clipboard_entries WHERE archived_at IS NOT NULL AND deleted_at IS NULL",
    );
    let mut values: Vec<Box<dyn ToSql + Sync>> = Vec::new();
    if let Some(search) = &filter.search {
        let pattern = format!("%{}%", search);
        values.push(Box::new(pattern.clone()));
        sql.push_str(&format!(" AND (content LIKE ${}", values.len()));
        values.push(Box::new(pattern));
        sql.push_str(&format!(" OR preview LIKE ${})", values.len()));
    }
    sql.push_str(" ORDER BY archived_at DESC");
    let limit = filter.limit.unwrap_or(50).clamp(1, 500);
    values.push(Box::new(limit));
    sql.push_str(&format!(" LIMIT ${}", values.len()));
    let params: Vec<&(dyn ToSql + Sync)> = values.iter().map(|value| value.as_ref()).collect();
    with_client(storage, |client| {
        Ok(client
            .query(&sql, &params)?
            .iter()
            .map(row_to_entry)
            .collect())
    })
}

pub fn clipboard_archive_count(storage: &Storage) -> RemoteResult<i64> {
    with_client(storage, |client| {
        Ok(client
            .query_one(
                "SELECT COUNT(*) FROM superclipboard.clipboard_entries WHERE archived_at IS NOT NULL AND deleted_at IS NULL",
                &[],
            )?
            .get(0))
    })
}

pub fn unarchive_clipboard(storage: &Storage, id: i64) -> RemoteResult<bool> {
    let mut client = connect(storage)?;
    Ok(client
        .execute(
            "UPDATE superclipboard.clipboard_entries SET archived_at = NULL, version = version + 1 WHERE id = $1",
            &[&id],
        )? > 0)
}

pub fn permanent_delete_clipboard(storage: &Storage, id: i64) -> RemoteResult<bool> {
    delete_clipboard(storage, id, false)
}

pub fn purge_old_clipboard_archives(storage: &Storage, days: i64) -> RemoteResult<u64> {
    let mut client = connect(storage)?;
    Ok(client.execute(
        "UPDATE superclipboard.clipboard_entries SET deleted_at = now()::text, version = version + 1
         WHERE archived_at IS NOT NULL AND deleted_at IS NULL AND archived_at::timestamptz < now() - ($1::int * interval '1 day')",
        &[&days],
    )?)
}

pub fn query_memos(storage: &Storage, filter: &MemoFilter) -> RemoteResult<Vec<Memo>> {
    let mut sql = String::from(
        "SELECT id, title, body, tags, pinned, sort_order, created_at, updated_at, archived_at
         FROM superclipboard.memos WHERE archived_at IS NULL AND deleted_at IS NULL",
    );
    let mut values: Vec<Box<dyn ToSql + Sync>> = Vec::new();
    if let Some(search) = &filter.search {
        let pattern = format!("%{}%", search);
        values.push(Box::new(pattern.clone()));
        sql.push_str(&format!(" AND (title LIKE ${}", values.len()));
        values.push(Box::new(pattern.clone()));
        sql.push_str(&format!(" OR body LIKE ${}", values.len()));
        values.push(Box::new(pattern));
        sql.push_str(&format!(" OR tags LIKE ${})", values.len()));
    }
    sql.push_str(" ORDER BY pinned DESC, sort_order DESC");
    let limit = filter.limit.unwrap_or(100).clamp(1, 500);
    values.push(Box::new(limit));
    sql.push_str(&format!(" LIMIT ${}", values.len()));
    let params: Vec<&(dyn ToSql + Sync)> = values.iter().map(|value| value.as_ref()).collect();
    with_client(storage, |client| {
        Ok(client
            .query(&sql, &params)?
            .iter()
            .map(row_to_memo)
            .collect())
    })
}

pub fn create_memo(storage: &Storage, title: &str, body: &str, tags: &str) -> RemoteResult<Memo> {
    let mut client = connect(storage)?;
    let uuid = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let row = client.query_one(
        "INSERT INTO superclipboard.memos (uuid, title, body, tags, sort_order, created_at, updated_at)
         VALUES ($1, $2, $3, $4, (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM superclipboard.memos), $5, $5)
         RETURNING id, title, body, tags, pinned, sort_order, created_at, updated_at, archived_at",
        &[&uuid, &title, &body, &tags, &now],
    )?;
    Ok(row_to_memo(&row))
}

pub fn update_memo(
    storage: &Storage,
    id: i64,
    title: &str,
    body: &str,
    tags: &str,
) -> RemoteResult<bool> {
    let mut client = connect(storage)?;
    let now = Utc::now().to_rfc3339();
    Ok(client
        .execute(
            "UPDATE superclipboard.memos SET title = $1, body = $2, tags = $3, updated_at = $4, version = version + 1 WHERE id = $5",
            &[&title, &body, &tags, &now, &id],
        )? > 0)
}

pub fn delete_memo(storage: &Storage, id: i64, archive: bool) -> RemoteResult<bool> {
    let mut client = connect(storage)?;
    let rows = if archive {
        client.execute(
            "UPDATE superclipboard.memos SET archived_at = now()::text, version = version + 1 WHERE id = $1 AND archived_at IS NULL",
            &[&id],
        )?
    } else {
        client.execute(
            "UPDATE superclipboard.memos SET deleted_at = now()::text, version = version + 1 WHERE id = $1",
            &[&id],
        )?
    };
    Ok(rows > 0)
}

pub fn toggle_memo_pin(storage: &Storage, id: i64) -> RemoteResult<bool> {
    let mut client = connect(storage)?;
    Ok(client
        .query_one(
            "UPDATE superclipboard.memos SET pinned = NOT pinned, version = version + 1 WHERE id = $1 RETURNING pinned",
            &[&id],
        )?
        .get("pinned"))
}

pub fn memo_count(storage: &Storage) -> RemoteResult<i64> {
    with_client(storage, |client| {
        Ok(client
            .query_one(
                "SELECT COUNT(*) FROM superclipboard.memos WHERE archived_at IS NULL AND deleted_at IS NULL",
                &[],
            )?
            .get(0))
    })
}

pub fn reorder_memos(storage: &Storage, orders: &[(i64, i64)]) -> RemoteResult<()> {
    let mut client = connect(storage)?;
    let mut tx = client.transaction()?;
    for (id, sort_order) in orders {
        tx.execute(
            "UPDATE superclipboard.memos SET sort_order = $1, version = version + 1 WHERE id = $2",
            &[sort_order, id],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn query_archived_memos(storage: &Storage, filter: &MemoFilter) -> RemoteResult<Vec<Memo>> {
    let mut sql = String::from(
        "SELECT id, title, body, tags, pinned, sort_order, created_at, updated_at, archived_at
         FROM superclipboard.memos WHERE archived_at IS NOT NULL AND deleted_at IS NULL",
    );
    let mut values: Vec<Box<dyn ToSql + Sync>> = Vec::new();
    if let Some(search) = &filter.search {
        let pattern = format!("%{}%", search);
        values.push(Box::new(pattern.clone()));
        sql.push_str(&format!(" AND (title LIKE ${}", values.len()));
        values.push(Box::new(pattern.clone()));
        sql.push_str(&format!(" OR body LIKE ${}", values.len()));
        values.push(Box::new(pattern));
        sql.push_str(&format!(" OR tags LIKE ${})", values.len()));
    }
    sql.push_str(" ORDER BY archived_at DESC");
    let limit = filter.limit.unwrap_or(100).clamp(1, 500);
    values.push(Box::new(limit));
    sql.push_str(&format!(" LIMIT ${}", values.len()));
    let params: Vec<&(dyn ToSql + Sync)> = values.iter().map(|value| value.as_ref()).collect();
    with_client(storage, |client| {
        Ok(client
            .query(&sql, &params)?
            .iter()
            .map(row_to_memo)
            .collect())
    })
}

pub fn memo_archive_count(storage: &Storage) -> RemoteResult<i64> {
    with_client(storage, |client| {
        Ok(client
            .query_one(
                "SELECT COUNT(*) FROM superclipboard.memos WHERE archived_at IS NOT NULL AND deleted_at IS NULL",
                &[],
            )?
            .get(0))
    })
}

pub fn unarchive_memo(storage: &Storage, id: i64) -> RemoteResult<bool> {
    let mut client = connect(storage)?;
    Ok(client.execute(
        "UPDATE superclipboard.memos SET archived_at = NULL, version = version + 1 WHERE id = $1",
        &[&id],
    )? > 0)
}

pub fn permanent_delete_memo(storage: &Storage, id: i64) -> RemoteResult<bool> {
    delete_memo(storage, id, false)
}

pub fn purge_old_memo_archives(storage: &Storage, days: i64) -> RemoteResult<u64> {
    let mut client = connect(storage)?;
    Ok(client.execute(
        "UPDATE superclipboard.memos SET deleted_at = now()::text, version = version + 1
         WHERE archived_at IS NOT NULL AND deleted_at IS NULL AND archived_at::timestamptz < now() - ($1::int * interval '1 day')",
        &[&days],
    )?)
}

pub fn archive_memo(storage: &Storage, id: i64) -> RemoteResult<bool> {
    delete_memo(storage, id, true)
}

pub fn archive_clipboard(storage: &Storage, id: i64) -> RemoteResult<bool> {
    delete_clipboard(storage, id, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;

    #[test]
    #[ignore]
    fn remote_schema_smoke_from_env() {
        let url = std::env::var("SUPERCLIPBOARD_REMOTE_TEST_URL")
            .expect("SUPERCLIPBOARD_REMOTE_TEST_URL is required");
        let db_path =
            std::env::temp_dir().join(format!("superclipboard-remote-smoke-{}.db", Uuid::new_v4()));
        let storage = Storage::new(&db_path).expect("create temp storage");
        storage.set_setting("storage_mode", "remote").unwrap();
        storage
            .set_setting("remote_db_connection_mode", "url")
            .unwrap();
        storage.set_setting("remote_db_url", &url).unwrap();
        storage.set_setting("remote_db_ssl_mode", "prefer").unwrap();

        ensure_schema(&storage).expect("ensure remote schema");
        let version = test_connection(&storage).expect("test remote connection");
        assert!(version.contains("PostgreSQL"));

        let memo = create_memo(
            &storage,
            "Codex remote smoke",
            "remote mode test body",
            "smoke",
        )
        .expect("create remote memo");
        assert!(memo.id > 0);

        let memos = query_memos(&storage, &MemoFilter::default()).expect("query remote memos");
        assert!(memos.iter().any(|item| item.id == memo.id));

        assert!(delete_memo(&storage, memo.id, false).expect("delete remote memo"));

        let entry = ClipboardEntry {
            id: 0,
            category: Category::Text,
            content_type: "text/plain".to_string(),
            content: format!("Codex remote smoke {}", Uuid::new_v4()),
            preview: "Codex remote smoke".to_string(),
            hash: Uuid::new_v4().to_string(),
            pinned: false,
            created_at: Utc::now(),
            original_content: None,
            updated_at: None,
            archived_at: None,
        };
        assert!(insert_clipboard(&storage, &entry).expect("insert remote clipboard"));

        let entries =
            query_clipboard(&storage, &QueryFilter::default()).expect("query remote clipboard");
        let inserted = entries
            .iter()
            .find(|item| item.preview == "Codex remote smoke")
            .expect("inserted remote clipboard entry");
        let current_stats = stats(&storage).expect("query remote stats");
        assert!(current_stats.total >= 1);
        assert!(delete_clipboard(&storage, inserted.id, false).expect("delete remote clipboard"));

        let _ = std::fs::remove_file(db_path);
    }
}
