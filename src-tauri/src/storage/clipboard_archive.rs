use super::*;

impl Storage {
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
    #[cfg(test)]
    pub fn update_entry(&self, id: i64, new_content: &str) -> Result<bool, StorageError> {
        self.update_entry_checked(id, new_content, None)
            .map(|result| result.updated)
    }

    pub fn update_entry_checked(
        &self,
        id: i64,
        new_content: &str,
        expected: Option<i64>,
    ) -> Result<UpdateResult, StorageError> {
        let conn = self.conn.lock().unwrap();
        let current: Option<(String, Option<String>, i64)> = conn
            .query_row(
                "SELECT content, original_content, content_version FROM clipboard_entries
                 WHERE id = ?1 AND archived_at IS NULL AND content_type NOT LIKE 'image/%'",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let (current_content, existing_original, version) = match current {
            Some(current) => current,
            None => return Ok(UpdateResult::updated(false)),
        };
        if expected.is_some_and(|expected| expected != version) {
            return Ok(UpdateResult::conflict());
        }
        if current_content == new_content {
            return Ok(UpdateResult::updated(true));
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
            "UPDATE clipboard_entries SET category = ?1, category_tags = ?2, content = ?3, preview = ?4, search_text = ?5, content_hash = ?6, original_content = ?7, updated_at = ?8, content_version = content_version + 1 WHERE id = ?9",
            params![category.to_string(), category_tags, new_content, preview, search_text, Self::hash_content(new_content), original, now, id],
        )?;
        Ok(UpdateResult::updated(rows > 0))
    }

    /// Get total count of entries, optionally filtered by category
    #[cfg(test)]
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

    /// Fetch all UI statistics with one aggregate scan of clipboard entries.
    pub fn stats(&self, include_auxiliary_tags: bool) -> Result<StorageStats, StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT
                COUNT(*) FILTER (WHERE ce.archived_at IS NULL) AS total,
                COUNT(*) FILTER (WHERE ce.archived_at IS NULL AND (ce.category = 'text' OR (?1 AND ce.category_tags LIKE '%\"text\"%'))) AS text,
                COUNT(*) FILTER (WHERE ce.archived_at IS NULL AND (ce.category = 'link' OR (?1 AND ce.category_tags LIKE '%\"link\"%'))) AS link,
                COUNT(*) FILTER (WHERE ce.archived_at IS NULL AND (ce.category = 'image' OR (?1 AND ce.category_tags LIKE '%\"image\"%'))) AS image,
                COUNT(*) FILTER (WHERE ce.archived_at IS NULL AND (ce.category = 'code' OR (?1 AND ce.category_tags LIKE '%\"code\"%'))) AS code,
                COUNT(*) FILTER (WHERE ce.archived_at IS NULL AND (ce.category = 'email' OR (?1 AND ce.category_tags LIKE '%\"email\"%'))) AS email,
                COUNT(*) FILTER (WHERE ce.archived_at IS NULL AND (ce.category = 'file_path' OR (?1 AND ce.category_tags LIKE '%\"file_path\"%'))) AS file_path,
                COUNT(*) FILTER (WHERE ce.archived_at IS NOT NULL) AS archive,
                COALESCE(SUM(LENGTH(ce.content) + LENGTH(ce.preview) + LENGTH(COALESCE(ce.original_content, ''))) FILTER (WHERE ce.archived_at IS NULL), 0) AS clipboard_size,
                (SELECT COUNT(*) FROM memos WHERE archived_at IS NULL) AS memo_count,
                (SELECT COUNT(*) FROM memos WHERE archived_at IS NOT NULL) AS memo_archive,
                (SELECT COALESCE(SUM(LENGTH(title) + LENGTH(body) + LENGTH(tags)), 0) FROM memos WHERE archived_at IS NULL) AS memo_size
             FROM clipboard_entries ce",
            params![include_auxiliary_tags],
            |row| {
                Ok(StorageStats {
                    total: row.get("total")?,
                    text: row.get("text")?,
                    link: row.get("link")?,
                    image: row.get("image")?,
                    code: row.get("code")?,
                    email: row.get("email")?,
                    file_path: row.get("file_path")?,
                    archive: row.get("archive")?,
                    clipboard_size: row.get("clipboard_size")?,
                    memo_count: row.get("memo_count")?,
                    memo_archive: row.get("memo_archive")?,
                    memo_size: row.get("memo_size")?,
                })
            },
        )
        .map_err(StorageError::from)
    }

    /// Get database size in bytes (page_count * page_size)
    pub fn db_size(&self) -> Result<i64, StorageError> {
        let conn = self.conn.lock().unwrap();
        let page_count: i64 = conn.query_row("PRAGMA page_count", [], |row| row.get(0))?;
        let page_size: i64 = conn.query_row("PRAGMA page_size", [], |row| row.get(0))?;
        Ok(page_count * page_size)
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
        let mut sql = String::from("SELECT id, category, content_type, content, preview, hash, pinned, created_at, original_content, updated_at, archived_at, category_tags, content_version FROM clipboard_entries WHERE archived_at IS NOT NULL");
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

        sql.push_str(" ORDER BY archived_at DESC, id DESC");

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
        let rows = conn.execute(
            "DELETE FROM clipboard_entries WHERE id = ?1 AND archived_at IS NOT NULL",
            params![id],
        )?;
        Ok(rows > 0)
    }

    // ─── Memo CRUD ──────────────────────────────────────────────────
}
