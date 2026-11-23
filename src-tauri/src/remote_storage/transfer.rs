use super::{with_client, RemoteResult, RemoteStorageError};
use crate::storage::{BackupData, ClipboardEntry, Memo};
use crate::transfer::{merge::ImportResult, prepare::Prepared, source};
use postgres::IsolationLevel;
use serde_json::{json, Value};
use uuid::Uuid;

pub(crate) fn workspace_snapshot(
    storage: &super::Storage,
    legacy: bool,
) -> RemoteResult<BackupData> {
    let schema = if legacy {
        "superclipboard"
    } else {
        "superclipboard_v4"
    };
    with_client(storage, |client| {
        let mut tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        let clips = read_table(&mut tx, schema, "clipboard_entries")?
            .into_iter()
            .map(|mut row| {
                let created = row["created_at"].as_str().unwrap_or_default();
                row["created_at"] = json!(
                    source::timestamp(created).map_err(RemoteStorageError::InvalidOperation)?
                );
                decode_list(&mut row, "category_tags")?;
                convert::<ClipboardEntry>(row)
            })
            .collect::<RemoteResult<Vec<_>>>()?;
        let memos = read_table(&mut tx, schema, "memos")?
            .into_iter()
            .map(|mut row| {
                decode_list(&mut row, "auto_tags")?;
                if row["sort_order"].is_null() {
                    row["sort_order"] = row["id"].clone();
                }
                convert::<Memo>(row)
            })
            .collect::<RemoteResult<Vec<_>>>()?;
        tx.commit()?;
        Ok(source::snapshot(
            clips,
            memos,
            if legacy {
                "PostgreSQL 2.x / 3.x"
            } else {
                crate::APP_VERSION
            },
        ))
    })
}

fn convert<T: serde::de::DeserializeOwned>(row: Value) -> RemoteResult<T> {
    let id = row["id"].clone();
    serde_json::from_value(row)
        .map_err(|e| RemoteStorageError::InvalidOperation(format!("Record #{id}: {e}")))
}

fn decode_list(row: &mut Value, key: &str) -> RemoteResult<()> {
    let Some(raw) = row[key].as_str() else {
        if row[key].is_null() {
            row[key] = json!([]);
        }
        return Ok(());
    };
    row[key] = serde_json::from_str(if raw.is_empty() { "[]" } else { raw }).map_err(|e| {
        RemoteStorageError::InvalidOperation(format!("Invalid {key} on record #{}: {e}", row["id"]))
    })?;
    Ok(())
}

fn read_table(
    tx: &mut postgres::Transaction<'_>,
    schema: &str,
    table: &str,
) -> RemoteResult<Vec<Value>> {
    let qualified = format!("{schema}.{table}");
    let exists: bool = tx
        .query_one("SELECT to_regclass($1) IS NOT NULL", &[&qualified])?
        .get(0);
    if !exists && table == "memos" {
        return Ok(Vec::new());
    }
    if !exists {
        return Err(RemoteStorageError::InvalidOperation(format!(
            "Source table {qualified} was not found"
        )));
    }
    let sql = format!("SELECT row.data::text FROM (SELECT to_jsonb(t) AS data FROM {qualified} t ORDER BY id) row WHERE row.data ->> 'deleted_at' IS NULL");
    let rows = tx.query(&sql, &[])?;
    rows.into_iter()
        .map(|row| {
            serde_json::from_str(&row.get::<_, String>(0))
                .map_err(|e| RemoteStorageError::InvalidOperation(e.to_string()))
        })
        .collect()
}

pub(crate) fn merge_workspace(
    storage: &super::Storage,
    data: &Prepared,
) -> RemoteResult<ImportResult> {
    with_client(storage, |client| {
        let mut tx = client.transaction()?;
        tx.batch_execute(
            "SET LOCAL statement_timeout = '60s';
            LOCK TABLE superclipboard_v4.clipboard_entries, superclipboard_v4.memos
            IN SHARE ROW EXCLUSIVE MODE",
        )?;
        let mut result = ImportResult::default();
        let clip_statement = tx.prepare(
            "INSERT INTO superclipboard_v4.clipboard_entries
                (uuid, category, category_tags, content_type, content, preview, search_text, hash,
                 content_hash, pinned, created_at, original_content, updated_at, archived_at)
             SELECT $1, $2, $3, $4, $5, $6, $7,
                CASE WHEN EXISTS (SELECT 1 FROM superclipboard_v4.clipboard_entries WHERE hash = $8)
                    THEN $9 ELSE $8 END, $9, $10, $11, $12, $13, $14
             WHERE NOT EXISTS (
                SELECT 1 FROM superclipboard_v4.clipboard_entries WHERE content_hash = $9
             ) ON CONFLICT(hash) DO NOTHING",
        )?;
        for entry in &data.entries {
            let record = &entry.record;
            let changed = tx.execute(
                &clip_statement,
                &[
                    &Uuid::new_v4().to_string(),
                    &record.category.to_string(),
                    &entry.tags,
                    &record.content_type,
                    &record.content,
                    &record.preview,
                    &entry.search,
                    &record.hash,
                    &entry.current_hash,
                    &record.pinned,
                    &entry.created,
                    &record.original_content,
                    &record.updated_at,
                    &record.archived_at,
                ],
            )? as usize;
            result.clipboard += changed;
            result.duplicates += 1 - changed;
        }
        let memo_statement = tx.prepare(
            "INSERT INTO superclipboard_v4.memos
                (uuid, title, body, tags, auto_tags, search_text, pinned, sort_order, created_at, updated_at, archived_at)
             SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11
             WHERE NOT EXISTS (
                SELECT 1 FROM superclipboard_v4.memos WHERE created_at = $9 AND title = $2 AND body = $3 AND tags = $4
             )",
        )?;
        let mut memo_order: Vec<_> = data.memos.iter().collect();
        memo_order.sort_by_key(|memo| (memo.record.sort_order, memo.record.id));
        let mut order: i64 = tx
            .query_one(
                "SELECT COALESCE(MAX(sort_order), 0) FROM superclipboard_v4.memos",
                &[],
            )?
            .get(0);
        for memo in memo_order {
            let record = &memo.record;
            let changed = tx.execute(
                "INSERT INTO superclipboard_v4.workspace_imports (key) VALUES ($1) ON CONFLICT(key) DO NOTHING",
                &[&memo.key],
            )?;
            if changed == 0 {
                result.duplicates += 1;
                continue;
            }
            order += 1;
            let changed = tx.execute(
                &memo_statement,
                &[
                    &Uuid::new_v4().to_string(),
                    &record.title,
                    &record.body,
                    &record.tags,
                    &memo.auto_tags,
                    &memo.search,
                    &record.pinned,
                    &order,
                    &record.created_at,
                    &record.updated_at,
                    &record.archived_at,
                ],
            )? as usize;
            result.memos += changed;
            result.duplicates += 1 - changed;
        }
        tx.commit()?;
        Ok(result)
    })
}
