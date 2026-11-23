use crate::storage::BackupData;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use zip::write::FileOptions;

const PACKAGE_VERSION: u32 = 1;
const MANIFEST_FILE: &str = "manifest.json";
const DATA_FILE: &str = "data.json";
const CHECKSUMS_FILE: &str = "checksums.json";
const ASSETS_DIR: &str = "assets/";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupManifest {
    app: String,
    package_version: u32,
    backup_version: u32,
    app_version: String,
    created_at: String,
    data_file: String,
    assets_dir: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupChecksums {
    data_sha256: String,
}

pub fn read_backup_data(path: &Path) -> Result<BackupData, String> {
    read_package_backup(path)
}

pub fn write_backup_data(path: &Path, data: &BackupData) -> Result<(), String> {
    let data_json = serde_json::to_vec(data).map_err(|e| e.to_string())?;
    let data_sha256 = sha256_hex(&data_json);

    let manifest = BackupManifest {
        app: data.app.clone(),
        package_version: PACKAGE_VERSION,
        backup_version: data.backup_version,
        app_version: data.app_version.clone(),
        created_at: data.created_at.to_rfc3339(),
        data_file: DATA_FILE.to_string(),
        assets_dir: ASSETS_DIR.to_string(),
    };
    let checksums = BackupChecksums { data_sha256 };

    let file = File::create(path).map_err(|e| format!("Failed to write backup: {}", e))?;
    let mut archive = zip::ZipWriter::new(file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    archive
        .start_file(MANIFEST_FILE, options)
        .map_err(|e| format!("Failed to write backup manifest: {}", e))?;
    archive
        .write_all(&serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Failed to write backup manifest: {}", e))?;

    archive
        .start_file(DATA_FILE, options)
        .map_err(|e| format!("Failed to write backup data: {}", e))?;
    archive
        .write_all(&data_json)
        .map_err(|e| format!("Failed to write backup data: {}", e))?;

    archive
        .start_file(CHECKSUMS_FILE, options)
        .map_err(|e| format!("Failed to write backup checksums: {}", e))?;
    archive
        .write_all(&serde_json::to_vec_pretty(&checksums).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Failed to write backup checksums: {}", e))?;

    archive
        .add_directory(ASSETS_DIR, options)
        .map_err(|e| format!("Failed to write backup assets directory: {}", e))?;

    archive
        .finish()
        .map_err(|e| format!("Failed to finish backup package: {}", e))?;
    Ok(())
}

fn read_package_manifest(path: &Path) -> Result<BackupManifest, String> {
    read_zip_json(path, MANIFEST_FILE)
}

fn read_package_backup(path: &Path) -> Result<BackupData, String> {
    let manifest = read_package_manifest(path)?;
    if manifest.app != "SuperClipboard"
        || manifest.package_version != PACKAGE_VERSION
        || manifest.data_file != DATA_FILE
        || manifest.backup_version != 1
    {
        return Err("Unsupported SuperClipboard backup manifest".into());
    }
    let data_json = read_zip_file(path, DATA_FILE)?;
    let checksums: BackupChecksums = read_zip_json(path, CHECKSUMS_FILE)?;
    let actual_hash = sha256_hex(&data_json);
    if actual_hash != checksums.data_sha256 {
        return Err("Backup checksum mismatch".to_string());
    }

    serde_json::from_slice(&data_json).map_err(|e| format!("Invalid backup file: {}", e))
}

fn read_zip_json<T: for<'de> Deserialize<'de>>(path: &Path, name: &str) -> Result<T, String> {
    let bytes = read_zip_file(path, name)?;
    serde_json::from_slice(&bytes).map_err(|e| format!("Invalid backup package metadata: {}", e))
}

fn read_zip_file(path: &Path, name: &str) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| format!("Failed to read backup: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Invalid backup package: {}", e))?;
    let entry = archive
        .by_name(name)
        .map_err(|e| format!("Backup package is missing {}: {}", name, e))?;
    let limit = if name == DATA_FILE {
        512 * 1024 * 1024
    } else {
        64 * 1024
    };
    if entry.size() > limit {
        return Err(format!("Backup member {name} exceeds the size limit"));
    }
    let mut bytes = Vec::new();
    entry
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read {}: {}", name, e))?;
    if bytes.len() as u64 > limit {
        return Err(format!("Backup member {name} exceeds the size limit"));
    }
    Ok(bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::SettingEntry;
    use chrono::Utc;
    use uuid::Uuid;

    fn sample_backup() -> BackupData {
        BackupData {
            app: "SuperClipboard".to_string(),
            backup_version: 1,
            app_version: "2.3.2".to_string(),
            created_at: Utc::now(),
            clipboard_entries: Vec::new(),
            memos: Vec::new(),
            settings: vec![SettingEntry {
                key: "language".to_string(),
                value: "zh-CN".to_string(),
            }],
        }
    }

    fn temp_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("superclipboard-test-{}.scbackup", Uuid::new_v4()))
    }

    #[test]
    fn scbackup_package_round_trips_with_manifest_and_checksums() {
        let path = temp_path();
        let data = sample_backup();

        write_backup_data(&path, &data).unwrap();
        let restored = read_backup_data(&path).unwrap();

        assert_eq!(restored.app, "SuperClipboard");
        assert_eq!(restored.app_version, "2.3.2");
        assert_eq!(restored.settings.len(), 1);

        let file = File::open(&path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        assert!(archive.by_name(MANIFEST_FILE).is_ok());
        assert!(archive.by_name(DATA_FILE).is_ok());
        assert!(archive.by_name(CHECKSUMS_FILE).is_ok());
        assert!(archive.by_name(ASSETS_DIR).is_ok());

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn scbackup_rejects_checksum_mismatch() {
        let path = temp_path();
        let data = sample_backup();
        write_backup_data(&path, &data).unwrap();

        let file = File::options().read(true).write(true).open(&path).unwrap();
        let mut archive = zip::ZipWriter::new_append(file).unwrap();
        let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        archive.start_file(DATA_FILE, options).unwrap();
        archive.write_all(br#"{"app":"Broken"}"#).unwrap();
        archive.finish().unwrap();

        assert!(read_backup_data(&path).is_err());

        std::fs::remove_file(path).ok();
    }
}
