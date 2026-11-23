use super::*;

pub fn query_memos(storage: &Storage, filter: &MemoFilter) -> RemoteResult<Vec<Memo>> {
    let mut sql = String::from(
        "SELECT id, title, body, tags, auto_tags, pinned, sort_order, created_at, updated_at, archived_at, content_version AS version
         FROM superclipboard_v4.memos WHERE archived_at IS NULL AND deleted_at IS NULL",
    );
    let mut values: Vec<Box<dyn ToSql + Sync>> = Vec::new();
    if filter.summary {
        sql = sql.replacen("title, body,", "title, '' AS body,", 1);
    }
    if filter.favorites {
        sql.push_str(" AND pinned");
    }
    if let Some(search) = &filter.search {
        append_token_search(&mut sql, &mut values, search, &["search_text"]);
    }
    sql.push_str(" ORDER BY pinned DESC, sort_order DESC, id DESC");
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

pub fn get_memo(storage: &Storage, id: i64) -> RemoteResult<Option<Memo>> {
    with_client(storage, |client| {
        Ok(client.query_opt(
            "SELECT id, title, body, tags, auto_tags, pinned, sort_order, created_at, updated_at,
                    archived_at, content_version AS version
             FROM superclipboard_v4.memos WHERE id = $1 AND deleted_at IS NULL",
            &[&id],
        )?.as_ref().map(row_to_memo))
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
    let tags = memo_tags::manual_only(tags);
    let search_text = memo_search_text(title, body, &tags, auto_tags);
    let auto_tags = serde_json::to_string(auto_tags).unwrap_or_else(|_| "[]".to_string());
    with_client(storage, |client| {
        let row = client.query_one(
            "INSERT INTO superclipboard_v4.memos (uuid, title, body, tags, auto_tags, search_text, sort_order, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM superclipboard_v4.memos), $7, $7)
             RETURNING id, title, body, tags, auto_tags, pinned, sort_order, created_at, updated_at, archived_at, content_version AS version",
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
    let tags = memo_tags::manual_only(tags);
    let search_text = memo_search_text(title, body, &tags, auto_tags);
    let auto_tags = serde_json::to_string(auto_tags).unwrap_or_else(|_| "[]".to_string());
    with_client(storage, |client| {
        let rows = if let Some(expected_version) = expected_version {
            client.execute(
                "UPDATE superclipboard_v4.memos SET title = $1, body = $2, tags = $3, auto_tags = $4, search_text = $5, updated_at = $6, version = version + 1, content_version = content_version + 1 WHERE id = $7 AND content_version = $8 AND archived_at IS NULL AND deleted_at IS NULL",
                &[&title, &body, &tags, &auto_tags, &search_text, &now, &id, &expected_version],
            )?
        } else {
            client.execute(
                "UPDATE superclipboard_v4.memos SET title = $1, body = $2, tags = $3, auto_tags = $4, search_text = $5, updated_at = $6, version = version + 1, content_version = content_version + 1 WHERE id = $7 AND archived_at IS NULL AND deleted_at IS NULL",
                &[&title, &body, &tags, &auto_tags, &search_text, &now, &id],
            )?
        };
        if rows > 0 {
            return Ok(UpdateResult::updated(true));
        }
        let exists = client
            .query_opt(
                "SELECT 1 FROM superclipboard_v4.memos WHERE id = $1",
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

pub fn delete_memo(storage: &Storage, id: i64, archive: bool) -> RemoteResult<bool> {
    with_client(storage, |client| {
        let rows = if archive {
            client.execute(
                "UPDATE superclipboard_v4.memos SET archived_at = now()::text, version = version + 1 WHERE id = $1 AND archived_at IS NULL",
                &[&id],
            )?
        } else {
            client.execute("DELETE FROM superclipboard_v4.memos WHERE id = $1", &[&id])?
        };
        Ok(rows > 0)
    })
}

pub fn toggle_memo_pin(storage: &Storage, id: i64) -> RemoteResult<bool> {
    with_client(storage, |client| {
        Ok(client.query_one(
            "UPDATE superclipboard_v4.memos SET pinned = NOT pinned, version = version + 1 WHERE id = $1 RETURNING pinned",
            &[&id],
        )?
        .get("pinned"))
    })
}

pub fn memo_count(storage: &Storage) -> RemoteResult<i64> {
    with_client(storage, |client| {
        Ok(client
            .query_one(
                "SELECT COUNT(*) FROM superclipboard_v4.memos WHERE archived_at IS NULL AND deleted_at IS NULL",
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
                "UPDATE superclipboard_v4.memos
                 SET sort_order = $1, sort_version = sort_version + 1, version = version + 1
                 WHERE id = $2 AND pinned = false AND archived_at IS NULL AND deleted_at IS NULL
                   AND sort_order IS DISTINCT FROM $1",
                &[sort_order, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    })
}

pub fn query_archived_memos(storage: &Storage, filter: &MemoFilter) -> RemoteResult<Vec<Memo>> {
    let mut sql = String::from(
        "SELECT id, title, body, tags, auto_tags, pinned, sort_order, created_at, updated_at, archived_at, content_version AS version
         FROM superclipboard_v4.memos WHERE archived_at IS NOT NULL AND deleted_at IS NULL",
    );
    let mut values: Vec<Box<dyn ToSql + Sync>> = Vec::new();
    if filter.summary {
        sql = sql.replacen("title, body,", "title, '' AS body,", 1);
    }
    if let Some(search) = &filter.search {
        append_token_search(&mut sql, &mut values, search, &["search_text"]);
    }
    sql.push_str(" ORDER BY archived_at DESC, id DESC");
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
                "SELECT COUNT(*) FROM superclipboard_v4.memos WHERE archived_at IS NOT NULL AND deleted_at IS NULL",
                &[],
            )?
            .get(0))
    })
}

pub fn unarchive_memo(storage: &Storage, id: i64) -> RemoteResult<bool> {
    with_client(storage, |client| {
        Ok(client.execute(
            "UPDATE superclipboard_v4.memos SET archived_at = NULL, version = version + 1 WHERE id = $1",
            &[&id],
        )? > 0)
    })
}

pub fn permanent_delete_memo(storage: &Storage, id: i64) -> RemoteResult<bool> {
    with_client(storage, |client| {
        Ok(client.execute(
            "DELETE FROM superclipboard_v4.memos WHERE id = $1 AND archived_at IS NOT NULL",
            &[&id],
        )? > 0)
    })
}

pub fn purge_old_memo_archives(storage: &Storage, days: i64) -> RemoteResult<u64> {
    with_client(storage, |client| {
        Ok(client.execute(
            "DELETE FROM superclipboard_v4.memos
             WHERE archived_at IS NOT NULL AND archived_at::timestamptz < now() - ($1::int * interval '1 day')",
            &[&days],
        )?)
    })
}

pub fn empty_memo_archive(storage: &Storage) -> RemoteResult<u64> {
    with_client(storage, |client| {
        Ok(client.execute(
            "DELETE FROM superclipboard_v4.memos WHERE archived_at IS NOT NULL",
            &[],
        )?)
    })
}

pub fn archive_memo(storage: &Storage, id: i64) -> RemoteResult<bool> {
    delete_memo(storage, id, true)
}
