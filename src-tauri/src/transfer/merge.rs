use super::prepare::Prepared;
use crate::storage::Storage;
use rusqlite::{params, TransactionBehavior};
use serde::Serialize;

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub clipboard: usize,
    pub memos: usize,
    pub duplicates: usize,
}

pub fn local(storage: &Storage, data: &Prepared) -> Result<ImportResult, String> {
    let mut conn = storage.conn.lock().map_err(|e| e.to_string())?;
    (|| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut result = ImportResult::default();
        for entry in &data.entries {
            let record = &entry.record;
            let changed = tx.execute(
                "INSERT INTO clipboard_entries
                    (category, category_tags, content_type, content, preview, search_text, hash,
                     content_hash, pinned, created_at, original_content, updated_at, archived_at)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6,
                    CASE WHEN EXISTS (SELECT 1 FROM clipboard_entries WHERE hash = ?7) THEN ?8 ELSE ?7 END,
                    ?8, ?9, ?10, ?11, ?12, ?13
                 WHERE NOT EXISTS (SELECT 1 FROM clipboard_entries WHERE content_hash = ?8)
                 ON CONFLICT(hash) DO NOTHING",
                params![
                    record.category.to_string(), entry.tags, record.content_type, record.content, record.preview,
                    entry.search, record.hash, entry.current_hash, record.pinned, entry.created,
                    record.original_content, record.updated_at, record.archived_at
                ],
            )?;
            result.clipboard += changed;
            result.duplicates += 1 - changed;
        }
        let mut memo_order: Vec<_> = data.memos.iter().collect();
        memo_order.sort_by_key(|memo| (memo.record.sort_order, memo.record.id));
        let mut order: i64 = tx.query_row("SELECT COALESCE(MAX(sort_order), 0) FROM memos", [], |row| row.get(0))?;
        for memo in memo_order {
            let record = &memo.record;
            let changed = tx.execute(
                "INSERT INTO workspace_imports (key) VALUES (?1) ON CONFLICT(key) DO NOTHING",
                [&memo.key],
            )?;
            if changed == 0 {
                result.duplicates += 1;
                continue;
            }
            order += 1;
            let changed = tx.execute(
                "INSERT INTO memos
                    (title, body, tags, auto_tags, search_text, pinned, sort_order, created_at, updated_at, archived_at)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10
                 WHERE NOT EXISTS (
                     SELECT 1 FROM memos WHERE created_at = ?8 AND title = ?1 AND body = ?2 AND tags = ?3
                 )",
                params![
                    record.title, record.body, record.tags, memo.auto_tags, memo.search, record.pinned,
                    order, record.created_at, record.updated_at, record.archived_at
                ],
            )?;
            result.memos += changed;
            result.duplicates += 1 - changed;
        }
        tx.commit()?;
        Ok::<_, rusqlite::Error>(result)
    })().map_err(|e| format!("Import rolled back: {e}"))
}
