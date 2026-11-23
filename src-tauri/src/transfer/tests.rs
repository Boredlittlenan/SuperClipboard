use super::{merge, prepare::Prepared, source};
use crate::storage::{MemoFilter, QueryFilter, Storage};
use base64::{engine::general_purpose::STANDARD, Engine};
use rusqlite::Connection;
use std::path::Path;

fn legacy_database(path: &Path) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        "CREATE TABLE clipboard_entries (
           id INTEGER PRIMARY KEY, category TEXT, content_type TEXT, content TEXT,
           preview TEXT, hash TEXT, pinned INTEGER, created_at TEXT
         );
         INSERT INTO clipboard_entries VALUES
           (1, 'link', 'text', 'https://example.com', 'Example', 'legacy-hash', 1, '2025-01-02 03:04:05');",
    ).unwrap();
}

#[test]
fn early_sqlite_is_detected_and_merged_without_changing_the_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.data");
    legacy_database(&path);
    let bytes = std::fs::read(&path).unwrap();
    let (format, data) = source::read_file(&path).unwrap();
    assert_eq!(format, "SQLite");
    assert!(data.memos.is_empty());
    let target = Storage::new(Path::new(":memory:")).unwrap();
    let prepared = Prepared::new(data).unwrap();
    assert_eq!(prepared.counts.favorites, 1);
    assert_eq!(merge::local(&target, &prepared).unwrap().clipboard, 1);
    assert_eq!(merge::local(&target, &prepared).unwrap().duplicates, 1);
    let entries = target
        .query(&QueryFilter {
            summary: true,
            favorites: true,
            ..Default::default()
        })
        .unwrap();
    assert!(entries[0].content.is_empty());
    assert_eq!(
        target
            .get_entry_by_id(entries[0].id)
            .unwrap()
            .unwrap()
            .content,
        "https://example.com"
    );
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn destination_error_rolls_back_clipboard_and_memo_import_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.db");
    legacy_database(&path);
    let (_, mut data) = source::read_file(&path).unwrap();
    let source = Storage::new(Path::new(":memory:")).unwrap();
    let memo = source
        .create_memo("Imported memo", "Content", "label", &[])
        .unwrap();
    data.memos.push(memo);
    let data = Prepared::new(data).unwrap();
    let target = Storage::new(Path::new(":memory:")).unwrap();
    target
        .create_memo("Existing memo", "Content", "", &[])
        .unwrap();
    target
        .conn
        .lock()
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER fail_import BEFORE INSERT ON memos
         BEGIN SELECT RAISE(ABORT, 'test destination failure'); END;",
        )
        .unwrap();
    assert!(merge::local(&target, &data).is_err());
    assert!(target.query(&QueryFilter::default()).unwrap().is_empty());
    assert_eq!(target.get_memos(&MemoFilter::default()).unwrap().len(), 1);
    target
        .conn
        .lock()
        .unwrap()
        .execute_batch("DROP TRIGGER fail_import")
        .unwrap();
    let success = merge::local(&target, &data).unwrap();
    assert_eq!(
        (success.clipboard, success.memos, success.duplicates),
        (1, 1, 0)
    );
}

#[test]
fn postgres_archive_timestamps_are_accepted() {
    let time = source::timestamp("2025-01-02 03:04:05.123456+00").unwrap();
    assert_eq!(time.timestamp_subsec_micros(), 123456);
}

#[test]
fn legacy_embedded_images_survive_merge_and_invalid_images_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.db");
    legacy_database(&path);
    let (_, mut data) = source::read_file(&path).unwrap();
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(2, 2)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let encoded = STANDARD.encode(png.into_inner());
    let text = format!("Review this image.\n![image](data:image/png;base64,{encoded})");
    let source = Storage::new(Path::new(":memory:")).unwrap();
    data.memos.push(
        source
            .create_memo("Image memo", &text, "legacy", &[])
            .unwrap(),
    );
    let entry = &mut data.clipboard_entries[0];
    entry.category = crate::classifier::Category::Image;
    entry.content_type = "image/png".into();
    entry.content.clone_from(&encoded);
    let prepared = Prepared::new(data.clone()).unwrap();
    let target = Storage::new(Path::new(":memory:")).unwrap();
    merge::local(&target, &prepared).unwrap();
    assert_eq!(
        target.get_memos(&MemoFilter::default()).unwrap()[0].body,
        text
    );
    assert_eq!(
        target.query(&QueryFilter::default()).unwrap()[0].content,
        encoded
    );
    data.clipboard_entries[0].content = "corrupt!base64".into();
    assert!(Prepared::new(data)
        .err()
        .unwrap()
        .contains("Clipboard record #1"));
}
