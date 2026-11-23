use super::*;

impl Storage {
    /// Query memos with optional search filter
    pub fn get_memos(&self, filter: &MemoFilter) -> Result<Vec<Memo>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut sql = String::from("SELECT id, title, body, tags, auto_tags, pinned, sort_order, created_at, updated_at, archived_at, content_version FROM memos WHERE archived_at IS NULL");
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if filter.summary {
            sql = sql.replacen("title, body,", "title, '' AS body,", 1);
        }
        if filter.favorites {
            sql.push_str(" AND pinned = 1");
        }
        if let Some(ref search) = filter.search {
            append_token_search(&mut sql, &mut param_values, search, &["search_text"]);
        }

        sql.push_str(" ORDER BY pinned DESC, sort_order DESC, id DESC");

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

    pub fn get_memo(&self, id: i64) -> Result<Option<Memo>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let memo = conn.query_row(
            "SELECT id, title, body, tags, auto_tags, pinned, sort_order, created_at, updated_at, archived_at, content_version
             FROM memos WHERE id = ?1",
            [id], map_row_to_memo,
        ).optional()?;
        Ok(memo)
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
            "SELECT id, title, body, tags, auto_tags, pinned, sort_order, created_at, updated_at, archived_at, content_version FROM memos WHERE id = ?1",
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
        expected: Option<i64>,
    ) -> Result<UpdateResult, StorageError> {
        let conn = self.conn.lock().unwrap();
        let tags = memo_tags::manual_only(tags);
        let search_text = memo_search_text(title, body, &tags, auto_tags);
        let auto_tags = serde_json::to_string(auto_tags)?;
        let rows = conn.execute(
            "UPDATE memos SET title = ?1, body = ?2, tags = ?3, auto_tags = ?4, search_text = ?5,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), content_version = content_version + 1
             WHERE id = ?6 AND archived_at IS NULL AND (?7 IS NULL OR content_version = ?7)",
            params![title, body, tags, auto_tags, search_text, id, expected],
        )?;
        if rows == 0
            && expected.is_some()
            && conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM memos WHERE id = ?1 AND archived_at IS NULL)",
                [id],
                |row| row.get(0),
            )?
        {
            return Ok(UpdateResult::conflict());
        }
        Ok(UpdateResult::updated(rows > 0))
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
                "UPDATE memos SET sort_order = ?1 WHERE id = ?2 AND sort_order IS NOT ?3",
                params![sort_order, id, sort_order],
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
        let mut sql = String::from("SELECT id, title, body, tags, auto_tags, pinned, sort_order, created_at, updated_at, archived_at, content_version FROM memos WHERE archived_at IS NOT NULL");
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if filter.summary {
            sql = sql.replacen("title, body,", "title, '' AS body,", 1);
        }
        if let Some(ref search) = filter.search {
            append_token_search(&mut sql, &mut param_values, search, &["search_text"]);
        }

        sql.push_str(" ORDER BY archived_at DESC, id DESC");

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
        let rows = conn.execute(
            "DELETE FROM memos WHERE id = ?1 AND archived_at IS NOT NULL",
            params![id],
        )?;
        Ok(rows > 0)
    }
}
