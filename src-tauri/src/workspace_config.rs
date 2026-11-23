use crate::{remote_storage, restart_remote_listener, storage::Storage, AppState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tauri::{Emitter, Manager};

#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum Target {
    Local,
    Remote { url: String },
}

#[tauri::command]
pub async fn switch_workspace(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    target: Target,
    classic_profiles: Option<String>,
    active_profile: Option<String>,
) -> Result<(), String> {
    let storage = state.storage.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut values = match target {
            Target::Local => HashMap::from([("storage_mode".to_string(), "local".to_string())]),
            Target::Remote { url } => {
                if url.len() > 4096 {
                    return Err("PostgreSQL URI is too large".into());
                }
                let values = HashMap::from([
                    ("storage_mode".to_string(), "remote".to_string()),
                    ("remote_db_connection_mode".to_string(), "url".to_string()),
                    (
                        "remote_db_url_protected".to_string(),
                        credentials::encode(url.trim())?,
                    ),
                    ("remote_db_ready".to_string(), "true".to_string()),
                ]);
                let staged = Storage::new(Path::new(":memory:")).map_err(|e| e.to_string())?;
                staged.set_settings(&values).map_err(|e| e.to_string())?;
                remote_storage::ensure_schema(&staged).map_err(|e| e.to_string())?;
                let mut values = values;
                let (marker, version) =
                    remote_storage::schema_marker(&staged).map_err(|e| e.to_string())?;
                values.insert(marker, version);
                values.insert(
                    "workspace_id".into(),
                    remote_storage::workspace_id(&staged).map_err(|e| e.to_string())?,
                );
                values
            }
        };
        let active = values
            .get("storage_mode")
            .is_some_and(|mode| mode == "remote");
        if active {
            values.insert(
                "classic_remote_active_profile".into(),
                active_profile.unwrap_or_default(),
            );
        }
        if let Some(profiles) = classic_profiles {
            values.insert(
                "classic_remote_profiles_protected".into(),
                protect_profiles(&profiles)?,
            );
        }
        {
            let mut generation = storage.workspace.write().map_err(|e| e.to_string())?;
            storage.set_settings(&values).map_err(|e| e.to_string())?;
            *generation += 1;
        }
        restart_remote_listener(&app, &app.state::<AppState>());
        let _ = app.emit("workspace-changed", ());
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn id(storage: &Storage) -> Result<String, String> {
    if remote_storage::is_remote_mode(storage) {
        storage
            .get_setting("workspace_id")
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Reconnect to the PostgreSQL workspace".into())
    } else {
        Ok("local".into())
    }
}
pub(crate) mod credentials;

fn protect_profiles(profiles: &str) -> Result<String, String> {
    if profiles.len() > 128 * 1024 {
        return Err("Saved PostgreSQL profiles exceed the size limit".into());
    }
    credentials::encode(profiles)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassicRemoteSettings {
    is_remote: bool,
    url: String,
    active_profile: String,
    profiles: String,
}

#[tauri::command]
pub fn get_classic_remote_settings(
    state: tauri::State<'_, AppState>,
) -> Result<ClassicRemoteSettings, String> {
    let storage = &state.storage;
    let values = storage
        .get_settings(&[
            "storage_mode".into(),
            "remote_db_url_protected".into(),
            "classic_remote_active_profile".into(),
            "classic_remote_profiles_protected".into(),
        ])
        .map_err(|e| e.to_string())?;
    let decode = |key: &str| {
        values
            .get(key)
            .map(|value| credentials::decode(value))
            .transpose()
    };
    Ok(ClassicRemoteSettings {
        is_remote: values
            .get("storage_mode")
            .is_some_and(|mode| mode == "remote"),
        url: decode("remote_db_url_protected")?.unwrap_or_default(),
        active_profile: values
            .get("classic_remote_active_profile")
            .cloned()
            .unwrap_or_default(),
        profiles: decode("classic_remote_profiles_protected")?.unwrap_or_default(),
    })
}

#[tauri::command]
pub fn replace_classic_remote_profiles(
    state: tauri::State<'_, AppState>,
    profiles: String,
) -> Result<(), String> {
    state
        .storage
        .set_setting(
            "classic_remote_profiles_protected",
            &protect_profiles(&profiles)?,
        )
        .map_err(|e| e.to_string())
}
