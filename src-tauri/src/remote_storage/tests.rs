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
    reclassify_clipboard_entries(&storage).expect("record active classification rules");
    assert_eq!(
        classification_rules_applied_version(&storage).expect("read classification rules"),
        Some(CLASSIFICATION_RULES_VERSION)
    );
    with_client(&storage, |client| {
        let event_table: Option<String> = client
            .query_one(
                "SELECT to_regclass('superclipboard_v4.sync_events')::text",
                &[],
            )?
            .get(0);
        assert!(event_table.is_none());
        let mut transaction = client.transaction()?;
        transaction.batch_execute(remote_migrations::SEARCH_BACKFILL_SQL)?;
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
        "smoke,EMAIL,邮箱",
        &["image".to_string()],
    )
    .expect("create remote memo");
    assert!(memo.id > 0);
    assert_eq!(memo.tags, "smoke");

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

    reorder_memos(&storage, &[(memo.id, memo.sort_order + 1)]).expect("reorder remote memo");
    let reordered = query_memos(
        &storage,
        &MemoFilter {
            search: Some(memo_token.clone()),
            ..Default::default()
        },
    )
    .expect("query reordered remote memo");
    let reordered_memo = reordered
        .iter()
        .find(|item| item.id == memo.id)
        .expect("find reordered remote memo");
    assert_eq!(reordered_memo.version, memo.version);

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
        Some(reordered_memo.version),
    )
    .expect("update remote memo");
    assert!(updated.updated);

    assert!(delete_memo(&storage, memo.id, false).expect("delete remote memo"));
    with_client(&storage, |client| {
        let count: i64 = client
            .query_one(
                "SELECT COUNT(*) FROM superclipboard_v4.memos WHERE id = $1",
                &[&memo.id],
            )?
            .get(0);
        assert_eq!(count, 0);
        Ok(())
    })
    .expect("verify physical memo deletion");

    let text_token = format!("RemoteText{}", Uuid::new_v4().simple());
    let entry = ClipboardEntry {
        id: 0,
        category: Category::Text,
        category_tags: vec![Category::Text],
        content_type: "text/plain".to_string(),
        content: format!("Remote storage smoke {text_token}"),
        preview: text_token.clone(),
        hash: Uuid::new_v4().to_string(),
        pinned: false,
        created_at: Utc::now(),
        original_content: None,
        updated_at: None,
        archived_at: None,
        version: 1,
    };
    assert!(insert_clipboard(&storage, &entry)
        .expect("insert remote clipboard")
        .inserted());

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
    let current_stats = stats(&storage, false).expect("query remote stats");
    assert!(current_stats.total >= 1);
    assert!(delete_clipboard(&storage, inserted.id, false).expect("delete remote clipboard"));
    with_client(&storage, |client| {
        let count: i64 = client
            .query_one(
                "SELECT COUNT(*) FROM superclipboard_v4.clipboard_entries WHERE id = $1",
                &[&inserted.id],
            )?
            .get(0);
        assert_eq!(count, 0);
        Ok(())
    })
    .expect("verify physical clipboard deletion");

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
    assert!(insert_clipboard(&storage, &image_entry)
        .expect("insert remote image")
        .inserted());
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
