use super::*;
use uuid::Uuid;

fn temp_db_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("superclipboard-storage-{}.db", Uuid::new_v4()))
}

#[test]
fn settings_batch_round_trips_atomically() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    let values = HashMap::from([
        ("theme_mode".to_string(), "dark".to_string()),
        ("memo_enabled".to_string(), "true".to_string()),
    ]);

    storage.set_settings(&values).unwrap();
    storage.set_setting("language", "zh-CN").unwrap();
    storage
        .set_settings(&HashMap::from([
            ("storage_mode".to_string(), "remote".to_string()),
            ("remote_db_ready".to_string(), "true".to_string()),
        ]))
        .unwrap();
    let loaded = storage
        .get_settings(&[
            "theme_mode".to_string(),
            "memo_enabled".to_string(),
            "missing".to_string(),
        ])
        .unwrap();

    assert_eq!(loaded.get("theme_mode").map(String::as_str), Some("dark"));
    assert_eq!(loaded.get("memo_enabled").map(String::as_str), Some("true"));
    assert!(!loaded.contains_key("missing"));
    assert_eq!(
        storage.get_setting("language").unwrap().as_deref(),
        Some("zh-CN"),
    );

    drop(storage);
    let _ = std::fs::remove_file(db_path);
}

#[test]
fn auxiliary_categories_only_participate_when_enabled() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    let content = "https://example.com contact@example.com";
    let entry = ClipboardEntry {
        id: 0,
        category: Category::Link,
        category_tags: vec![Category::Link, Category::Email],
        content_type: "text/plain".to_string(),
        content: content.to_string(),
        preview: content.to_string(),
        hash: Storage::hash_content(content),
        pinned: false,
        created_at: Utc::now(),
        original_content: None,
        updated_at: None,
        archived_at: None,
        version: 1,
    };
    assert!(storage.insert(&entry).unwrap());

    let primary_only = QueryFilter {
        category: Some("email".to_string()),
        ..Default::default()
    };
    let include_auxiliary = QueryFilter {
        category: Some("email".to_string()),
        include_auxiliary_tags: true,
        ..Default::default()
    };

    assert!(storage.query(&primary_only).unwrap().is_empty());
    assert_eq!(storage.query(&include_auxiliary).unwrap().len(), 1);
    assert_eq!(storage.count(Some("email"), false).unwrap(), 0);
    assert_eq!(storage.count(Some("email"), true).unwrap(), 1);
    assert_eq!(storage.count(Some("link"), false).unwrap(), 1);

    drop(storage);
    let _ = std::fs::remove_file(db_path);
}

#[test]
fn stats_aggregates_categories_sizes_and_archives() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    let content = "https://example.com contact@example.com";
    let entry = ClipboardEntry {
        id: 0,
        category: Category::Link,
        category_tags: vec![Category::Link, Category::Email],
        content_type: "text/plain".to_string(),
        content: content.to_string(),
        preview: content.to_string(),
        hash: Storage::hash_content(content),
        pinned: false,
        created_at: Utc::now(),
        original_content: None,
        updated_at: None,
        archived_at: None,
        version: 1,
    };
    assert!(storage.insert(&entry).unwrap());
    storage.create_memo("Title", "Body", "tag", &[]).unwrap();

    let primary = storage.stats(false).unwrap();
    assert_eq!(primary.total, 1);
    assert_eq!(primary.link, 1);
    assert_eq!(primary.email, 0);
    assert_eq!(primary.memo_count, 1);
    assert!(primary.clipboard_size > 0);
    assert!(primary.memo_size > 0);

    let auxiliary = storage.stats(true).unwrap();
    assert_eq!(auxiliary.email, 1);

    assert!(storage.archive_entry(1).unwrap());
    let archived = storage.stats(true).unwrap();
    assert_eq!(archived.total, 0);
    assert_eq!(archived.archive, 1);

    drop(storage);
    let _ = std::fs::remove_file(db_path);
}

#[test]
fn clear_unpinned_category_respects_auxiliary_tags_and_pins() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    let insert = |content: &str, category: Category, category_tags: Vec<Category>, pinned| {
        storage
            .insert(&ClipboardEntry {
                id: 0,
                category,
                category_tags,
                content_type: "text/plain".to_string(),
                content: content.to_string(),
                preview: content.to_string(),
                hash: Storage::hash_content(content),
                pinned,
                created_at: Utc::now(),
                original_content: None,
                updated_at: None,
                archived_at: None,
                version: 1,
            })
            .unwrap()
    };

    assert!(insert(
        "https://example.com contact@example.com",
        Category::Link,
        vec![Category::Link, Category::Email],
        false,
    ));
    assert!(insert(
        "primary@example.com",
        Category::Email,
        vec![Category::Email],
        false,
    ));
    assert!(insert(
        "pinned@example.com",
        Category::Email,
        vec![Category::Email],
        true,
    ));
    assert!(insert(
        "plain text",
        Category::Text,
        vec![Category::Text],
        false,
    ));

    assert_eq!(
        storage.clear_unpinned(false, Some("email"), false).unwrap(),
        1
    );
    assert_eq!(storage.count(Some("email"), false).unwrap(), 1);
    assert_eq!(storage.count(Some("link"), false).unwrap(), 1);

    assert_eq!(
        storage.clear_unpinned(false, Some("email"), true).unwrap(),
        1
    );
    assert_eq!(storage.count(Some("email"), true).unwrap(), 1);
    assert_eq!(storage.count(Some("link"), false).unwrap(), 0);
    assert_eq!(storage.count(Some("text"), false).unwrap(), 1);

    drop(storage);
    let _ = std::fs::remove_file(db_path);
}

#[test]
fn clear_unpinned_category_archives_only_matching_entries() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    for (content, category, category_tags) in [
        (
            "https://example.com contact@example.com",
            Category::Link,
            vec![Category::Link, Category::Email],
        ),
        ("plain text", Category::Text, vec![Category::Text]),
    ] {
        assert!(storage
            .insert(&ClipboardEntry {
                id: 0,
                category,
                category_tags,
                content_type: "text/plain".to_string(),
                content: content.to_string(),
                preview: content.to_string(),
                hash: Storage::hash_content(content),
                pinned: false,
                created_at: Utc::now(),
                original_content: None,
                updated_at: None,
                archived_at: None,
                version: 1,
            })
            .unwrap());
    }

    assert_eq!(
        storage.clear_unpinned(true, Some("email"), true).unwrap(),
        1
    );
    assert_eq!(storage.count(Some("link"), false).unwrap(), 0);
    assert_eq!(storage.count(Some("text"), false).unwrap(), 1);
    assert_eq!(storage.archive_count().unwrap(), 1);

    drop(storage);
    let _ = std::fs::remove_file(db_path);
}

#[test]
fn clear_all_unpinned_keeps_recycle_bin_entries() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    for (content, pinned) in [
        ("active clipboard", false),
        ("archived clipboard", false),
        ("pinned clipboard", true),
    ] {
        assert!(storage
            .insert(&ClipboardEntry {
                id: 0,
                category: Category::Text,
                category_tags: vec![Category::Text],
                content_type: "text/plain".to_string(),
                content: content.to_string(),
                preview: content.to_string(),
                hash: Storage::hash_content(content),
                pinned,
                created_at: Utc::now(),
                original_content: None,
                updated_at: None,
                archived_at: None,
                version: 1,
            })
            .unwrap());
    }

    let archived_id = storage
        .query(&QueryFilter::default())
        .unwrap()
        .into_iter()
        .find(|entry| entry.content == "archived clipboard")
        .unwrap()
        .id;
    assert!(storage.archive_entry(archived_id).unwrap());

    assert_eq!(storage.clear_unpinned(false, None, false).unwrap(), 1);
    assert_eq!(storage.count(None, false).unwrap(), 1);
    assert_eq!(storage.archive_count().unwrap(), 1);

    drop(storage);
    let _ = std::fs::remove_file(db_path);
}

#[test]
fn batch_delete_archives_and_removes_mixed_categories() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    for (content, category) in [
        ("https://example.com", Category::Link),
        ("plain batch item", Category::Text),
    ] {
        let entry = ClipboardEntry {
            id: 0,
            category: category.clone(),
            category_tags: vec![category],
            content_type: "text/plain".to_string(),
            content: content.to_string(),
            preview: content.to_string(),
            hash: Storage::hash_content(content),
            pinned: false,
            created_at: Utc::now(),
            original_content: None,
            updated_at: None,
            archived_at: None,
            version: 1,
        };
        assert!(storage.insert(&entry).unwrap());
    }
    let ids = storage
        .query(&QueryFilter::default())
        .unwrap()
        .into_iter()
        .map(|entry| entry.id)
        .collect::<Vec<_>>();

    assert_eq!(storage.delete_entries(&ids, true).unwrap(), 2);
    assert_eq!(storage.count(None, false).unwrap(), 0);
    assert_eq!(storage.archive_count().unwrap(), 2);
    assert_eq!(storage.delete_entries(&ids, false).unwrap(), 2);
    assert_eq!(storage.archive_count().unwrap(), 0);

    drop(storage);
    let _ = std::fs::remove_file(db_path);
}

#[test]
fn empty_archives_permanently_remove_only_archived_records() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    for content in ["active clipboard", "archived clipboard"] {
        assert!(storage
            .insert(&ClipboardEntry {
                id: 0,
                category: Category::Text,
                category_tags: vec![Category::Text],
                content_type: "text/plain".to_string(),
                content: content.to_string(),
                preview: content.to_string(),
                hash: Storage::hash_content(content),
                pinned: false,
                created_at: Utc::now(),
                original_content: None,
                updated_at: None,
                archived_at: None,
                version: 1,
            })
            .unwrap());
    }
    let archived_entry_id = storage
        .query(&QueryFilter::default())
        .unwrap()
        .into_iter()
        .find(|entry| entry.content == "archived clipboard")
        .unwrap()
        .id;
    storage.archive_entry(archived_entry_id).unwrap();

    storage.create_memo("Active memo", "keep", "", &[]).unwrap();
    let archived_memo = storage
        .create_memo("Archived memo", "remove", "", &[])
        .unwrap();
    storage.archive_memo(archived_memo.id).unwrap();

    assert_eq!(storage.empty_archive().unwrap(), 1);
    assert_eq!(storage.empty_memo_archive().unwrap(), 1);
    assert_eq!(storage.archive_count().unwrap(), 0);
    assert_eq!(storage.memo_archive_count().unwrap(), 0);
    assert_eq!(storage.count(None, false).unwrap(), 1);
    assert_eq!(storage.memo_count().unwrap(), 1);

    drop(storage);
    let _ = std::fs::remove_file(db_path);
}

#[test]
fn backup_data_round_trips_between_databases() {
    let source_path = temp_db_path();
    let target_path = temp_db_path();
    let source = Storage::new(&source_path).unwrap();
    let target = Storage::new(&target_path).unwrap();

    let entry = ClipboardEntry {
        id: 0,
        category: Category::Text,
        category_tags: vec![Category::Text],
        content_type: "text".to_string(),
        content: "alpha beta release note".to_string(),
        preview: "alpha beta".to_string(),
        hash: Storage::hash_content("alpha beta release note"),
        pinned: true,
        created_at: Utc::now(),
        original_content: None,
        updated_at: None,
        archived_at: None,
        version: 1,
    };

    source.insert(&entry).unwrap();
    source
        .create_memo("Project note", "remember the backup package", "backup", &[])
        .unwrap();
    source.set_setting("language", "zh-CN").unwrap();

    let data = source.export_backup_data("2.3.2").unwrap();
    target.set_setting("language", "en").unwrap();
    let prepared = crate::transfer::prepare::Prepared::new(data).unwrap();
    let summary = crate::transfer::merge::local(&target, &prepared).unwrap();
    let repeated = crate::transfer::merge::local(&target, &prepared).unwrap();
    assert_eq!(
        (repeated.clipboard, repeated.memos, repeated.duplicates),
        (0, 0, 2)
    );

    assert_eq!(summary.clipboard, 1);
    assert_eq!(summary.memos, 1);
    assert_eq!(
        target
            .query(&QueryFilter {
                search: Some("alpha release".to_string()),
                ..Default::default()
            })
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        target
            .get_memos(&MemoFilter {
                search: Some("backup package".to_string()),
                ..Default::default()
            })
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        target.get_setting("language").unwrap(),
        Some("en".to_string())
    );

    std::fs::remove_file(source_path).ok();
    std::fs::remove_file(target_path).ok();
}

#[test]
fn schema_migration_backfills_persisted_memo_auto_tags() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    storage
        .create_memo("Contact", "user@example.com", "", &[])
        .unwrap();
    storage.set_setting("schema_version", "3").unwrap();
    drop(storage);

    let migrated = Storage::new(&db_path).unwrap();
    let memos = migrated.get_memos(&MemoFilter::default()).unwrap();
    assert_eq!(memos.len(), 1);
    assert_eq!(memos[0].auto_tags, vec!["email".to_string()]);
    assert_eq!(
        migrated.get_setting("schema_version").unwrap().as_deref(),
        Some(crate::storage_migrations::SCHEMA_VERSION),
    );

    drop(migrated);
    std::fs::remove_file(db_path).ok();
}

#[test]
fn schema_migration_separates_localized_auto_tags_from_manual_tags() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    let memo = storage
        .create_memo(
            "Contact",
            "user@example.com",
            "project",
            &["email".to_string()],
        )
        .unwrap();
    {
        let conn = storage.conn.lock().unwrap();
        conn.execute(
            "UPDATE memos SET tags = ?1 WHERE id = ?2",
            params!["project,EMAIL,邮箱", memo.id],
        )
        .unwrap();
    }
    storage.set_setting("schema_version", "5").unwrap();
    drop(storage);

    let migrated = Storage::new(&db_path).unwrap();
    let memos = migrated.get_memos(&MemoFilter::default()).unwrap();
    assert_eq!(memos[0].tags, "project");
    assert_eq!(memos[0].auto_tags, vec!["email".to_string()]);
    assert_eq!(
        migrated.get_setting("schema_version").unwrap().as_deref(),
        Some(crate::storage_migrations::SCHEMA_VERSION),
    );

    drop(migrated);
    std::fs::remove_file(db_path).ok();
}

#[test]
fn schema_migration_normalizes_legacy_memo_timestamps_to_utc() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    let memo = storage.create_memo("Time", "body", "", &[]).unwrap();
    {
        let conn = storage.conn.lock().unwrap();
        conn.execute(
            "UPDATE memos SET created_at = ?1, updated_at = ?2, archived_at = ?3 WHERE id = ?4",
            params![
                "2026-07-16 00:45:00",
                "2026-07-16 00:46:00",
                "2026-07-16 00:47:00",
                memo.id
            ],
        )
        .unwrap();
    }
    storage.set_setting("schema_version", "6").unwrap();
    drop(storage);

    let migrated = Storage::new(&db_path).unwrap();
    let memo = migrated
        .query_archived_memos(&MemoFilter::default())
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(memo.created_at, "2026-07-16T00:45:00Z");
    assert_eq!(memo.updated_at, "2026-07-16T00:46:00Z");
    assert_eq!(memo.archived_at.as_deref(), Some("2026-07-16T00:47:00Z"));

    drop(migrated);
    std::fs::remove_file(db_path).ok();
}

#[test]
fn manual_reclassification_updates_only_classification_metadata() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    let content = "E:/Code/SuperClipboard3/docs/index.html";
    let created_at = Utc::now();
    let original_content = Some("original path".to_string());
    let updated_at = Some("2026-07-17T01:02:03Z".to_string());
    storage
        .insert(&ClipboardEntry {
            id: 0,
            category: Category::Code,
            category_tags: vec![Category::Code, Category::Text],
            content_type: "text/plain".to_string(),
            content: content.to_string(),
            preview: content.to_string(),
            hash: Storage::hash_content(content),
            pinned: false,
            created_at,
            original_content: original_content.clone(),
            updated_at: updated_at.clone(),
            archived_at: None,
            version: 1,
        })
        .unwrap();
    let entry_id = storage.query(&QueryFilter::default()).unwrap()[0].id;
    {
        let conn = storage.conn.lock().unwrap();
        conn.execute(
            "UPDATE clipboard_entries
             SET original_content = ?1, updated_at = ?2
             WHERE id = ?3",
            params![original_content, updated_at, entry_id],
        )
        .unwrap();
    }

    assert_eq!(storage.reclassify_clipboard_entries().unwrap(), 1);
    assert_eq!(storage.reclassify_clipboard_entries().unwrap(), 0);
    assert_eq!(
        storage
            .get_setting("classification_rules_applied_version")
            .unwrap()
            .as_deref(),
        Some("1")
    );
    let entries = storage.query(&QueryFilter::default()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].category, Category::FilePath);
    assert_eq!(entries[0].category_tags, vec![Category::FilePath]);
    assert_eq!(entries[0].content, content);
    assert_eq!(entries[0].created_at, created_at);
    assert_eq!(entries[0].original_content, original_content);
    assert_eq!(entries[0].updated_at, updated_at);
    assert!(!entries[0].pinned);

    drop(storage);
    std::fs::remove_file(db_path).ok();
}

#[test]
fn search_ignores_embedded_image_payloads() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    let image_payload = "iVBORw0KGgoSEARCHPAYLOAD123456";
    let entry = ClipboardEntry {
        id: 0,
        category: Category::Image,
        category_tags: vec![Category::Image],
        content_type: "image/png".to_string(),
        content: image_payload.to_string(),
        preview: "[Image 640x480]".to_string(),
        hash: Storage::hash_content(image_payload),
        pinned: false,
        created_at: Utc::now(),
        original_content: None,
        updated_at: None,
        archived_at: None,
        version: 1,
    };
    storage.insert(&entry).unwrap();
    storage
        .create_memo(
            "Meeting note",
            "before image\n![image](data:image/png;base64,iVBORw0KGgoMEMOPAYLOAD123456)\nafter image",
            "project",
            &["image".to_string()],
        )
        .unwrap();

    assert!(storage
        .query(&QueryFilter {
            search: Some("SEARCHPAYLOAD".to_string()),
            ..Default::default()
        })
        .unwrap()
        .is_empty());
    assert_eq!(
        storage
            .query(&QueryFilter {
                search: Some("图片 640x480".to_string()),
                ..Default::default()
            })
            .unwrap()
            .len(),
        1
    );
    assert!(storage
        .get_memos(&MemoFilter {
            search: Some("MEMOPAYLOAD".to_string()),
            ..Default::default()
        })
        .unwrap()
        .is_empty());
    assert_eq!(
        storage
            .get_memos(&MemoFilter {
                search: Some("before after project 图片".to_string()),
                ..Default::default()
            })
            .unwrap()
            .len(),
        1
    );

    drop(storage);
    std::fs::remove_file(db_path).ok();
}

#[test]
fn schema_migration_backfills_search_text() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    let entry = ClipboardEntry {
        id: 0,
        category: Category::Text,
        category_tags: vec![Category::Link],
        content_type: "text/plain".to_string(),
        content: "https://migration.example.com".to_string(),
        preview: "https://migration.example.com".to_string(),
        hash: Storage::hash_content("https://migration.example.com"),
        pinned: false,
        created_at: Utc::now(),
        original_content: None,
        updated_at: None,
        archived_at: None,
        version: 1,
    };
    storage.insert(&entry).unwrap();
    storage
        .create_memo("Migration", "searchable history", "legacy", &[])
        .unwrap();
    {
        let conn = storage.conn.lock().unwrap();
        conn.execute("UPDATE clipboard_entries SET search_text = ''", [])
            .unwrap();
        conn.execute("UPDATE memos SET search_text = ''", [])
            .unwrap();
    }
    storage.set_setting("schema_version", "4").unwrap();
    drop(storage);

    let migrated = Storage::new(&db_path).unwrap();
    assert_eq!(
        migrated
            .query(&QueryFilter {
                search: Some("migration.example.com 链接".to_string()),
                ..Default::default()
            })
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        migrated
            .get_memos(&MemoFilter {
                search: Some("searchable legacy".to_string()),
                ..Default::default()
            })
            .unwrap()
            .len(),
        1
    );

    drop(migrated);
    std::fs::remove_file(db_path).ok();
}

#[test]
fn update_entry_preserves_first_copied_content() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    let entry = ClipboardEntry {
        id: 0,
        category: Category::Text,
        category_tags: vec![Category::Text],
        content_type: "text".to_string(),
        content: "before edit".to_string(),
        preview: "before edit".to_string(),
        hash: Storage::hash_content("before edit"),
        pinned: false,
        created_at: Utc::now(),
        original_content: None,
        updated_at: None,
        archived_at: None,
        version: 1,
    };

    storage.insert(&entry).unwrap();
    let id = storage.query(&QueryFilter::default()).unwrap()[0].id;
    assert!(storage.update_entry(id, "after edit").unwrap());

    assert!(storage.update_entry(id, "after another edit").unwrap());

    let updated = storage.get_entry_by_id(id).unwrap().unwrap();
    assert_eq!(updated.content, "after another edit");
    assert_eq!(updated.original_content.as_deref(), Some("before edit"));
    assert!(updated.updated_at.is_some());

    let copied_edited_content = ClipboardEntry {
        id: 0,
        category: Category::Text,
        category_tags: vec![Category::Text],
        content_type: "text/plain".to_string(),
        content: "after another edit".to_string(),
        preview: "after another edit".to_string(),
        hash: Storage::hash_content("after another edit"),
        pinned: false,
        created_at: Utc::now(),
        original_content: None,
        updated_at: None,
        archived_at: None,
        version: 1,
    };
    assert!(!storage.insert(&copied_edited_content).unwrap());

    std::fs::remove_file(db_path).ok();
}

#[test]
fn duplicate_capture_promotes_active_entry_without_overwriting_edits() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    let first_created_at = Utc::now() - chrono::Duration::minutes(10);
    let original = ClipboardEntry {
        id: 0,
        category: Category::Text,
        category_tags: vec![Category::Text],
        content_type: "text/plain".to_string(),
        content: "first copy".to_string(),
        preview: "first copy".to_string(),
        hash: Storage::hash_content("first copy"),
        pinned: false,
        created_at: first_created_at,
        original_content: None,
        updated_at: None,
        archived_at: None,
        version: 1,
    };
    assert_eq!(
        storage.insert_or_promote(&original).unwrap(),
        ClipboardInsertResult::Inserted
    );
    let original_id = storage.query(&QueryFilter::default()).unwrap()[0].id;
    assert!(storage.update_entry(original_id, "edited copy").unwrap());

    let another = ClipboardEntry {
        id: 0,
        category: Category::Text,
        category_tags: vec![Category::Text],
        content_type: "text/plain".to_string(),
        content: "another item".to_string(),
        preview: "another item".to_string(),
        hash: Storage::hash_content("another item"),
        pinned: false,
        created_at: first_created_at + chrono::Duration::minutes(5),
        original_content: None,
        updated_at: None,
        archived_at: None,
        version: 1,
    };
    assert_eq!(
        storage.insert_or_promote(&another).unwrap(),
        ClipboardInsertResult::Inserted
    );

    let recaptured_at = first_created_at + chrono::Duration::minutes(15);
    let recaptured = ClipboardEntry {
        id: 0,
        category: Category::Text,
        category_tags: vec![Category::Text],
        content_type: "text/plain".to_string(),
        content: "edited copy".to_string(),
        preview: "edited copy".to_string(),
        hash: Storage::hash_content("edited copy"),
        pinned: false,
        created_at: recaptured_at,
        original_content: None,
        updated_at: None,
        archived_at: None,
        version: 1,
    };
    assert_eq!(
        storage.insert_or_promote(&recaptured).unwrap(),
        ClipboardInsertResult::Promoted
    );

    let entries = storage.query(&QueryFilter::default()).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].id, original_id);
    assert_eq!(entries[0].content, "edited copy");
    assert_eq!(entries[0].original_content.as_deref(), Some("first copy"));
    assert_eq!(entries[0].created_at, recaptured_at);

    std::fs::remove_file(db_path).ok();
}

#[test]
fn merged_clipboard_entry_rolls_back_when_a_source_is_missing() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    for content in ["merge source one", "merge source two"] {
        let entry = ClipboardEntry {
            id: 0,
            category: Category::Text,
            category_tags: vec![Category::Text],
            content_type: "text/plain".to_string(),
            content: content.to_string(),
            preview: content.to_string(),
            hash: Storage::hash_content(content),
            pinned: false,
            created_at: Utc::now(),
            original_content: None,
            updated_at: None,
            archived_at: None,
            version: 1,
        };
        assert!(storage.insert(&entry).unwrap());
    }
    let source_ids = storage
        .query(&QueryFilter::default())
        .unwrap()
        .into_iter()
        .map(|entry| entry.id)
        .collect::<Vec<_>>();
    let merged = ClipboardEntry {
        id: 0,
        category: Category::Text,
        category_tags: vec![Category::Text],
        content_type: "text/plain".to_string(),
        content: "merged result".to_string(),
        preview: "merged result".to_string(),
        hash: Storage::hash_content("merged result"),
        pinned: false,
        created_at: Utc::now(),
        original_content: None,
        updated_at: None,
        archived_at: None,
        version: 1,
    };
    let mut ids_with_missing_source = source_ids.clone();
    ids_with_missing_source.push(999_999);

    assert!(storage
        .merge_clipboard_entry(&merged, &ids_with_missing_source, true, false)
        .is_err());

    let remaining = storage.query(&QueryFilter::default()).unwrap();
    assert_eq!(remaining.len(), 2);
    assert!(remaining
        .iter()
        .all(|entry| entry.content != "merged result"));

    std::fs::remove_file(db_path).ok();
}

#[test]
fn update_entry_with_unchanged_content_keeps_metadata_empty() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    let entry = ClipboardEntry {
        id: 0,
        category: Category::Text,
        category_tags: vec![Category::Text],
        content_type: "text/plain".to_string(),
        content: "unchanged".to_string(),
        preview: "unchanged".to_string(),
        hash: Storage::hash_content("unchanged"),
        pinned: false,
        created_at: Utc::now(),
        original_content: None,
        updated_at: None,
        archived_at: None,
        version: 1,
    };
    storage.insert(&entry).unwrap();
    let id = storage.query(&QueryFilter::default()).unwrap()[0].id;

    assert!(storage.update_entry(id, "unchanged").unwrap());
    let stored = storage.get_entry_by_id(id).unwrap().unwrap();
    assert_eq!(stored.content, "unchanged");
    assert!(stored.original_content.is_none());
    assert!(stored.updated_at.is_none());

    std::fs::remove_file(db_path).ok();
}

#[test]
fn reverting_entry_to_original_clears_edit_metadata() {
    let db_path = temp_db_path();
    let storage = Storage::new(&db_path).unwrap();
    let entry = ClipboardEntry {
        id: 0,
        category: Category::Text,
        category_tags: vec![Category::Text],
        content_type: "text/plain".to_string(),
        content: "first copy".to_string(),
        preview: "first copy".to_string(),
        hash: Storage::hash_content("first copy"),
        pinned: false,
        created_at: Utc::now(),
        original_content: None,
        updated_at: None,
        archived_at: None,
        version: 1,
    };
    storage.insert(&entry).unwrap();
    let id = storage.query(&QueryFilter::default()).unwrap()[0].id;

    assert!(storage.update_entry(id, "temporary edit").unwrap());
    assert!(storage.update_entry(id, "first copy").unwrap());
    let stored = storage.get_entry_by_id(id).unwrap().unwrap();
    assert_eq!(stored.content, "first copy");
    assert!(stored.original_content.is_none());
    assert!(stored.updated_at.is_none());

    std::fs::remove_file(db_path).ok();
}
