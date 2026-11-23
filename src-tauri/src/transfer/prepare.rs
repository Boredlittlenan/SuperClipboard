use super::source::timestamp;
use crate::{
    classifier::Category,
    search_index::{clipboard_search_text, memo_search_text},
    storage::{BackupData, ClipboardEntry, Memo, Storage},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub clipboard: usize,
    pub memos: usize,
    pub images: usize,
    pub favorites: usize,
    pub archived: usize,
}

#[derive(Clone, Serialize)]
pub struct PreviewItem {
    pub kind: &'static str,
    pub label: String,
}

pub struct Prepared {
    pub entries: Vec<ImportEntry>,
    pub memos: Vec<ImportMemo>,
    pub counts: Counts,
    pub sample: Vec<PreviewItem>,
    pub version: String,
}

pub struct ImportEntry {
    pub record: ClipboardEntry,
    pub tags: String,
    pub search: String,
    pub current_hash: String,
    pub created: String,
}

pub struct ImportMemo {
    pub record: Memo,
    pub auto_tags: String,
    pub search: String,
    pub key: String,
}

fn validate_image(mime: &str, encoded: &str) -> Result<(), String> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| "Malformed Base64 image")?;
    let format = image::guess_format(&bytes).map_err(|_| "Unrecognized image bytes")?;
    if format.to_mime_type() != mime
        || !matches!(
            mime,
            "image/png" | "image/jpeg" | "image/gif" | "image/webp"
        )
    {
        return Err("Unsupported image type or MIME mismatch".into());
    }
    Ok(())
}

fn normalized_date(value: &mut Option<String>) -> Result<(), String> {
    if let Some(date) = value {
        *date = timestamp(date)?.to_rfc3339();
    }
    Ok(())
}

fn clip(mut record: ClipboardEntry) -> Result<ImportEntry, String> {
    if record.content.is_empty()
        || record.content.contains('\0')
        || record
            .original_content
            .as_ref()
            .is_some_and(|text| text.contains('\0'))
    {
        return Err("Empty content or unsupported NUL character".into());
    }
    if record.content_type.starts_with("image/") {
        validate_image(&record.content_type, &record.content)?;
    }
    normalized_date(&mut record.updated_at)?;
    normalized_date(&mut record.archived_at)?;
    let mut tags = vec![record.category.clone()];
    for tag in &record.category_tags {
        if !tags.contains(tag) {
            tags.push(tag.clone());
        }
    }
    let search = clipboard_search_text(
        &record.content_type,
        &record.content,
        &record.preview,
        &tags,
    );
    let current_hash = Storage::hash_content(&record.content);
    let created = record.created_at.to_rfc3339();
    Ok(ImportEntry {
        record,
        tags: serde_json::to_string(&tags).map_err(|e| e.to_string())?,
        search,
        current_hash,
        created,
    })
}

fn memo(mut record: Memo) -> Result<ImportMemo, String> {
    if [&record.title, &record.body, &record.tags]
        .iter()
        .any(|value| value.contains('\0'))
    {
        return Err("Unsupported NUL character".into());
    }
    record.created_at = timestamp(&record.created_at)?.to_rfc3339();
    record.updated_at = timestamp(&record.updated_at)?.to_rfc3339();
    normalized_date(&mut record.archived_at)?;
    static ATTACHMENT: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(data:image/[A-Za-z0-9.+-]+);base64,([^\s)]+)")
            .expect("valid attachment pattern")
    });
    for image in ATTACHMENT.captures_iter(&record.body) {
        validate_image(&image[1][5..], &image[2])?;
    }
    if record.auto_tags.is_empty() {
        record.auto_tags = crate::memo_tags::infer(&record.title, &record.body);
    }
    let search = memo_search_text(&record.title, &record.body, &record.tags, &record.auto_tags);
    let payload = serde_json::to_vec(&(
        &record.created_at,
        &record.title,
        &record.body,
        &record.tags,
    ))
    .map_err(|e| e.to_string())?;
    let key = Storage::hash_bytes(&payload);
    let auto_tags = serde_json::to_string(&record.auto_tags).map_err(|e| e.to_string())?;
    Ok(ImportMemo {
        record,
        auto_tags,
        search,
        key,
    })
}

impl Prepared {
    pub fn new(data: BackupData) -> Result<Self, String> {
        if data.app != "SuperClipboard" || data.backup_version != 1 {
            return Err("Unsupported SuperClipboard backup format/version".into());
        }
        let mut counts = Counts::default();
        let mut sample = Vec::new();
        let entries = data
            .clipboard_entries
            .into_iter()
            .map(|entry| {
                counts.clipboard += 1;
                counts.images += usize::from(entry.category == Category::Image);
                counts.favorites += usize::from(entry.pinned);
                counts.archived += usize::from(entry.archived_at.is_some());
                if sample.len() < 3 {
                    sample.push(PreviewItem {
                        kind: "clipboard",
                        label: entry.preview.chars().take(100).collect(),
                    });
                }
                let id = entry.id;
                clip(entry).map_err(|error| format!("Clipboard record #{id}: {error}"))
            })
            .collect::<Result<_, _>>()?;
        let memos = data
            .memos
            .into_iter()
            .map(|record| {
                counts.memos += 1;
                counts.favorites += usize::from(record.pinned);
                counts.archived += usize::from(record.archived_at.is_some());
                if counts.memos <= 3 {
                    sample.push(PreviewItem {
                        kind: "memo",
                        label: record.title.chars().take(100).collect(),
                    });
                }
                let id = record.id;
                memo(record).map_err(|error| format!("Memo #{id}: {error}"))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            entries,
            memos,
            counts,
            sample,
            version: data.app_version,
        })
    }
}
