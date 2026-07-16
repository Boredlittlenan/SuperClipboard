use crate::classifier::{classify_image, classify_image_tags, classify_text_tags, Category};
use crate::storage::{ClipboardEntry, Storage};
use crate::storage_backend;
use arboard::Clipboard;
use base64::Engine;
use chrono::Utc;
use log::{debug, error, info, warn};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::Emitter;

/// State shared across the clipboard monitoring thread
pub struct ClipboardMonitor {
    running: Arc<Mutex<bool>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl ClipboardMonitor {
    pub fn new() -> Self {
        Self {
            running: Arc::new(Mutex::new(false)),
            handle: None,
        }
    }

    /// Start the clipboard monitoring loop in a background thread
    pub fn start(&mut self, app_handle: tauri::AppHandle, storage: Arc<Storage>) {
        let running = self.running.clone();
        *running.lock().unwrap() = true;

        let handle = std::thread::Builder::new()
            .name("clipboard-monitor".into())
            .spawn(move || {
                info!("Clipboard monitor started");

                let mut clipboard = match Clipboard::new() {
                    Ok(c) => c,
                    Err(e) => {
                        error!("Failed to initialize clipboard: {}", e);
                        return;
                    }
                };

                // Track last seen clipboard payload to avoid re-processing.
                let mut last_clipboard_hash = String::new();

                // Poll interval: 300ms — responsive enough for UX, low CPU usage
                let poll_interval = Duration::from_millis(300);

                while *running.lock().unwrap() {
                    std::thread::sleep(poll_interval);

                    // Try to read image first
                    if let Ok(img) = clipboard.get_image() {
                        if img.width > 0 && img.height > 0 {
                            let image_hash = hash_image_payload(&img);

                            if image_hash != last_clipboard_hash {
                                last_clipboard_hash = image_hash.clone();

                                // Encode image as PNG base64
                                let img_data = encode_image_to_base64(&img);
                                if let Some(data) = img_data {
                                    let entry =
                                        make_image_entry(data, img.width, img.height, &image_hash);
                                    let insert_result =
                                        storage_backend::insert_entry(&storage, &entry);
                                    match insert_result {
                                        Ok(true) => {
                                            debug!("Captured image: {}x{}", img.width, img.height);
                                            let _ = app_handle.emit("clipboard-changed", &entry);
                                        }
                                        Ok(false) => {} // duplicate
                                        Err(e) => {
                                            warn!("Failed to store image: {}", e);
                                        }
                                    }
                                }
                                continue;
                            }
                        }
                    }

                    // Try to read text
                    if let Ok(text) = clipboard.get_text() {
                        if text.is_empty() {
                            continue;
                        }

                        let hash = Storage::hash_content(&text);
                        if hash == last_clipboard_hash {
                            continue;
                        }
                        last_clipboard_hash = hash.clone();

                        let entry = make_text_entry(text);
                        let category = entry.category.clone();
                        let insert_result = storage_backend::insert_entry(&storage, &entry);
                        match insert_result {
                            Ok(true) => {
                                debug!("Captured {}: {:?}", category, entry.preview);
                                let _ = app_handle.emit("clipboard-changed", &entry);
                            }
                            Ok(false) => {} // duplicate
                            Err(e) => {
                                warn!("Failed to store entry: {}", e);
                            }
                        }
                    }
                }

                info!("Clipboard monitor stopped");
            })
            .expect("Failed to spawn clipboard monitor thread");

        self.handle = Some(handle);
    }

    /// Stop the monitoring loop
    pub fn stop(&mut self) {
        *self.running.lock().unwrap() = false;
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Build a normalized text entry so manual imports and polling share category logic.
pub(crate) fn make_text_entry(text: String) -> ClipboardEntry {
    let category_tags = classify_text_tags(&text);
    let category = category_tags.first().cloned().unwrap_or(Category::Text);
    let preview = generate_preview(&text, &category);

    ClipboardEntry {
        id: 0,
        category,
        category_tags,
        content_type: "text/plain".to_string(),
        hash: Storage::hash_content(&text),
        content: text,
        preview,
        pinned: false,
        created_at: Utc::now(),
        original_content: None,
        updated_at: None,
        archived_at: None,
        version: 1,
    }
}

/// Build a normalized image entry from the raw RGBA payload used by the system clipboard.
pub(crate) fn make_image_entry(
    content: String,
    width: usize,
    height: usize,
    image_hash: &str,
) -> ClipboardEntry {
    ClipboardEntry {
        id: 0,
        category: classify_image(),
        category_tags: classify_image_tags(),
        content_type: "image/png".to_string(),
        preview: format!("[Image {width}x{height}]"),
        hash: Storage::hash_content(&format!("image:{image_hash}")),
        content,
        pinned: false,
        created_at: Utc::now(),
        original_content: None,
        updated_at: None,
        archived_at: None,
        version: 1,
    }
}

impl Drop for ClipboardMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Generate a short preview string for UI display
fn generate_preview(text: &str, category: &Category) -> String {
    match category {
        Category::Link => text.to_string(),
        Category::Email => text.to_string(),
        Category::FilePath => text.to_string(),
        Category::Image => "[Image]".to_string(),
        Category::Code => {
            let first_line = text.lines().next().unwrap_or("");
            let char_count = first_line.chars().count();
            if char_count > 80 {
                let truncated: String = first_line.chars().take(80).collect();
                format!("{}...", truncated)
            } else {
                first_line.to_string()
            }
        }
        Category::Text => {
            let clean = text.replace('\n', " ").replace('\r', "");
            let char_count = clean.chars().count();
            if char_count > 120 {
                let truncated: String = clean.chars().take(120).collect();
                format!("{}...", truncated)
            } else {
                clean
            }
        }
    }
}

fn hash_image_payload(img: &arboard::ImageData) -> String {
    let mut bytes = Vec::with_capacity(img.bytes.len() + 32);
    bytes.extend_from_slice(b"image/png-raw");
    bytes.extend_from_slice(&(img.width as u64).to_le_bytes());
    bytes.extend_from_slice(&(img.height as u64).to_le_bytes());
    bytes.extend_from_slice(&img.bytes);
    Storage::hash_bytes(&bytes)
}

/// Encode arboard image data to base64 PNG
fn encode_image_to_base64(img: &arboard::ImageData) -> Option<String> {
    use image::{ImageBuffer, Rgba};

    let img_buffer: ImageBuffer<Rgba<u8>, _> =
        ImageBuffer::from_raw(img.width as u32, img.height as u32, img.bytes.to_vec())?;

    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    if img_buffer
        .write_to(&mut cursor, image::ImageFormat::Png)
        .is_err()
    {
        return None;
    }

    Some(base64::engine::general_purpose::STANDARD.encode(&buf))
}
