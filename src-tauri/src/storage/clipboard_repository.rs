use super::*;

impl Storage {
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

    /// Insert a new clipboard entry, returns Ok(true) if inserted, Ok(false) if duplicate.
    #[cfg(test)]
    pub fn insert(&self, entry: &ClipboardEntry) -> Result<bool, StorageError> {
        Ok(self.insert_or_promote(entry)?.inserted())
    }

    /// Capture clipboard content and promote an active duplicate to the newest position.
    pub fn insert_or_promote(
        &self,
        entry: &ClipboardEntry,
    ) -> Result<ClipboardInsertResult, StorageError> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        let outcome = Self::insert_or_promote_in_transaction(&tx, entry)?;
        tx.commit()?;
        Ok(outcome)
    }

    fn insert_or_promote_in_transaction(
        tx: &rusqlite::Transaction<'_>,
        entry: &ClipboardEntry,
    ) -> Result<ClipboardInsertResult, StorageError> {
        let category_tags = normalize_category_tags(entry.category_tags.clone());
        let category = category_tags.first().cloned().unwrap_or(Category::Text);
        let search_text = clipboard_search_text(
            &entry.content_type,
            &entry.content,
            &entry.preview,
            &category_tags,
        );
        let category_tags = category_tags_json(&category_tags)?;
        let content_hash = Self::hash_content(&entry.content);
        let result = tx.execute(
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
                content_hash,
                entry.pinned as i32,
                entry.created_at.to_rfc3339(),
            ],
        )?;
        if result > 0 {
            return Ok(ClipboardInsertResult::Inserted);
        }

        let promoted = tx.execute(
            "UPDATE clipboard_entries
             SET created_at = ?1
             WHERE id = (
                SELECT id FROM clipboard_entries
                WHERE archived_at IS NULL AND (hash = ?2 OR content_hash = ?3)
                ORDER BY created_at DESC
                LIMIT 1
             )",
            params![entry.created_at.to_rfc3339(), entry.hash, content_hash],
        )?;

        if promoted > 0 {
            return Ok(ClipboardInsertResult::Promoted);
        }

        let archived_duplicate_id = tx
            .query_row(
                "SELECT id FROM clipboard_entries
                 WHERE archived_at IS NOT NULL AND (hash = ?1 OR content_hash = ?2)
                 ORDER BY archived_at DESC
                 LIMIT 1",
                params![entry.hash, content_hash],
                |row| row.get(0),
            )
            .optional()?;
        Ok(archived_duplicate_id
            .map(ClipboardInsertResult::ArchivedDuplicate)
            .unwrap_or(ClipboardInsertResult::Ignored))
    }

    /// Atomically store a merged text entry and optionally remove its source entries.
    pub fn merge_clipboard_entry(
        &self,
        entry: &ClipboardEntry,
        source_ids: &[i64],
        delete_sources: bool,
        archive_sources: bool,
    ) -> Result<(ClipboardInsertResult, u64), StorageError> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        let outcome = Self::insert_or_promote_in_transaction(&tx, entry)?;
        let deleted = if delete_sources {
            Self::delete_active_entries_in_transaction(&tx, source_ids, archive_sources)?
        } else {
            0
        };
        tx.commit()?;
        Ok((outcome, deleted))
    }

    /// Atomically create a merged image memo and optionally remove its source entries.
    pub fn merge_memo_entry(&self, request: MergeMemoRequest<'_>) -> Result<u64, StorageError> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        let tags = memo_tags::manual_only(request.tags);
        let search_text = memo_search_text(request.title, request.body, &tags, request.auto_tags);
        let auto_tags = serde_json::to_string(request.auto_tags)?;
        tx.execute(
            "INSERT INTO memos (title, body, tags, auto_tags, search_text, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM memos), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![request.title, request.body, tags, auto_tags, search_text],
        )?;
        let deleted = if request.delete_sources {
            Self::delete_active_entries_in_transaction(
                &tx,
                request.source_ids,
                request.archive_sources,
            )?
        } else {
            0
        };
        tx.commit()?;
        Ok(deleted)
    }

    fn delete_active_entries_in_transaction(
        tx: &rusqlite::Transaction<'_>,
        ids: &[i64],
        archive: bool,
    ) -> Result<u64, StorageError> {
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
                tx.execute(
                    "DELETE FROM clipboard_entries WHERE id = ?1 AND archived_at IS NULL",
                    params![id],
                )?
            };
            affected += rows as u64;
        }
        if affected != ids.len() as u64 {
            return Err(StorageError::InvalidOperation(
                "Some selected clipboard entries are no longer available.".to_string(),
            ));
        }
        Ok(affected)
    }

    /// Query entries with optional filters
    pub fn query(&self, filter: &QueryFilter) -> Result<Vec<ClipboardEntry>, StorageError> {
        let conn = self.conn.lock().unwrap();

        let mut sql = String::from("SELECT id, category, content_type, content, preview, hash, pinned, created_at, original_content, updated_at, archived_at, category_tags, content_version FROM clipboard_entries WHERE archived_at IS NULL");
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if filter.summary {
            sql = sql
                .replacen("content_type, content,", "content_type, '' AS content,", 1)
                .replacen(
                    "created_at, original_content,",
                    "created_at, NULL AS original_content,",
                    1,
                );
        }
        if filter.favorites {
            sql.push_str(" AND pinned = 1");
        }
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

        sql.push_str(" ORDER BY pinned DESC, created_at DESC, id DESC");

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
            "SELECT id, category, content_type, content, preview, hash, pinned, created_at, original_content, updated_at, archived_at, category_tags, content_version
             FROM clipboard_entries WHERE id = ?1",
        )?;

        let entry = stmt.query_row(params![id], map_row_to_entry).ok();

        Ok(entry)
    }

    /// Fetch selected clipboard entries while preserving the caller's display order.
    pub fn get_entries_by_ids(&self, ids: &[i64]) -> Result<Vec<ClipboardEntry>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, category, content_type, content, preview, hash, pinned, created_at, original_content, updated_at, archived_at, category_tags, content_version
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
}
