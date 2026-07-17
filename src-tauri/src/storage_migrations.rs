use crate::classifier::Category;
use crate::memo_tags;
use crate::search_index::{clipboard_search_text, memo_search_text};
use crate::storage::Storage;
use rusqlite::{params, Connection, Result as SqlResult};

pub(crate) const SCHEMA_VERSION: &str = "9";
type MemoMigrationRow = (i64, String, String, String, String);

pub fn run(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS clipboard_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            category TEXT NOT NULL,
            category_tags TEXT NOT NULL DEFAULT '[]',
            content_type TEXT NOT NULL,
            content TEXT NOT NULL,
            preview TEXT NOT NULL DEFAULT '',
            search_text TEXT NOT NULL DEFAULT '',
            hash TEXT NOT NULL UNIQUE,
            content_hash TEXT NOT NULL,
            pinned INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            original_content TEXT,
            updated_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_category ON clipboard_entries(category);
        CREATE INDEX IF NOT EXISTS idx_created_at ON clipboard_entries(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_hash ON clipboard_entries(hash);

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS memos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL DEFAULT '',
            body TEXT NOT NULL DEFAULT '',
            tags TEXT NOT NULL DEFAULT '',
            auto_tags TEXT NOT NULL DEFAULT '[]',
            search_text TEXT NOT NULL DEFAULT '',
            pinned INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );
        CREATE INDEX IF NOT EXISTS idx_memos_updated_at ON memos(updated_at DESC);
        ",
    )?;

    let applied_version = conn.query_row(
        "SELECT COALESCE((SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'schema_version'), 0)",
        [],
        |row| row.get::<_, i64>(0),
    )?;

    // These ALTER statements also cover databases created before explicit versioning.
    let _ = conn.execute_batch("ALTER TABLE clipboard_entries ADD COLUMN original_content TEXT");
    let _ = conn.execute_batch("ALTER TABLE clipboard_entries ADD COLUMN updated_at TEXT");
    let _ =
        conn.execute_batch("ALTER TABLE memos ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0");
    let _ = conn.execute_batch(
        "UPDATE memos SET sort_order = (
            SELECT rn FROM (
                SELECT id, ROW_NUMBER() OVER (ORDER BY pinned DESC, created_at DESC) AS rn FROM memos
            ) ranked WHERE ranked.id = memos.id
        ) WHERE sort_order = 0",
    );
    let _ = conn.execute_batch("ALTER TABLE clipboard_entries ADD COLUMN archived_at TEXT");
    let _ = conn.execute_batch("ALTER TABLE memos ADD COLUMN archived_at TEXT");
    let _ = conn.execute_batch("ALTER TABLE memos ADD COLUMN auto_tags TEXT NOT NULL DEFAULT '[]'");
    let _ = conn.execute_batch(
        "ALTER TABLE clipboard_entries ADD COLUMN category_tags TEXT NOT NULL DEFAULT '[]'",
    );
    let _ = conn.execute_batch(
        "ALTER TABLE clipboard_entries ADD COLUMN search_text TEXT NOT NULL DEFAULT ''",
    );
    let _ = conn.execute_batch("ALTER TABLE clipboard_entries ADD COLUMN content_hash TEXT");
    let _ = conn.execute_batch("ALTER TABLE memos ADD COLUMN search_text TEXT NOT NULL DEFAULT ''");

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_clipboard_active_order
            ON clipboard_entries(pinned DESC, created_at DESC)
            WHERE archived_at IS NULL;
         CREATE INDEX IF NOT EXISTS idx_clipboard_archive_order
            ON clipboard_entries(archived_at DESC)
            WHERE archived_at IS NOT NULL;
         CREATE INDEX IF NOT EXISTS idx_memos_active_order
            ON memos(pinned DESC, sort_order DESC)
            WHERE archived_at IS NULL;
         CREATE INDEX IF NOT EXISTS idx_memos_archive_order
            ON memos(archived_at DESC)
            WHERE archived_at IS NOT NULL;",
    )?;

    if applied_version < 4 {
        let rows = {
            let mut statement = conn.prepare(
                "SELECT id, title, body FROM memos WHERE auto_tags = '[]' OR auto_tags = ''",
            )?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<SqlResult<Vec<_>>>()?;
            rows
        };
        for (id, title, body) in rows {
            let auto_tags = serde_json::to_string(&memo_tags::infer(&title, &body))
                .unwrap_or_else(|_| "[]".to_string());
            conn.execute(
                "UPDATE memos SET auto_tags = ?1 WHERE id = ?2",
                params![auto_tags, id],
            )?;
        }
    }

    if applied_version < 5 {
        let rows = {
            let mut statement = conn.prepare(
                "SELECT id, category, category_tags, content_type, content, preview FROM clipboard_entries",
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
                    ))
                })?
                .collect::<SqlResult<Vec<_>>>()?;
            rows
        };
        for (id, category, category_tags, content_type, content, preview) in rows {
            let tags = category_tags_from_json(category_from_str(&category), Some(category_tags));
            let search_text = clipboard_search_text(&content_type, &content, &preview, &tags);
            conn.execute(
                "UPDATE clipboard_entries SET search_text = ?1 WHERE id = ?2",
                params![search_text, id],
            )?;
        }

        let rows = memo_rows(conn)?;
        for (id, title, body, tags, auto_tags) in rows {
            let auto_tags = serde_json::from_str::<Vec<String>>(&auto_tags).unwrap_or_default();
            let search_text = memo_search_text(&title, &body, &tags, &auto_tags);
            conn.execute(
                "UPDATE memos SET search_text = ?1 WHERE id = ?2",
                params![search_text, id],
            )?;
        }
    }

    if applied_version < 6 {
        for (id, title, body, tags, auto_tags) in memo_rows(conn)? {
            let tags = memo_tags::manual_only(&tags);
            let auto_tags = serde_json::from_str::<Vec<String>>(&auto_tags).unwrap_or_default();
            let search_text = memo_search_text(&title, &body, &tags, &auto_tags);
            conn.execute(
                "UPDATE memos SET tags = ?1, search_text = ?2 WHERE id = ?3",
                params![tags, search_text, id],
            )?;
        }
    }

    if applied_version < 7 {
        conn.execute_batch(
            "UPDATE memos
             SET created_at = replace(created_at, ' ', 'T') || 'Z'
             WHERE created_at GLOB '????-??-?? ??:??:??*';
             UPDATE memos
             SET updated_at = replace(updated_at, ' ', 'T') || 'Z'
             WHERE updated_at GLOB '????-??-?? ??:??:??*';
             UPDATE memos
             SET archived_at = replace(archived_at, ' ', 'T') || 'Z'
             WHERE archived_at GLOB '????-??-?? ??:??:??*';
             UPDATE clipboard_entries
             SET archived_at = replace(archived_at, ' ', 'T') || 'Z'
             WHERE archived_at GLOB '????-??-?? ??:??:??*';",
        )?;
    }

    if applied_version < 8 {
        let rows = {
            let mut statement = conn.prepare("SELECT id, content FROM clipboard_entries")?;
            let rows = statement
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<SqlResult<Vec<_>>>()?;
            rows
        };
        for (id, content) in rows {
            conn.execute(
                "UPDATE clipboard_entries SET content_hash = ?1 WHERE id = ?2",
                params![Storage::hash_content(&content), id],
            )?;
        }
    }

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_content_hash ON clipboard_entries(content_hash)",
    )?;
    conn.execute(
        "INSERT INTO settings (key, value) VALUES ('schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![SCHEMA_VERSION],
    )?;
    Ok(())
}

fn memo_rows(conn: &Connection) -> SqlResult<Vec<MemoMigrationRow>> {
    let mut statement = conn.prepare("SELECT id, title, body, tags, auto_tags FROM memos")?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?
        .collect::<SqlResult<Vec<_>>>()?;
    Ok(rows)
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

fn category_tags_from_json(fallback: Category, value: Option<String>) -> Vec<Category> {
    let tags = value
        .and_then(|json| serde_json::from_str::<Vec<Category>>(&json).ok())
        .unwrap_or_default();
    if tags.is_empty() {
        vec![fallback]
    } else {
        let mut normalized = Vec::new();
        for tag in tags {
            if !normalized.contains(&tag) {
                normalized.push(tag);
            }
        }
        normalized
    }
}
