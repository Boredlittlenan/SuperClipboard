use crate::classifier::{classify_text_tags, Category};
use crate::memo_tags;
use crate::search_index::{clipboard_search_text, memo_search_text};
use crate::storage::{ClipboardEntry, Memo, MemoFilter, QueryFilter, Storage, UpdateResult};
use chrono::{DateTime, Utc};
use fallible_iterator::FallibleIterator;
use native_tls::TlsConnector;
use postgres::types::ToSql;
use postgres::{Client, NoTls, Row};
use postgres_native_tls::MakeTlsConnector;
use r2d2::{CustomizeConnection, Pool, PooledConnection};
use r2d2_postgres::PostgresConnectionManager;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

const DEFAULT_PORT: &str = "5432";
const DEFAULT_SSL_MODE: &str = "prefer";
const REMOTE_SCHEMA_VERSION: i64 = 4;
const REMOTE_SCHEMA_CACHE_PREFIX: &str = "remote_schema_version_";
const REMOTE_SEARCH_BACKFILL_SQL: &str = r#"
    UPDATE superclipboard.clipboard_entries
    SET search_text = regexp_replace(trim(concat_ws(' ',
        CASE WHEN content_type LIKE 'image/%' THEN '' ELSE content END,
        preview,
        CASE WHEN category = 'text' OR category_tags LIKE '%"text"%' THEN 'text 文本' ELSE '' END,
        CASE WHEN category = 'link' OR category_tags LIKE '%"link"%' THEN 'link url 链接' ELSE '' END,
        CASE WHEN category = 'image' OR category_tags LIKE '%"image"%' THEN 'image 图片' ELSE '' END,
        CASE WHEN category = 'code' OR category_tags LIKE '%"code"%' THEN 'code 代码' ELSE '' END,
        CASE WHEN category = 'email' OR category_tags LIKE '%"email"%' THEN 'email 邮箱' ELSE '' END,
        CASE WHEN category = 'file_path' OR category_tags LIKE '%"file_path"%' THEN 'file path 文件 路径' ELSE '' END
    )), '\s+', ' ', 'g');

    UPDATE superclipboard.memos
    SET search_text = regexp_replace(trim(concat_ws(' ',
        title,
        regexp_replace(
            regexp_replace(body, '!\[[^]]*\]\(data:image/[^)]*\)', ' ', 'g'),
            'data:image/[A-Za-z0-9.+-]+;base64,[A-Za-z0-9+/=]+', ' ', 'g'
        ),
        tags,
        auto_tags,
        CASE WHEN auto_tags LIKE '%"text"%' THEN 'text 文本' ELSE '' END,
        CASE WHEN auto_tags LIKE '%"link"%' THEN 'link url 链接' ELSE '' END,
        CASE WHEN auto_tags LIKE '%"image"%' THEN 'image 图片' ELSE '' END,
        CASE WHEN auto_tags LIKE '%"code"%' THEN 'code 代码' ELSE '' END,
        CASE WHEN auto_tags LIKE '%"email"%' THEN 'email 邮箱' ELSE '' END,
        CASE WHEN auto_tags LIKE '%"file_path"%' OR auto_tags LIKE '%"path"%' THEN 'file path 文件 路径' ELSE '' END
    )), '\s+', ' ', 'g');
"#;

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
    pool: RemotePool,
}

static POOL_CACHE: OnceLock<Mutex<Option<CachedPool>>> = OnceLock::new();

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
        Ok(Some(version)) if version == REMOTE_SCHEMA_VERSION.to_string()
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
        url.push_str(&format!("?sslmode={}&connect_timeout=4", ssl_mode));
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
        url.push_str("connect_timeout=4");
    }
    Ok(RemoteDbConfig { url, ssl_mode })
}

fn pool_builder<M>() -> r2d2::Builder<M>
where
    M: r2d2::ManageConnection<Connection = Client, Error = postgres::Error>,
{
    Pool::builder()
        .max_size(4)
        .connection_timeout(Duration::from_secs(4))
        .idle_timeout(Some(Duration::from_secs(10 * 60)))
        .max_lifetime(Some(Duration::from_secs(30 * 60)))
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
    client.batch_execute("LISTEN superclipboard_changes")?;
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

fn cached_pool(config: &RemoteDbConfig) -> RemoteResult<RemotePool> {
    let key = config.cache_key();
    let cache = POOL_CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache
        .lock()
        .map_err(|_| RemoteStorageError::CacheUnavailable)?;

    if !matches!(guard.as_ref(), Some(cached) if cached.key == key) {
        *guard = Some(CachedPool {
            key,
            pool: build_pool(config)?,
        });
    }

    guard
        .as_ref()
        .map(|cached| cached.pool.clone())
        .ok_or(RemoteStorageError::CacheUnavailable)
}

pub fn invalidate_pool() {
    if let Some(cache) = POOL_CACHE.get() {
        if let Ok(mut guard) = cache.lock() {
            *guard = None;
        }
    }
}

fn with_pooled_connection<T>(
    pool: &RemotePool,
    action: &mut impl FnMut(&mut Client) -> RemoteResult<T>,
) -> RemoteResult<T> {
    match pool {
        RemotePool::NoTls(pool) => {
            let mut client: PooledConnection<NoTlsManager> = pool.get()?;
            action(&mut client)
        }
        RemotePool::NativeTls(pool) => {
            let mut client: PooledConnection<NativeTlsManager> = pool.get()?;
            action(&mut client)
        }
    }
}

fn with_client<T>(
    storage: &Storage,
    mut action: impl FnMut(&mut Client) -> RemoteResult<T>,
) -> RemoteResult<T> {
    let config = remote_config(storage)?;
    let pool = cached_pool(&config)?;
    with_pooled_connection(&pool, &mut action)
}

pub fn test_connection(storage: &Storage) -> RemoteResult<String> {
    with_client(storage, |client| {
        let version: String = client.query_one("SELECT version()", &[])?.get(0);
        Ok(version)
    })
}

pub fn ensure_schema(storage: &Storage) -> RemoteResult<()> {
    let result = with_client(storage, |client| {
        let mut transaction = client.transaction()?;
        transaction.batch_execute(
            "
            SELECT pg_advisory_xact_lock(78242351);
            CREATE SCHEMA IF NOT EXISTS superclipboard;
            CREATE TABLE IF NOT EXISTS superclipboard.schema_migrations (
                version BIGINT PRIMARY KEY,
                description TEXT NOT NULL,
                applied_at TEXT NOT NULL DEFAULT now()::text
            );
            ",
        )?;
        let applied_version: i64 = transaction
            .query_one(
                "SELECT COALESCE(MAX(version), 0) FROM superclipboard.schema_migrations",
                &[],
            )?
            .get(0);
        if applied_version > REMOTE_SCHEMA_VERSION {
            return Err(RemoteStorageError::SchemaTooNew(applied_version));
        }

        transaction.batch_execute(
        "
        CREATE SCHEMA IF NOT EXISTS superclipboard;

        CREATE TABLE IF NOT EXISTS superclipboard.clipboard_entries (
            id BIGSERIAL PRIMARY KEY,
            uuid TEXT NOT NULL UNIQUE,
            category TEXT NOT NULL,
            category_tags TEXT NOT NULL DEFAULT '[]',
            content_type TEXT NOT NULL,
            content TEXT NOT NULL,
            preview TEXT NOT NULL DEFAULT '',
            search_text TEXT NOT NULL DEFAULT '',
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
        CREATE INDEX IF NOT EXISTS idx_sc_clipboard_active_order
            ON superclipboard.clipboard_entries(pinned DESC, created_at DESC)
            WHERE archived_at IS NULL AND deleted_at IS NULL;
        ALTER TABLE superclipboard.clipboard_entries
            ADD COLUMN IF NOT EXISTS category_tags TEXT NOT NULL DEFAULT '[]';
        ALTER TABLE superclipboard.clipboard_entries
            ADD COLUMN IF NOT EXISTS search_text TEXT NOT NULL DEFAULT '';

        CREATE TABLE IF NOT EXISTS superclipboard.memos (
            id BIGSERIAL PRIMARY KEY,
            uuid TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL DEFAULT '',
            body TEXT NOT NULL DEFAULT '',
            tags TEXT NOT NULL DEFAULT '',
            auto_tags TEXT NOT NULL DEFAULT '[]',
            search_text TEXT NOT NULL DEFAULT '',
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
        CREATE INDEX IF NOT EXISTS idx_sc_memos_active_order
            ON superclipboard.memos(pinned DESC, sort_order DESC)
            WHERE archived_at IS NULL AND deleted_at IS NULL;
        ALTER TABLE superclipboard.memos
            ADD COLUMN IF NOT EXISTS auto_tags TEXT NOT NULL DEFAULT '[]';
        ALTER TABLE superclipboard.memos
            ADD COLUMN IF NOT EXISTS search_text TEXT NOT NULL DEFAULT '';

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

        if applied_version < 1 {
            transaction.execute(
                "INSERT INTO superclipboard.schema_migrations (version, description) VALUES ($1, $2)",
                &[&1_i64, &"initial clipboard and memo schema"],
            )?;
        }
        if applied_version < 2 {
            transaction.execute(
                "INSERT INTO superclipboard.schema_migrations (version, description) VALUES ($1, $2)",
                &[&2_i64, &"sync events, notifications, and optimistic record versions"],
            )?;
        }
        if applied_version < 3 {
            let rows = transaction.query(
                "SELECT id, title, body FROM superclipboard.memos WHERE auto_tags = '[]' OR auto_tags = ''",
                &[],
            )?;
            for row in rows {
                let id: i64 = row.get("id");
                let title: String = row.get("title");
                let body: String = row.get("body");
                let auto_tags = serde_json::to_string(&memo_tags::infer(&title, &body))
                    .unwrap_or_else(|_| "[]".to_string());
                transaction.execute(
                    "UPDATE superclipboard.memos SET auto_tags = $1 WHERE id = $2",
                    &[&auto_tags, &id],
                )?;
            }
            transaction.execute(
                "INSERT INTO superclipboard.schema_migrations (version, description) VALUES ($1, $2)",
                &[&3_i64, &"persisted memo auto tags"],
            )?;
        }
        if applied_version < 4 {
            transaction.batch_execute(REMOTE_SEARCH_BACKFILL_SQL)?;

            transaction.execute(
                "INSERT INTO superclipboard.schema_migrations (version, description) VALUES ($1, $2)",
                &[&4_i64, &"search text excludes embedded image data"],
            )?;
        }
        transaction.commit()?;
        Ok(())
    });
    if result.is_ok() {
        if let Ok(config) = remote_config(storage) {
            let key = format!("{REMOTE_SCHEMA_CACHE_PREFIX}{}", config.cache_key());
            let _ = storage.set_setting(&key, &REMOTE_SCHEMA_VERSION.to_string());
        }
    }
    result
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

pub fn insert_clipboard(storage: &Storage, entry: &ClipboardEntry) -> RemoteResult<bool> {
    let uuid = Uuid::new_v4().to_string();
    let category_tags = normalize_category_tags(entry.category_tags.clone());
    let category = category_tags.first().cloned().unwrap_or(Category::Text);
    let search_text = clipboard_search_text(
        &entry.content_type,
        &entry.content,
        &entry.preview,
        &category_tags,
    );
    let category_tags = category_tags_json(&category_tags);
    with_client(storage, |client| {
        Ok(client.execute(
            "INSERT INTO superclipboard.clipboard_entries
             (uuid, category, category_tags, content_type, content, preview, search_text, hash, pinned, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT(hash) DO NOTHING",
            &[
                &uuid,
                &category.to_string(),
                &category_tags,
                &entry.content_type,
                &entry.content,
                &entry.preview,
                &search_text,
                &entry.hash,
                &entry.pinned,
                &entry.created_at.to_rfc3339(),
            ],
        )? > 0)
    })
}

pub fn query_clipboard(
    storage: &Storage,
    filter: &QueryFilter,
) -> RemoteResult<Vec<ClipboardEntry>> {
    let mut sql = String::from(
        "SELECT id, category, category_tags, content_type,
                CASE WHEN category = 'image' THEN '' ELSE content END AS content,
                preview, hash, pinned, created_at, original_content, updated_at, archived_at, version
         FROM superclipboard.clipboard_entries WHERE archived_at IS NULL AND deleted_at IS NULL",
    );
    let mut values: Vec<Box<dyn ToSql + Sync>> = Vec::new();

    if let Some(category) = &filter.category {
        values.push(Box::new(category.clone()));
        let category_index = values.len();
        values.push(Box::new(category_tag_pattern(category)));
        let tag_index = values.len();
        sql.push_str(&format!(
            " AND (category = ${category_index} OR category_tags LIKE ${tag_index})"
        ));
    }
    if let Some(search) = &filter.search {
        append_token_search(&mut sql, &mut values, search, &["search_text"]);
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
            "SELECT id, category, category_tags, content_type, content, preview, hash, pinned, created_at, original_content, updated_at, archived_at, version
             FROM superclipboard.clipboard_entries WHERE id = $1 AND deleted_at IS NULL",
            &[&id],
        )?;
        Ok(row.as_ref().map(row_to_entry))
    })
}

pub fn delete_clipboard(storage: &Storage, id: i64, archive: bool) -> RemoteResult<bool> {
    with_client(storage, |client| {
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
    })
}

pub fn toggle_clipboard_pin(storage: &Storage, id: i64) -> RemoteResult<bool> {
    with_client(storage, |client| {
        Ok(client
            .query_one(
                "UPDATE superclipboard.clipboard_entries SET pinned = NOT pinned, version = version + 1 WHERE id = $1 RETURNING pinned",
                &[&id],
            )?
            .get("pinned"))
    })
}

pub fn update_clipboard(
    storage: &Storage,
    id: i64,
    content: &str,
    expected_version: Option<i64>,
) -> RemoteResult<UpdateResult> {
    with_client(storage, |client| {
        let current = client.query_opt(
            "SELECT content, original_content FROM superclipboard.clipboard_entries WHERE id = $1",
            &[&id],
        )?;
        let Some(row) = current else {
            return Ok(UpdateResult::updated(false));
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
        let categories = classify_text_tags(content);
        let category = categories.first().cloned().unwrap_or(Category::Text);
        let search_text = clipboard_search_text("text/plain", content, &preview, &categories);
        let category_tags = category_tags_json(&categories);
        let rows = if let Some(expected_version) = expected_version {
            client.execute(
                "UPDATE superclipboard.clipboard_entries
                 SET category = $1, category_tags = $2, content = $3, preview = $4, search_text = $5, original_content = $6, updated_at = $7, version = version + 1
                 WHERE id = $8 AND version = $9",
                &[&category.to_string(), &category_tags, &content, &preview, &search_text, &original, &now, &id, &expected_version],
            )?
        } else {
            client.execute(
                "UPDATE superclipboard.clipboard_entries
                 SET category = $1, category_tags = $2, content = $3, preview = $4, search_text = $5, original_content = $6, updated_at = $7, version = version + 1
                 WHERE id = $8",
                &[&category.to_string(), &category_tags, &content, &preview, &search_text, &original, &now, &id],
            )?
        };
        if rows > 0 {
            return Ok(UpdateResult::updated(true));
        }
        let exists = client
            .query_opt(
                "SELECT 1 FROM superclipboard.clipboard_entries WHERE id = $1",
                &[&id],
            )?
            .is_some();
        Ok(if exists {
            UpdateResult::conflict()
        } else {
            UpdateResult::updated(false)
        })
    })
}

pub fn stats(storage: &Storage) -> RemoteResult<RemoteStats> {
    with_client(storage, |client| {
        let row = client.query_one(
            "
        SELECT
            COUNT(*) FILTER (WHERE ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS total,
            COUNT(*) FILTER (WHERE (ce.category = 'text' OR ce.category_tags LIKE '%\"text\"%') AND ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS text,
            COUNT(*) FILTER (WHERE (ce.category = 'link' OR ce.category_tags LIKE '%\"link\"%') AND ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS link,
            COUNT(*) FILTER (WHERE (ce.category = 'image' OR ce.category_tags LIKE '%\"image\"%') AND ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS image,
            COUNT(*) FILTER (WHERE (ce.category = 'code' OR ce.category_tags LIKE '%\"code\"%') AND ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS code,
            COUNT(*) FILTER (WHERE (ce.category = 'email' OR ce.category_tags LIKE '%\"email\"%') AND ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS email,
            COUNT(*) FILTER (WHERE (ce.category = 'file_path' OR ce.category_tags LIKE '%\"file_path\"%') AND ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS file_path,
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
    with_client(storage, |client| {
        if archive {
            Ok(client.execute(
                "UPDATE superclipboard.clipboard_entries SET archived_at = now()::text, version = version + 1
                 WHERE pinned = false AND archived_at IS NULL AND deleted_at IS NULL",
                &[],
            )?)
        } else {
            Ok(client.execute(
                "UPDATE superclipboard.clipboard_entries SET deleted_at = now()::text, version = version + 1
                 WHERE pinned = false AND deleted_at IS NULL",
                &[],
            )?)
        }
    })
}

pub fn query_archived_clipboard(
    storage: &Storage,
    filter: &QueryFilter,
) -> RemoteResult<Vec<ClipboardEntry>> {
    let mut sql = String::from(
        "SELECT id, category, category_tags, content_type,
                CASE WHEN category = 'image' THEN '' ELSE content END AS content,
                preview, hash, pinned, created_at, original_content, updated_at, archived_at, version
         FROM superclipboard.clipboard_entries WHERE archived_at IS NOT NULL AND deleted_at IS NULL",
    );
    let mut values: Vec<Box<dyn ToSql + Sync>> = Vec::new();
    if let Some(category) = &filter.category {
        values.push(Box::new(category.clone()));
        let category_index = values.len();
        values.push(Box::new(category_tag_pattern(category)));
        let tag_index = values.len();
        sql.push_str(&format!(
            " AND (category = ${category_index} OR category_tags LIKE ${tag_index})"
        ));
    }
    if let Some(search) = &filter.search {
        append_token_search(&mut sql, &mut values, search, &["search_text"]);
    }
    sql.push_str(" ORDER BY archived_at DESC");
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
    with_client(storage, |client| {
        Ok(client.execute(
            "UPDATE superclipboard.clipboard_entries SET archived_at = NULL, version = version + 1 WHERE id = $1",
            &[&id],
        )? > 0)
    })
}

pub fn permanent_delete_clipboard(storage: &Storage, id: i64) -> RemoteResult<bool> {
    delete_clipboard(storage, id, false)
}

pub fn purge_old_clipboard_archives(storage: &Storage, days: i64) -> RemoteResult<u64> {
    with_client(storage, |client| {
        Ok(client.execute(
            "UPDATE superclipboard.clipboard_entries SET deleted_at = now()::text, version = version + 1
             WHERE archived_at IS NOT NULL AND deleted_at IS NULL AND archived_at::timestamptz < now() - ($1::int * interval '1 day')",
            &[&days],
        )?)
    })
}

pub fn query_memos(storage: &Storage, filter: &MemoFilter) -> RemoteResult<Vec<Memo>> {
    let mut sql = String::from(
        "SELECT id, title, body, tags, auto_tags, pinned, sort_order, created_at, updated_at, archived_at, version
         FROM superclipboard.memos WHERE archived_at IS NULL AND deleted_at IS NULL",
    );
    let mut values: Vec<Box<dyn ToSql + Sync>> = Vec::new();
    if let Some(search) = &filter.search {
        append_token_search(&mut sql, &mut values, search, &["search_text"]);
    }
    sql.push_str(" ORDER BY pinned DESC, sort_order DESC");
    let limit = filter.limit.unwrap_or(100).clamp(1, 500);
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
            .map(row_to_memo)
            .collect())
    })
}

pub fn create_memo(
    storage: &Storage,
    title: &str,
    body: &str,
    tags: &str,
    auto_tags: &[String],
) -> RemoteResult<Memo> {
    let uuid = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let search_text = memo_search_text(title, body, tags, auto_tags);
    let auto_tags = serde_json::to_string(auto_tags).unwrap_or_else(|_| "[]".to_string());
    with_client(storage, |client| {
        let row = client.query_one(
            "INSERT INTO superclipboard.memos (uuid, title, body, tags, auto_tags, search_text, sort_order, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM superclipboard.memos), $7, $7)
             RETURNING id, title, body, tags, auto_tags, pinned, sort_order, created_at, updated_at, archived_at, version",
            &[&uuid, &title, &body, &tags, &auto_tags, &search_text, &now],
        )?;
        Ok(row_to_memo(&row))
    })
}

pub fn update_memo(
    storage: &Storage,
    id: i64,
    title: &str,
    body: &str,
    tags: &str,
    auto_tags: &[String],
    expected_version: Option<i64>,
) -> RemoteResult<UpdateResult> {
    let now = Utc::now().to_rfc3339();
    let search_text = memo_search_text(title, body, tags, auto_tags);
    let auto_tags = serde_json::to_string(auto_tags).unwrap_or_else(|_| "[]".to_string());
    with_client(storage, |client| {
        let rows = if let Some(expected_version) = expected_version {
            client.execute(
                "UPDATE superclipboard.memos SET title = $1, body = $2, tags = $3, auto_tags = $4, search_text = $5, updated_at = $6, version = version + 1 WHERE id = $7 AND version = $8",
                &[&title, &body, &tags, &auto_tags, &search_text, &now, &id, &expected_version],
            )?
        } else {
            client.execute(
                "UPDATE superclipboard.memos SET title = $1, body = $2, tags = $3, auto_tags = $4, search_text = $5, updated_at = $6, version = version + 1 WHERE id = $7",
                &[&title, &body, &tags, &auto_tags, &search_text, &now, &id],
            )?
        };
        if rows > 0 {
            return Ok(UpdateResult::updated(true));
        }
        let exists = client
            .query_opt("SELECT 1 FROM superclipboard.memos WHERE id = $1", &[&id])?
            .is_some();
        Ok(if exists {
            UpdateResult::conflict()
        } else {
            UpdateResult::updated(false)
        })
    })
}

pub fn delete_memo(storage: &Storage, id: i64, archive: bool) -> RemoteResult<bool> {
    with_client(storage, |client| {
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
    })
}

pub fn toggle_memo_pin(storage: &Storage, id: i64) -> RemoteResult<bool> {
    with_client(storage, |client| {
        Ok(client.query_one(
            "UPDATE superclipboard.memos SET pinned = NOT pinned, version = version + 1 WHERE id = $1 RETURNING pinned",
            &[&id],
        )?
        .get("pinned"))
    })
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
    with_client(storage, |client| {
        let mut tx = client.transaction()?;
        for (id, sort_order) in orders {
            tx.execute(
                "UPDATE superclipboard.memos SET sort_order = $1, version = version + 1 WHERE id = $2",
                &[sort_order, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    })
}

pub fn query_archived_memos(storage: &Storage, filter: &MemoFilter) -> RemoteResult<Vec<Memo>> {
    let mut sql = String::from(
        "SELECT id, title, body, tags, auto_tags, pinned, sort_order, created_at, updated_at, archived_at, version
         FROM superclipboard.memos WHERE archived_at IS NOT NULL AND deleted_at IS NULL",
    );
    let mut values: Vec<Box<dyn ToSql + Sync>> = Vec::new();
    if let Some(search) = &filter.search {
        append_token_search(&mut sql, &mut values, search, &["search_text"]);
    }
    sql.push_str(" ORDER BY archived_at DESC");
    let limit = filter.limit.unwrap_or(100).clamp(1, 500);
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
    with_client(storage, |client| {
        Ok(client.execute(
            "UPDATE superclipboard.memos SET archived_at = NULL, version = version + 1 WHERE id = $1",
            &[&id],
        )? > 0)
    })
}

pub fn permanent_delete_memo(storage: &Storage, id: i64) -> RemoteResult<bool> {
    delete_memo(storage, id, false)
}

pub fn purge_old_memo_archives(storage: &Storage, days: i64) -> RemoteResult<u64> {
    with_client(storage, |client| {
        Ok(client.execute(
            "UPDATE superclipboard.memos SET deleted_at = now()::text, version = version + 1
             WHERE archived_at IS NOT NULL AND deleted_at IS NULL AND archived_at::timestamptz < now() - ($1::int * interval '1 day')",
            &[&days],
        )?)
    })
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
    fn cache_key_is_stable_without_exposing_credentials() {
        let config = RemoteDbConfig {
            url: "postgresql://user:secret@example.test/database".to_string(),
            ssl_mode: "require".to_string(),
        };
        let key = config.cache_key();
        assert_eq!(key, config.cache_key());
        assert!(!key.contains("secret"));
        assert_ne!(
            key,
            RemoteDbConfig {
                url: "postgresql://user:other@example.test/database".to_string(),
                ssl_mode: "require".to_string(),
            }
            .cache_key()
        );
    }

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
        assert!(is_schema_current(&storage));
        with_client(&storage, |client| {
            let mut transaction = client.transaction()?;
            transaction.batch_execute(REMOTE_SEARCH_BACKFILL_SQL)?;
            transaction.rollback()?;
            Ok(())
        })
        .expect("validate remote search backfill");
        let version = test_connection(&storage).expect("test remote connection");
        assert!(version.contains("PostgreSQL"));

        let memo_token = format!("RemoteMemo{}", Uuid::new_v4().simple());
        let memo_payload = format!("iVBORw0KGgo{}", Uuid::new_v4().simple());
        let memo = create_memo(
            &storage,
            &memo_token,
            &format!("remote mode test body\n![image](data:image/png;base64,{memo_payload})"),
            "smoke",
            &["image".to_string()],
        )
        .expect("create remote memo");
        assert!(memo.id > 0);

        let memos = query_memos(
            &storage,
            &MemoFilter {
                search: Some(memo_token.clone()),
                ..Default::default()
            },
        )
        .expect("search remote memos");
        assert!(memos.iter().any(|item| item.id == memo.id));
        let payload_matches = query_memos(
            &storage,
            &MemoFilter {
                search: Some(memo_payload),
                ..Default::default()
            },
        )
        .expect("exclude remote memo image payload");
        assert!(payload_matches.iter().all(|item| item.id != memo.id));

        let conflict = update_memo(
            &storage,
            memo.id,
            &memo_token,
            "stale update",
            "smoke",
            &[],
            Some(memo.version + 1),
        )
        .expect("detect remote memo conflict");
        assert!(conflict.conflict);

        let updated = update_memo(
            &storage,
            memo.id,
            &memo_token,
            "updated remote mode test body",
            "smoke",
            &[],
            Some(memo.version),
        )
        .expect("update remote memo");
        assert!(updated.updated);

        assert!(delete_memo(&storage, memo.id, false).expect("delete remote memo"));

        let text_token = format!("RemoteText{}", Uuid::new_v4().simple());
        let entry = ClipboardEntry {
            id: 0,
            category: Category::Text,
            category_tags: vec![Category::Text],
            content_type: "text/plain".to_string(),
            content: format!("Codex remote smoke {text_token}"),
            preview: text_token.clone(),
            hash: Uuid::new_v4().to_string(),
            pinned: false,
            created_at: Utc::now(),
            original_content: None,
            updated_at: None,
            archived_at: None,
            version: 1,
        };
        assert!(insert_clipboard(&storage, &entry).expect("insert remote clipboard"));

        let entries = query_clipboard(
            &storage,
            &QueryFilter {
                search: Some(text_token),
                ..Default::default()
            },
        )
        .expect("query remote clipboard");
        let inserted = entries
            .iter()
            .find(|item| item.hash == entry.hash)
            .expect("inserted remote clipboard entry");
        let current_stats = stats(&storage).expect("query remote stats");
        assert!(current_stats.total >= 1);
        assert!(delete_clipboard(&storage, inserted.id, false).expect("delete remote clipboard"));

        let image_preview_token = format!("RemoteImage{}", Uuid::new_v4().simple());
        let image_payload = format!("iVBORw0KGgo{}", Uuid::new_v4().simple());
        let image_entry = ClipboardEntry {
            id: 0,
            category: Category::Image,
            category_tags: vec![Category::Image],
            content_type: "image/png".to_string(),
            content: image_payload.clone(),
            preview: format!("[Image {image_preview_token}]"),
            hash: Uuid::new_v4().to_string(),
            pinned: false,
            created_at: Utc::now(),
            original_content: None,
            updated_at: None,
            archived_at: None,
            version: 1,
        };
        assert!(insert_clipboard(&storage, &image_entry).expect("insert remote image"));
        let payload_matches = query_clipboard(
            &storage,
            &QueryFilter {
                search: Some(image_payload),
                ..Default::default()
            },
        )
        .expect("exclude remote clipboard image payload");
        assert!(payload_matches
            .iter()
            .all(|item| item.hash != image_entry.hash));
        let image_matches = query_clipboard(
            &storage,
            &QueryFilter {
                search: Some(image_preview_token),
                ..Default::default()
            },
        )
        .expect("search remote image metadata");
        let image = image_matches
            .iter()
            .find(|item| item.hash == image_entry.hash)
            .expect("inserted remote image entry");
        assert!(delete_clipboard(&storage, image.id, false).expect("delete remote image"));

        let _ = std::fs::remove_file(db_path);
    }
}
