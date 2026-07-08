use crate::remote_storage;
use crate::storage::Storage;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStatusInfo {
    pub mode: String,
    pub health: String,
    pub message: String,
}

pub fn status(storage: &Storage) -> StorageStatusInfo {
    let configured_mode = storage
        .get_setting("storage_mode")
        .ok()
        .flatten()
        .unwrap_or_else(|| "local".to_string());

    if configured_mode != "remote" {
        return StorageStatusInfo {
            mode: "local".to_string(),
            health: "local".to_string(),
            message: String::new(),
        };
    }

    let ready = matches!(
        storage.get_setting("remote_db_ready"),
        Ok(Some(value)) if value == "true"
    );
    if !ready {
        return StorageStatusInfo {
            mode: "remote".to_string(),
            health: "notReady".to_string(),
            message: String::new(),
        };
    }

    match remote_storage::test_connection(storage) {
        Ok(message) => StorageStatusInfo {
            mode: "remote".to_string(),
            health: "connected".to_string(),
            message,
        },
        Err(err) => StorageStatusInfo {
            mode: "remote".to_string(),
            health: "failed".to_string(),
            message: err.to_string(),
        },
    }
}
