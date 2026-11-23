pub(crate) mod merge;
pub(crate) mod prepare;
pub(crate) mod source;
#[cfg(test)]
mod tests;

use crate::{backup, remote_storage, AppState};
use prepare::{Counts, Prepared, PreviewItem};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Default, Clone)]
pub(crate) struct Transfers(Arc<Mutex<Option<Pending>>>);

struct Pending {
    token: String,
    generation: u64,
    data: Prepared,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    token: String,
    format: String,
    app_version: String,
    counts: Counts,
    sample: Vec<PreviewItem>,
}

#[tauri::command]
pub fn discover_legacy_sources(state: tauri::State<'_, AppState>) -> Result<Vec<PathBuf>, String> {
    let current = &state.data_directory;
    let Some(parent) = current.parent() else {
        return Ok(Vec::new());
    };
    let mut candidates = Vec::new();
    for directory in [
        current.clone(),
        parent.join("com.superclipboard3.app"),
        parent.join("SuperClipboard3"),
    ] {
        let file = directory.join("clipboard.db");
        if file.is_file() && !candidates.contains(&file) {
            candidates.push(file);
        }
    }
    Ok(candidates)
}

async fn job<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn preview_import(
    state: tauri::State<'_, AppState>,
    path: Option<PathBuf>,
) -> Result<ImportPreview, String> {
    let storage = state.storage.clone();
    let transfers = state.transfers.clone();
    job(move || {
        let workspace = storage.workspace.read().map_err(|e| e.to_string())?;
        let (format, data) = if let Some(path) = path {
            source::read_file(&path)?
        } else {
            if !remote_storage::is_remote_mode(&storage) {
                return Err("Connect to the old PostgreSQL server through Storage first".into());
            }
            let data =
                remote_storage::workspace_snapshot(&storage, true).map_err(|e| e.to_string())?;
            ("PostgreSQL".into(), data)
        };
        let data = Prepared::new(data)?;
        let token = Uuid::new_v4().to_string();
        let preview = ImportPreview {
            token: token.clone(),
            format,
            app_version: data.version.clone(),
            counts: data.counts.clone(),
            sample: data.sample.clone(),
        };
        *transfers.0.lock().map_err(|e| e.to_string())? = Some(Pending {
            token,
            generation: *workspace,
            data,
        });
        Ok(preview)
    })
    .await
}

#[tauri::command]
pub async fn commit_import(
    state: tauri::State<'_, AppState>,
    token: String,
) -> Result<merge::ImportResult, String> {
    let storage = state.storage.clone();
    let transfers = state.transfers.clone();
    job(move || {
        let workspace = storage.workspace.read().map_err(|e| e.to_string())?;
        let mut pending = transfers.0.lock().map_err(|e| e.to_string())?;
        let import = pending
            .as_ref()
            .filter(|preview| preview.token == token && preview.generation == *workspace)
            .ok_or("Import preview expired. Preview the source again.")?;
        let outcome = if remote_storage::is_remote_mode(&storage) {
            remote_storage::merge_workspace(&storage, &import.data)
                .map_err(|e| format!("Import rolled back: {e}"))
        } else {
            merge::local(&storage, &import.data)
        };
        if outcome.is_ok() {
            pending.take();
        }
        outcome
    })
    .await
}

#[tauri::command]
pub async fn export_workspace(
    state: tauri::State<'_, AppState>,
    path: PathBuf,
) -> Result<(), String> {
    let storage = state.storage.clone();
    job(move || {
        let _workspace = storage.workspace.read().map_err(|e| e.to_string())?;
        let data = source::export(&storage)?;
        let directory = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let output = tempfile::NamedTempFile::new_in(directory).map_err(|e| e.to_string())?;
        backup::write_backup_data(output.path(), &data)?;
        output.as_file().sync_all().map_err(|e| e.to_string())?;
        output
            .persist_noclobber(&path)
            .map_err(|e| format!("Could not save backup (choose a new filename): {}", e.error))?;
        Ok(())
    })
    .await
}
