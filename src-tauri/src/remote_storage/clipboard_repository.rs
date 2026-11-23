use super::*;

pub fn insert_clipboard(
    storage: &Storage,
    entry: &ClipboardEntry,
) -> RemoteResult<ClipboardInsertResult> {
    with_client(storage, |client| {
        let mut transaction = client.transaction()?;
        let outcome = insert_clipboard_in_transaction(&mut transaction, entry)?;
        transaction.commit()?;
        Ok(outcome)
    })
}

fn insert_clipboard_in_transaction(
    transaction: &mut Transaction<'_>,
    entry: &ClipboardEntry,
) -> RemoteResult<ClipboardInsertResult> {
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
    let content_hash = Storage::hash_content(&entry.content);
    let inserted = transaction.execute(
        "INSERT INTO superclipboard_v4.clipboard_entries
         (uuid, category, category_tags, content_type, content, preview, search_text, hash, content_hash, pinned, created_at)
         SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11
         WHERE NOT EXISTS (
            SELECT 1 FROM superclipboard_v4.clipboard_entries
            WHERE hash = $8 OR content_hash = $9
         )
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
            &content_hash,
            &entry.pinned,
            &entry.created_at.to_rfc3339(),
        ],
    )?;
    if inserted > 0 {
        return Ok(ClipboardInsertResult::Inserted);
    }

    let promoted = transaction.execute(
        "UPDATE superclipboard_v4.clipboard_entries
         SET created_at = $1
         WHERE id = (
            SELECT id FROM superclipboard_v4.clipboard_entries
            WHERE archived_at IS NULL AND deleted_at IS NULL
              AND (hash = $2 OR content_hash = $3)
            ORDER BY created_at DESC
            LIMIT 1
         )",
        &[&entry.created_at.to_rfc3339(), &entry.hash, &content_hash],
    )?;
    if promoted > 0 {
        return Ok(ClipboardInsertResult::Promoted);
    }

    let archived_duplicate_id = transaction
        .query_opt(
            "SELECT id FROM superclipboard_v4.clipboard_entries
             WHERE archived_at IS NOT NULL AND deleted_at IS NULL
               AND (hash = $1 OR content_hash = $2)
             ORDER BY archived_at DESC
             LIMIT 1",
            &[&entry.hash, &content_hash],
        )?
        .map(|row| row.get(0));
    Ok(archived_duplicate_id
        .map(ClipboardInsertResult::ArchivedDuplicate)
        .unwrap_or(ClipboardInsertResult::Ignored))
}

fn delete_active_clipboards_in_transaction(
    transaction: &mut Transaction<'_>,
    ids: &[i64],
    archive: bool,
) -> RemoteResult<u64> {
    let affected = if archive {
        transaction.execute(
            "UPDATE superclipboard_v4.clipboard_entries
             SET archived_at = now()::text, version = version + 1
             WHERE id = ANY($1) AND archived_at IS NULL AND deleted_at IS NULL",
            &[&ids],
        )?
    } else {
        transaction.execute(
            "DELETE FROM superclipboard_v4.clipboard_entries
             WHERE id = ANY($1) AND archived_at IS NULL AND deleted_at IS NULL",
            &[&ids],
        )?
    };
    if affected != ids.len() as u64 {
        return Err(RemoteStorageError::InvalidOperation(
            "Some selected clipboard entries are no longer available.".to_string(),
        ));
    }
    Ok(affected)
}

/// Atomically store a merged text entry and optionally remove its source entries.
pub fn merge_clipboard_entry(
    storage: &Storage,
    entry: &ClipboardEntry,
    source_ids: &[i64],
    delete_sources: bool,
    archive_sources: bool,
) -> RemoteResult<(ClipboardInsertResult, u64)> {
    with_client(storage, |client| {
        let mut transaction = client.transaction()?;
        let outcome = insert_clipboard_in_transaction(&mut transaction, entry)?;
        let deleted = if delete_sources {
            delete_active_clipboards_in_transaction(&mut transaction, source_ids, archive_sources)?
        } else {
            0
        };
        transaction.commit()?;
        Ok((outcome, deleted))
    })
}

/// Atomically create a merged image memo and optionally remove its source entries.
pub fn merge_memo_entry(storage: &Storage, request: MergeMemoRequest<'_>) -> RemoteResult<u64> {
    let title = request.title.to_string();
    let body = request.body.to_string();
    let tags = memo_tags::manual_only(request.tags);
    let auto_tags = request.auto_tags.to_vec();
    let source_ids = request.source_ids.to_vec();
    let delete_sources = request.delete_sources;
    let archive_sources = request.archive_sources;
    let search_text = memo_search_text(&title, &body, &tags, &auto_tags);
    let auto_tags = serde_json::to_string(&auto_tags).unwrap_or_else(|_| "[]".to_string());
    let now = Utc::now().to_rfc3339();
    let uuid = Uuid::new_v4().to_string();
    with_client(storage, |client| {
        let mut transaction = client.transaction()?;
        transaction.execute(
            "INSERT INTO superclipboard_v4.memos
             (uuid, title, body, tags, auto_tags, search_text, sort_order, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6,
                (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM superclipboard_v4.memos), $7, $7)",
            &[&uuid, &title, &body, &tags, &auto_tags, &search_text, &now],
        )?;
        let deleted = if delete_sources {
            delete_active_clipboards_in_transaction(&mut transaction, &source_ids, archive_sources)?
        } else {
            0
        };
        transaction.commit()?;
        Ok(deleted)
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
         FROM superclipboard_v4.clipboard_entries WHERE archived_at IS NULL AND deleted_at IS NULL",
    );
    let mut values: Vec<Box<dyn ToSql + Sync>> = Vec::new();

    if filter.summary {
        sql = sql
            .replacen(
                "CASE WHEN category = 'image' THEN '' ELSE content END AS content",
                "'' AS content",
                1,
            )
            .replacen(
                "created_at, original_content,",
                "created_at, NULL::text AS original_content,",
                1,
            );
    }
    if filter.favorites {
        sql.push_str(" AND pinned");
    }
    if let Some(category) = &filter.category {
        append_category_filter(
            &mut sql,
            &mut values,
            category,
            filter.include_auxiliary_tags,
        );
    }
    if let Some(search) = &filter.search {
        append_token_search(&mut sql, &mut values, search, &["search_text"]);
    }

    sql.push_str(" ORDER BY pinned DESC, created_at DESC, id DESC");
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
             FROM superclipboard_v4.clipboard_entries WHERE id = $1 AND deleted_at IS NULL",
            &[&id],
        )?;
        Ok(row.as_ref().map(row_to_entry))
    })
}

pub fn get_clipboard_by_ids(
    storage: &Storage,
    requested_ids: &[i64],
) -> RemoteResult<Vec<ClipboardEntry>> {
    let ids = requested_ids.to_vec();
    with_client(storage, |client| {
        let rows = client.query(
            "SELECT id, category, category_tags, content_type, content, preview, hash, pinned, created_at, original_content, updated_at, archived_at, version
             FROM superclipboard_v4.clipboard_entries
             WHERE id = ANY($1) AND deleted_at IS NULL",
            &[&ids],
        )?;
        let entries = rows.iter().map(row_to_entry).collect::<Vec<_>>();
        Ok(ids
            .iter()
            .filter_map(|id| entries.iter().find(|entry| entry.id == *id).cloned())
            .collect())
    })
}

pub fn delete_clipboard(storage: &Storage, id: i64, archive: bool) -> RemoteResult<bool> {
    with_client(storage, |client| {
        let rows = if archive {
            client.execute(
                "UPDATE superclipboard_v4.clipboard_entries SET archived_at = now()::text, version = version + 1 WHERE id = $1 AND archived_at IS NULL",
                &[&id],
            )?
        } else {
            client.execute(
                "DELETE FROM superclipboard_v4.clipboard_entries WHERE id = $1",
                &[&id],
            )?
        };
        Ok(rows > 0)
    })
}

pub fn delete_clipboards(storage: &Storage, ids: &[i64], archive: bool) -> RemoteResult<u64> {
    if ids.is_empty() {
        return Ok(0);
    }
    let ids = ids.to_vec();
    with_client(storage, |client| {
        if archive {
            Ok(client.execute(
                "UPDATE superclipboard_v4.clipboard_entries
                 SET archived_at = now()::text, version = version + 1
                 WHERE id = ANY($1) AND archived_at IS NULL AND deleted_at IS NULL",
                &[&ids],
            )?)
        } else {
            Ok(client.execute(
                "DELETE FROM superclipboard_v4.clipboard_entries WHERE id = ANY($1)",
                &[&ids],
            )?)
        }
    })
}

pub fn toggle_clipboard_pin(storage: &Storage, id: i64) -> RemoteResult<bool> {
    with_client(storage, |client| {
        Ok(client
            .query_one(
                "UPDATE superclipboard_v4.clipboard_entries SET pinned = NOT pinned, version = version + 1 WHERE id = $1 RETURNING pinned",
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
            "SELECT content, original_content FROM superclipboard_v4.clipboard_entries WHERE id = $1",
            &[&id],
        )?;
        let Some(row) = current else {
            return Ok(UpdateResult::updated(false));
        };
        let current_content: String = row.get("content");
        let original_content: Option<String> = row.get("original_content");
        if current_content == content {
            return Ok(UpdateResult::updated(true));
        }
        let reverts_to_original = original_content.as_deref() == Some(content);
        let original = if reverts_to_original {
            None
        } else {
            Some(original_content.unwrap_or(current_content))
        };
        let preview = if content.chars().count() > 200 {
            content.chars().take(200).collect::<String>()
        } else {
            content.to_string()
        };
        let now = (!reverts_to_original).then(|| Utc::now().to_rfc3339());
        let categories = classify_text_tags(content);
        let category = categories.first().cloned().unwrap_or(Category::Text);
        let search_text = clipboard_search_text("text/plain", content, &preview, &categories);
        let category_tags = category_tags_json(&categories);
        let content_hash = Storage::hash_content(content);
        let rows = if let Some(expected_version) = expected_version {
            client.execute(
                "UPDATE superclipboard_v4.clipboard_entries
                 SET category = $1, category_tags = $2, content = $3, preview = $4, search_text = $5, content_hash = $6, original_content = $7, updated_at = $8, version = version + 1
                 WHERE id = $9 AND version = $10",
                &[&category.to_string(), &category_tags, &content, &preview, &search_text, &content_hash, &original, &now, &id, &expected_version],
            )?
        } else {
            client.execute(
                "UPDATE superclipboard_v4.clipboard_entries
                 SET category = $1, category_tags = $2, content = $3, preview = $4, search_text = $5, content_hash = $6, original_content = $7, updated_at = $8, version = version + 1
                 WHERE id = $9",
                &[&category.to_string(), &category_tags, &content, &preview, &search_text, &content_hash, &original, &now, &id],
            )?
        };
        if rows > 0 {
            return Ok(UpdateResult::updated(true));
        }
        let exists = client
            .query_opt(
                "SELECT 1 FROM superclipboard_v4.clipboard_entries WHERE id = $1",
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

pub fn stats(storage: &Storage, include_auxiliary_tags: bool) -> RemoteResult<RemoteStats> {
    with_client(storage, |client| {
        let row = client.query_one(
            "
        SELECT
            COUNT(*) FILTER (WHERE ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS total,
            COUNT(*) FILTER (WHERE (ce.category = 'text' OR ($1 AND ce.category_tags LIKE '%\"text\"%')) AND ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS text,
            COUNT(*) FILTER (WHERE (ce.category = 'link' OR ($1 AND ce.category_tags LIKE '%\"link\"%')) AND ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS link,
            COUNT(*) FILTER (WHERE (ce.category = 'image' OR ($1 AND ce.category_tags LIKE '%\"image\"%')) AND ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS image,
            COUNT(*) FILTER (WHERE (ce.category = 'code' OR ($1 AND ce.category_tags LIKE '%\"code\"%')) AND ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS code,
            COUNT(*) FILTER (WHERE (ce.category = 'email' OR ($1 AND ce.category_tags LIKE '%\"email\"%')) AND ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS email,
            COUNT(*) FILTER (WHERE (ce.category = 'file_path' OR ($1 AND ce.category_tags LIKE '%\"file_path\"%')) AND ce.archived_at IS NULL AND ce.deleted_at IS NULL)::bigint AS file_path,
            COUNT(*) FILTER (WHERE ce.archived_at IS NOT NULL AND ce.deleted_at IS NULL)::bigint AS archive,
            COALESCE(SUM(LENGTH(ce.content) + LENGTH(ce.preview) + LENGTH(COALESCE(ce.original_content, '')))
                FILTER (WHERE ce.archived_at IS NULL AND ce.deleted_at IS NULL), 0)::bigint AS clipboard_size,
            (SELECT COUNT(*) FROM superclipboard_v4.memos WHERE archived_at IS NULL AND deleted_at IS NULL)::bigint AS memo_count,
            (SELECT COUNT(*) FROM superclipboard_v4.memos WHERE archived_at IS NOT NULL AND deleted_at IS NULL)::bigint AS memo_archive,
            (SELECT COALESCE(SUM(LENGTH(title) + LENGTH(body) + LENGTH(tags)), 0)::bigint
             FROM superclipboard_v4.memos WHERE archived_at IS NULL AND deleted_at IS NULL) AS memo_size
        FROM superclipboard_v4.clipboard_entries ce
        ",
            &[&include_auxiliary_tags],
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

pub fn clear_clipboard_unpinned(
    storage: &Storage,
    archive: bool,
    category: Option<&str>,
    include_auxiliary_tags: bool,
) -> RemoteResult<u64> {
    with_client(storage, |client| {
        if let Some(category) = category {
            let tag_pattern = category_tag_pattern(category);
            return if archive {
                Ok(client.execute(
                    "UPDATE superclipboard_v4.clipboard_entries
                     SET archived_at = now()::text, version = version + 1
                     WHERE pinned = false AND archived_at IS NULL AND deleted_at IS NULL
                       AND (category = $1 OR ($2 AND category_tags LIKE $3))",
                    &[&category, &include_auxiliary_tags, &tag_pattern],
                )?)
            } else {
                Ok(client.execute(
                    "DELETE FROM superclipboard_v4.clipboard_entries
                     WHERE pinned = false AND archived_at IS NULL AND deleted_at IS NULL
                       AND (category = $1 OR ($2 AND category_tags LIKE $3))",
                    &[&category, &include_auxiliary_tags, &tag_pattern],
                )?)
            };
        }

        if archive {
            Ok(client.execute(
                "UPDATE superclipboard_v4.clipboard_entries SET archived_at = now()::text, version = version + 1
                 WHERE pinned = false AND archived_at IS NULL AND deleted_at IS NULL",
                &[],
            )?)
        } else {
            Ok(client.execute(
                "DELETE FROM superclipboard_v4.clipboard_entries
                 WHERE pinned = false AND archived_at IS NULL AND deleted_at IS NULL",
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
         FROM superclipboard_v4.clipboard_entries WHERE archived_at IS NOT NULL AND deleted_at IS NULL",
    );
    let mut values: Vec<Box<dyn ToSql + Sync>> = Vec::new();
    if filter.summary {
        sql = sql
            .replacen(
                "CASE WHEN category = 'image' THEN '' ELSE content END AS content",
                "'' AS content",
                1,
            )
            .replacen(
                "created_at, original_content,",
                "created_at, NULL::text AS original_content,",
                1,
            );
    }
    if let Some(category) = &filter.category {
        append_category_filter(
            &mut sql,
            &mut values,
            category,
            filter.include_auxiliary_tags,
        );
    }
    if let Some(search) = &filter.search {
        append_token_search(&mut sql, &mut values, search, &["search_text"]);
    }
    sql.push_str(" ORDER BY archived_at DESC, id DESC");
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
                "SELECT COUNT(*) FROM superclipboard_v4.clipboard_entries WHERE archived_at IS NOT NULL AND deleted_at IS NULL",
                &[],
            )?
            .get(0))
    })
}

pub fn unarchive_clipboard(storage: &Storage, id: i64) -> RemoteResult<bool> {
    with_client(storage, |client| {
        Ok(client.execute(
            "UPDATE superclipboard_v4.clipboard_entries SET archived_at = NULL, version = version + 1 WHERE id = $1",
            &[&id],
        )? > 0)
    })
}

pub fn permanent_delete_clipboard(storage: &Storage, id: i64) -> RemoteResult<bool> {
    with_client(storage, |client| {
        Ok(client.execute(
            "DELETE FROM superclipboard_v4.clipboard_entries WHERE id = $1 AND archived_at IS NOT NULL",
            &[&id],
        )? > 0)
    })
}

pub fn purge_old_clipboard_archives(storage: &Storage, days: i64) -> RemoteResult<u64> {
    with_client(storage, |client| {
        Ok(client.execute(
            "DELETE FROM superclipboard_v4.clipboard_entries
             WHERE archived_at IS NOT NULL AND archived_at::timestamptz < now() - ($1::int * interval '1 day')",
            &[&days],
        )?)
    })
}

pub fn empty_clipboard_archive(storage: &Storage) -> RemoteResult<u64> {
    with_client(storage, |client| {
        Ok(client.execute(
            "DELETE FROM superclipboard_v4.clipboard_entries WHERE archived_at IS NOT NULL",
            &[],
        )?)
    })
}
