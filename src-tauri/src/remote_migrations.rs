use crate::memo_tags;
use crate::search_index::memo_search_text;
use crate::storage::Storage;
use postgres::Transaction;

pub const VERSION: i64 = 9;

pub(crate) const SEARCH_BACKFILL_SQL: &str = r#"
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

pub fn apply(
    transaction: &mut Transaction<'_>,
    applied_version: i64,
) -> Result<(), postgres::Error> {
    transaction.batch_execute(
    "
    CREATE SCHEMA IF NOT EXISTS superclipboard;

    CREATE TABLE IF NOT EXISTS superclipboard.metadata (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        updated_at TEXT NOT NULL DEFAULT now()::text
    );

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
        content_hash TEXT NOT NULL DEFAULT '',
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
    ALTER TABLE superclipboard.clipboard_entries
        ADD COLUMN IF NOT EXISTS content_hash TEXT NOT NULL DEFAULT '';
    CREATE INDEX IF NOT EXISTS idx_sc_clipboard_content_hash
        ON superclipboard.clipboard_entries(content_hash);

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
            &[&2_i64, &"notifications and optimistic record versions"],
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
        transaction.batch_execute(SEARCH_BACKFILL_SQL)?;

        transaction.execute(
            "INSERT INTO superclipboard.schema_migrations (version, description) VALUES ($1, $2)",
            &[&4_i64, &"search text excludes embedded image data"],
        )?;
    }
    if applied_version < 5 {
        let rows = transaction.query(
            "SELECT id, title, body, tags, auto_tags FROM superclipboard.memos",
            &[],
        )?;
        for row in rows {
            let id: i64 = row.get("id");
            let title: String = row.get("title");
            let body: String = row.get("body");
            let tags = memo_tags::manual_only(row.get::<_, String>("tags").as_str());
            let auto_tags =
                serde_json::from_str::<Vec<String>>(row.get::<_, String>("auto_tags").as_str())
                    .unwrap_or_default();
            let search_text = memo_search_text(&title, &body, &tags, &auto_tags);
            transaction.execute(
                "UPDATE superclipboard.memos SET tags = $1, search_text = $2 WHERE id = $3",
                &[&tags, &search_text, &id],
            )?;
        }
        transaction.execute(
            "INSERT INTO superclipboard.schema_migrations (version, description) VALUES ($1, $2)",
            &[
                &5_i64,
                &"separate manual memo tags from localized auto tags",
            ],
        )?;
    }
    if applied_version < 6 {
        let rows = transaction.query(
            "SELECT id, content FROM superclipboard.clipboard_entries",
            &[],
        )?;
        for row in rows {
            let id: i64 = row.get("id");
            let content: String = row.get("content");
            let content_hash = Storage::hash_content(&content);
            transaction.execute(
                "UPDATE superclipboard.clipboard_entries SET content_hash = $1 WHERE id = $2",
                &[&content_hash, &id],
            )?;
        }
        transaction.execute(
            "INSERT INTO superclipboard.schema_migrations (version, description) VALUES ($1, $2)",
            &[
                &6_i64,
                &"current clipboard content hashes for deduplication",
            ],
        )?;
    }
    if applied_version < 7 {
        transaction.execute(
            "INSERT INTO superclipboard.schema_migrations (version, description) VALUES ($1, $2)",
            &[&7_i64, &"manual clipboard reclassification support"],
        )?;
    }
    if applied_version < 8 {
        transaction.batch_execute(
            "DELETE FROM superclipboard.clipboard_entries WHERE deleted_at IS NOT NULL;
             DELETE FROM superclipboard.memos WHERE deleted_at IS NOT NULL;
             DROP TABLE IF EXISTS superclipboard.sync_events;",
        )?;
        transaction.execute(
            "INSERT INTO superclipboard.schema_migrations (version, description) VALUES ($1, $2)",
            &[
                &8_i64,
                &"physical deletes and notification-only synchronization",
            ],
        )?;
    }
    if applied_version < 9 {
        transaction.execute(
            "INSERT INTO superclipboard.schema_migrations (version, description) VALUES ($1, $2)",
            &[&9_i64, &"classification rules metadata"],
        )?;
    }

    Ok(())
}
