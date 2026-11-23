mod memo_repository;
pub use memo_repository::*;
mod clipboard_repository;
pub use clipboard_repository::*;
mod transfer;
use crate::classifier::{classify_text_tags, Category, CLASSIFICATION_RULES_VERSION};
use crate::memo_tags;
use crate::remote_migrations;
use crate::search_index::{clipboard_search_text, memo_search_text};
use crate::storage::{
    ClipboardEntry, ClipboardInsertResult, Memo, MemoFilter, MergeMemoRequest, QueryFilter,
    Storage, UpdateResult,
};
use chrono::{DateTime, Utc};
use fallible_iterator::FallibleIterator;
use log::warn;
use native_tls::TlsConnector;
use postgres::types::ToSql;
use postgres::{Client, NoTls, Row, Transaction};
use postgres_native_tls::MakeTlsConnector;
use r2d2::{CustomizeConnection, Pool, PooledConnection};
use r2d2_postgres::PostgresConnectionManager;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use thiserror::Error;
pub(crate) use transfer::{merge_workspace, workspace_snapshot};
use uuid::Uuid;

const DEFAULT_PORT: &str = "5432";
const DEFAULT_SSL_MODE: &str = "prefer";
const REMOTE_SCHEMA_CACHE_PREFIX: &str = "remote_schema_version_";
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
    #[error("Remote storage schema version {0} is newer than this app supports")]
    SchemaTooNew(i64),
    #[error("Remote storage connection pool error: {0}")]
    Pool(#[from] r2d2::Error),
    #[error("Remote storage operation failed: {0}")]
    InvalidOperation(String),
}

pub type RemoteResult<T> = Result<T, RemoteStorageError>;

#[derive(Debug, Clone)]
struct RemoteDbConfig {
    url: String,
    ssl_mode: String,
}

impl RemoteDbConfig {
    fn cache_key(&self) -> String {
        Storage::hash_content(&format!("{}|{}", self.ssl_mode, self.url))
    }
}

type NoTlsManager = PostgresConnectionManager<NoTls>;
type NativeTlsManager = PostgresConnectionManager<MakeTlsConnector>;

#[derive(Debug)]
struct RemoteConnectionCustomizer;

impl CustomizeConnection<Client, postgres::Error> for RemoteConnectionCustomizer {
    fn on_acquire(&self, client: &mut Client) -> Result<(), postgres::Error> {
        client.batch_execute("SET statement_timeout = '8000ms'; SET lock_timeout = '3000ms';")
    }
}

#[derive(Clone)]
enum RemotePool {
    NoTls(Pool<NoTlsManager>),
    NativeTls(Pool<NativeTlsManager>),
}

struct CachedPool {
    key: String,
    generation: u64,
    pool: RemotePool,
}

#[derive(Clone)]
struct PoolSnapshot {
    key: String,
    generation: u64,
    pool: RemotePool,
}

static POOL_CACHE: OnceLock<Mutex<Option<CachedPool>>> = OnceLock::new();
static NEXT_POOL_GENERATION: AtomicU64 = AtomicU64::new(1);

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

fn category_tags_json(tags: &[Category]) -> String {
    serde_json::to_string(&normalize_category_tags(tags.to_vec()))
        .unwrap_or_else(|_| "[\"text\"]".to_string())
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

fn category_tags_from_json(fallback: Category, value: String) -> Vec<Category> {
    let parsed = serde_json::from_str::<Vec<Category>>(&value).unwrap_or_default();
    if parsed.is_empty() {
        vec![fallback]
    } else {
        normalize_category_tags(parsed)
    }
}

fn category_tag_pattern(category: &str) -> String {
    format!("%\"{}\"%", category)
}

fn append_category_filter(
    sql: &mut String,
    values: &mut Vec<Box<dyn ToSql + Sync>>,
    category: &str,
    include_auxiliary_tags: bool,
) {
    values.push(Box::new(category.to_string()));
    let category_index = values.len();
    if include_auxiliary_tags {
        values.push(Box::new(category_tag_pattern(category)));
        let tag_index = values.len();
        sql.push_str(&format!(
            " AND (category = ${category_index} OR category_tags LIKE ${tag_index})"
        ));
    } else {
        sql.push_str(&format!(" AND category = ${category_index}"));
    }
}

pub fn is_remote_mode(storage: &Storage) -> bool {
    matches!(storage.get_setting("storage_mode"), Ok(Some(mode)) if mode == "remote")
        && matches!(storage.get_setting("remote_db_ready"), Ok(Some(ready)) if ready == "true")
        && remote_config(storage).is_ok()
}

pub fn is_schema_current(storage: &Storage) -> bool {
    let Ok(config) = remote_config(storage) else {
        return false;
    };
    let key = format!("{REMOTE_SCHEMA_CACHE_PREFIX}{}", config.cache_key());
    matches!(
        storage.get_setting(&key),
        Ok(Some(version)) if version == remote_migrations::VERSION.to_string()
    )
}

fn setting(storage: &Storage, key: &str) -> Option<String> {
    storage
        .get_setting(key)
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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
    values: &mut Vec<Box<dyn ToSql + Sync>>,
    search: &str,
    columns: &[&str],
) {
    for token in search_tokens(search) {
        let pattern = format!("%{}%", token);
        let mut clauses = Vec::with_capacity(columns.len());
        for column in columns {
            values.push(Box::new(pattern.clone()));
            clauses.push(format!("{column} ILIKE ${}", values.len()));
        }
        sql.push_str(" AND (");
        sql.push_str(&clauses.join(" OR "));
        sql.push(')');
    }
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
        url.push_str(&format!(
            "?sslmode={}&connect_timeout=4&keepalives_idle=15",
            ssl_mode
        ));
        return Ok(RemoteDbConfig { url, ssl_mode });
    }

    let mut url = if let Some(protected) = setting(storage, "remote_db_url_protected") {
        crate::workspace_config::credentials::decode(&protected)
            .map_err(RemoteStorageError::InvalidOperation)?
    } else {
        setting(storage, "remote_db_url").ok_or(RemoteStorageError::NotConfigured)?
    };
    if !url.starts_with("postgres://") && !url.starts_with("postgresql://") {
        return Err(RemoteStorageError::InvalidUrl(
            "URL must start with postgres:// or postgresql://".to_string(),
        ));
    }
    let mut parsed = url::Url::parse(&url)
        .map_err(|_| RemoteStorageError::InvalidUrl("Malformed PostgreSQL URI".into()))?;
    let mut query: Vec<(String, String)> = parsed.query_pairs().into_owned().collect();
    let ssl_mode = query
        .iter()
        .rev()
        .find(|(key, _)| key == "sslmode")
        .map(|(_, value)| value.clone())
        .unwrap_or(ssl_mode);
    // The native TLS connector validates certificates and hostnames when TLS is required.
    let driver_mode = match ssl_mode.as_str() {
        "verify-full" => "require",
        "disable" | "prefer" | "require" => &ssl_mode,
        _ => {
            return Err(RemoteStorageError::InvalidUrl(
                "Unsupported PostgreSQL TLS mode".into(),
            ))
        }
    };
    query.retain(|(key, _)| key != "sslmode");
    parsed
        .query_pairs_mut()
        .clear()
        .extend_pairs(query)
        .append_pair("sslmode", driver_mode);
    url = parsed.to_string();
    if !url.contains("connect_timeout=") {
        let separator = if url.contains('?') { '&' } else { '?' };
        url.push(separator);
        url.push_str("connect_timeout=4");
    }
    if !url.contains("keepalives_idle=") {
        let separator = if url.contains('?') { '&' } else { '?' };
        url.push(separator);
        url.push_str("keepalives_idle=15");
    }
    Ok(RemoteDbConfig { url, ssl_mode })
}

fn pool_builder<M>() -> r2d2::Builder<M>
where
    M: r2d2::ManageConnection<Connection = Client, Error = postgres::Error>,
{
    Pool::builder()
        .max_size(4)
        .min_idle(Some(0))
        .connection_timeout(Duration::from_secs(3))
        .idle_timeout(Some(Duration::from_secs(5 * 60)))
        .max_lifetime(Some(Duration::from_secs(15 * 60)))
        .test_on_check_out(true)
        .connection_customizer(Box::new(RemoteConnectionCustomizer))
}

fn build_no_tls_pool(config: &RemoteDbConfig) -> RemoteResult<RemotePool> {
    let pg_config = config.url.parse()?;
    let manager = PostgresConnectionManager::new(pg_config, NoTls);
    Ok(RemotePool::NoTls(pool_builder().build(manager)?))
}

fn build_native_tls_pool(config: &RemoteDbConfig) -> RemoteResult<RemotePool> {
    let pg_config = config.url.parse()?;
    let connector = MakeTlsConnector::new(TlsConnector::builder().build()?);
    let manager = PostgresConnectionManager::new(pg_config, connector);
    Ok(RemotePool::NativeTls(pool_builder().build(manager)?))
}

fn build_pool(config: &RemoteDbConfig) -> RemoteResult<RemotePool> {
    if config.ssl_mode == "disable" {
        return build_no_tls_pool(config);
    }

    let tls_pool = build_native_tls_pool(config)?;
    if config.ssl_mode != "prefer" || test_pool_checkout(&tls_pool).is_ok() {
        return Ok(tls_pool);
    }

    build_no_tls_pool(config)
}

fn connect_direct(config: &RemoteDbConfig) -> RemoteResult<Client> {
    match config.ssl_mode.as_str() {
        "disable" => Ok(Client::connect(&config.url, NoTls)?),
        "prefer" => {
            let connector = MakeTlsConnector::new(TlsConnector::builder().build()?);
            match Client::connect(&config.url, connector) {
                Ok(client) => Ok(client),
                Err(_) => Ok(Client::connect(&config.url, NoTls)?),
            }
        }
        _ => {
            let connector = MakeTlsConnector::new(TlsConnector::builder().build()?);
            Ok(Client::connect(&config.url, connector)?)
        }
    }
}

pub fn listen_for_changes(
    storage: &Storage,
    stop: &AtomicBool,
    mut on_change: impl FnMut(&str),
) -> RemoteResult<()> {
    let config = remote_config(storage)?;
    let mut client = connect_direct(&config)?;
    client.batch_execute("LISTEN superclipboard_v4_changes")?;
    while !stop.load(Ordering::Relaxed) {
        let notification = {
            let mut notifications = client.notifications();
            let next = notifications.timeout_iter(Duration::from_secs(1)).next()?;
            next
        };
        match notification {
            Some(notification) => on_change(notification.payload()),
            None if client.is_closed() => {
                return Err(RemoteStorageError::InvalidUrl(
                    "Remote notification connection closed".to_string(),
                ));
            }
            None => {}
        }
    }

    Ok(())
}

fn test_pool_checkout(pool: &RemotePool) -> RemoteResult<()> {
    match pool {
        RemotePool::NoTls(pool) => {
            let _ = pool.get()?;
        }
        RemotePool::NativeTls(pool) => {
            let _ = pool.get()?;
        }
    }
    Ok(())
}

fn cached_pool(config: &RemoteDbConfig) -> RemoteResult<PoolSnapshot> {
    let key = config.cache_key();
    let cache = POOL_CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache
        .lock()
        .map_err(|_| RemoteStorageError::CacheUnavailable)?;

    if !matches!(guard.as_ref(), Some(cached) if cached.key == key) {
        *guard = Some(CachedPool {
            key,
            generation: NEXT_POOL_GENERATION.fetch_add(1, Ordering::Relaxed),
            pool: build_pool(config)?,
        });
    }

    guard
        .as_ref()
        .map(|cached| PoolSnapshot {
            key: cached.key.clone(),
            generation: cached.generation,
            pool: cached.pool.clone(),
        })
        .ok_or(RemoteStorageError::CacheUnavailable)
}

fn invalidate_pool_snapshot(snapshot: &PoolSnapshot) {
    let Some(cache) = POOL_CACHE.get() else {
        return;
    };
    let Ok(mut guard) = cache.lock() else {
        return;
    };
    if matches!(
        guard.as_ref(),
        Some(cached)
            if cached.key == snapshot.key && cached.generation == snapshot.generation
    ) {
        *guard = None;
    }
}

enum PooledActionError {
    Checkout(r2d2::Error),
    Operation(RemoteStorageError),
}

fn pool_state(pool: &RemotePool) -> (u32, u32) {
    let state = match pool {
        RemotePool::NoTls(pool) => pool.state(),
        RemotePool::NativeTls(pool) => pool.state(),
    };
    (state.connections, state.idle_connections)
}

fn with_pooled_connection<T>(
    pool: &RemotePool,
    action: &mut impl FnMut(&mut Client) -> RemoteResult<T>,
) -> Result<T, PooledActionError> {
    match pool {
        RemotePool::NoTls(pool) => {
            let mut client: PooledConnection<NoTlsManager> =
                pool.get().map_err(PooledActionError::Checkout)?;
            action(&mut client).map_err(PooledActionError::Operation)
        }
        RemotePool::NativeTls(pool) => {
            let mut client: PooledConnection<NativeTlsManager> =
                pool.get().map_err(PooledActionError::Checkout)?;
            action(&mut client).map_err(PooledActionError::Operation)
        }
    }
}

fn with_client<T>(
    storage: &Storage,
    mut action: impl FnMut(&mut Client) -> RemoteResult<T>,
) -> RemoteResult<T> {
    let config = remote_config(storage)?;
    let snapshot = cached_pool(&config)?;
    match with_pooled_connection(&snapshot.pool, &mut action) {
        Ok(value) => Ok(value),
        Err(PooledActionError::Operation(error)) => Err(error),
        Err(PooledActionError::Checkout(error)) => {
            let (connections, idle_connections) = pool_state(&snapshot.pool);
            warn!(
                "Remote pool checkout failed for generation {} (connections: {}, idle: {}); rebuilding once: {}",
                snapshot.generation, connections, idle_connections, error
            );
            invalidate_pool_snapshot(&snapshot);

            let current_config = remote_config(storage)?;
            if current_config.cache_key() != snapshot.key {
                return Err(RemoteStorageError::Pool(error));
            }

            let retry_snapshot = cached_pool(&current_config)?;
            match with_pooled_connection(&retry_snapshot.pool, &mut action) {
                Ok(value) => Ok(value),
                Err(PooledActionError::Operation(error)) => Err(error),
                Err(PooledActionError::Checkout(error)) => Err(RemoteStorageError::Pool(error)),
            }
        }
    }
}

pub fn test_connection(storage: &Storage) -> RemoteResult<String> {
    with_client(storage, |client| {
        let version: String = client.query_one("SELECT version()", &[])?.get(0);
        Ok(version)
    })
}

pub fn workspace_id(storage: &Storage) -> RemoteResult<String> {
    with_client(storage, |client| {
        client.execute(
            "INSERT INTO superclipboard_v4.metadata (key, value) VALUES ('workspace_id', $1)
             ON CONFLICT (key) DO NOTHING",
            &[&Uuid::new_v4().to_string()],
        )?;
        Ok(client
            .query_one(
                "SELECT value FROM superclipboard_v4.metadata WHERE key = 'workspace_id'",
                &[],
            )?
            .get(0))
    })
}

pub(crate) fn schema_marker(storage: &Storage) -> RemoteResult<(String, String)> {
    let config = remote_config(storage)?;
    Ok((
        format!("{REMOTE_SCHEMA_CACHE_PREFIX}{}", config.cache_key()),
        remote_migrations::VERSION.to_string(),
    ))
}

pub fn ensure_schema(storage: &Storage) -> RemoteResult<()> {
    let result = with_client(storage, |client| {
        let mut transaction = client.transaction()?;
        transaction.batch_execute(
            "
            SELECT pg_advisory_xact_lock(78242354);
            CREATE SCHEMA IF NOT EXISTS superclipboard_v4;
            CREATE TABLE IF NOT EXISTS superclipboard_v4.schema_migrations (
                version BIGINT PRIMARY KEY,
                description TEXT NOT NULL,
                applied_at TEXT NOT NULL DEFAULT now()::text
            );
            ",
        )?;
        let applied_version: i64 = transaction
            .query_one(
                "SELECT COALESCE(MAX(version), 0) FROM superclipboard_v4.schema_migrations",
                &[],
            )?
            .get(0);
        if applied_version > remote_migrations::VERSION {
            return Err(RemoteStorageError::SchemaTooNew(applied_version));
        }

        remote_migrations::apply(&mut transaction, applied_version)?;
        transaction.commit()?;
        Ok(())
    });
    if result.is_ok() {
        if let Ok((key, version)) = schema_marker(storage) {
            let _ = storage.set_setting(&key, &version);
        }
    }
    result
}

/// Recompute classification metadata for existing non-image clipboard entries.
/// This is intentionally user-triggered and never runs as part of schema setup.
pub fn reclassify_clipboard_entries(storage: &Storage) -> RemoteResult<u64> {
    with_client(storage, |client| {
        let mut transaction = client.transaction()?;
        let rows = transaction.query(
            "SELECT id, content_type, content, preview
             FROM superclipboard_v4.clipboard_entries
             WHERE deleted_at IS NULL AND content_type NOT LIKE 'image/%'",
            &[],
        )?;

        let mut ids = Vec::with_capacity(rows.len());
        let mut categories = Vec::with_capacity(rows.len());
        let mut category_tags = Vec::with_capacity(rows.len());
        let mut search_texts = Vec::with_capacity(rows.len());
        for row in rows {
            let content_type: String = row.get("content_type");
            let content: String = row.get("content");
            let preview: String = row.get("preview");
            let detected = classify_text_tags(&content);
            let category = detected.first().cloned().unwrap_or(Category::Text);

            ids.push(row.get::<_, i64>("id"));
            categories.push(category.to_string());
            category_tags.push(category_tags_json(&detected));
            search_texts.push(clipboard_search_text(
                &content_type,
                &content,
                &preview,
                &detected,
            ));
        }

        let changed = if ids.is_empty() {
            0
        } else {
            transaction.execute(
                "UPDATE superclipboard_v4.clipboard_entries AS target
             SET category = changes.category,
                 category_tags = changes.category_tags,
                 search_text = changes.search_text
             FROM UNNEST($1::bigint[], $2::text[], $3::text[], $4::text[])
                  AS changes(id, category, category_tags, search_text)
             WHERE target.id = changes.id
               AND (target.category, target.category_tags, target.search_text)
                   IS DISTINCT FROM
                   (changes.category, changes.category_tags, changes.search_text)",
                &[&ids, &categories, &category_tags, &search_texts],
            )?
        };
        transaction.execute(
            "INSERT INTO superclipboard_v4.metadata (key, value, updated_at)
             VALUES ('classification_rules_applied_version', $1, now()::text)
             ON CONFLICT (key) DO UPDATE
             SET value = EXCLUDED.value, updated_at = EXCLUDED.updated_at",
            &[&CLASSIFICATION_RULES_VERSION.to_string()],
        )?;
        transaction.commit()?;
        Ok(changed)
    })
}

pub fn classification_rules_applied_version(storage: &Storage) -> RemoteResult<Option<i64>> {
    with_client(storage, |client| {
        let value = client.query_opt(
            "SELECT value FROM superclipboard_v4.metadata
             WHERE key = 'classification_rules_applied_version'",
            &[],
        )?;
        Ok(value.and_then(|row| row.get::<_, String>(0).parse::<i64>().ok()))
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
    let fallback_category = category_from_str(&category);
    let category_tags: String = row.get("category_tags");
    let category_tags = category_tags_from_json(fallback_category, category_tags);
    let category = category_tags.first().cloned().unwrap_or(Category::Text);
    let created_at: String = row.get("created_at");
    ClipboardEntry {
        id: row.get("id"),
        category,
        category_tags,
        content_type: row.get("content_type"),
        content: row.get("content"),
        preview: row.get("preview"),
        hash: row.get("hash"),
        pinned: row.get("pinned"),
        created_at: parse_created_at(created_at),
        original_content: row.get("original_content"),
        updated_at: row.get("updated_at"),
        archived_at: row.get("archived_at"),
        version: row.get("version"),
    }
}

fn row_to_memo(row: &Row) -> Memo {
    Memo {
        id: row.get("id"),
        title: row.get("title"),
        body: row.get("body"),
        tags: row.get("tags"),
        auto_tags: serde_json::from_str::<Vec<String>>(row.get::<_, String>("auto_tags").as_str())
            .unwrap_or_default(),
        pinned: row.get("pinned"),
        sort_order: row.get("sort_order"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        archived_at: row.get("archived_at"),
        version: row.get("version"),
    }
}

pub fn archive_clipboard(storage: &Storage, id: i64) -> RemoteResult<bool> {
    delete_clipboard(storage, id, true)
}

#[cfg(test)]
mod tests;
