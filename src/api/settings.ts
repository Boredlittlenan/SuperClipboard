import { invoke } from '@tauri-apps/api/core';

/** Get a user setting value by key */
export async function getSetting(key: string): Promise<string | null> {
  return invoke('get_setting', { key });
}

/** Read multiple settings with one IPC call. Missing keys are omitted. */
export async function getSettings(keys: string[]): Promise<Record<string, string>> {
  return invoke('get_settings', { keys });
}

/** Set a user setting value */
export async function setSetting(key: string, value: string): Promise<void> {
  return invoke('set_setting', { key, value });
}

/** Persist multiple settings atomically with one IPC call. */
export async function setSettings(values: Record<string, string>): Promise<void> {
  return invoke('set_settings', { values });
}

/** Check if auto-start on boot is enabled */
export async function getAutostartEnabled(): Promise<boolean> {
  return invoke('get_autostart_enabled');
}

/** Enable or disable auto-start on boot */
export async function setAutostartEnabled(enabled: boolean): Promise<boolean> {
  return invoke('set_autostart_enabled', { enabled });
}

/** Get the current global shortcut */
export async function getShortcut(): Promise<string> {
  return invoke('get_shortcut');
}

/** Set a new global shortcut */
export async function setShortcut(shortcut: string): Promise<string> {
  return invoke('set_shortcut', { newShortcut: shortcut });
}

/** Temporarily disable or restore the global shortcut while recording a new one */
export async function setShortcutRecording(recording: boolean): Promise<void> {
  return invoke('set_shortcut_recording', { recording });
}

export interface UpdateInfo {
  currentVersion: string;
  latestVersion: string;
  downloadUrl: string;
  hasUpdate: boolean;
  releaseName: string;
  releaseNotes: string;
  publishedAt: string;
}

export interface BackupFileInfo {
  fileName: string;
  createdAt: string;
  sizeBytes: number;
  displayPath: string;
  appVersion: string;
  backupVersion: number;
}

export interface RestoreSummary {
  clipboardEntries: number;
  memos: number;
  settings: number;
}

export interface StorageStatusInfo {
  mode: 'local' | 'remote';
  health: 'local' | 'connected' | 'failed' | 'notReady';
  message: string;
}

/** Check for updates from GitHub Releases */
export async function checkUpdate(): Promise<UpdateInfo> {
  const info = await invoke<UpdateInfo & {
    current_version?: string;
    latest_version?: string;
    download_url?: string;
    has_update?: boolean;
    release_name?: string;
    release_notes?: string;
    published_at?: string;
  }>('check_update');

  return {
    currentVersion: info.currentVersion ?? info.current_version ?? '',
    latestVersion: info.latestVersion ?? info.latest_version ?? '',
    downloadUrl: info.downloadUrl ?? info.download_url ?? '',
    hasUpdate: info.hasUpdate ?? info.has_update ?? false,
    releaseName: info.releaseName ?? info.release_name ?? '',
    releaseNotes: info.releaseNotes ?? info.release_notes ?? '',
    publishedAt: info.publishedAt ?? info.published_at ?? '',
  };
}

/** Open a URL in the system default browser */
export async function openUrl(url: string): Promise<void> {
  return invoke('open_url', { url });
}

export async function testRemoteStorage(): Promise<string> {
  return invoke('test_remote_storage');
}

export async function initializeRemoteStorage(): Promise<void> {
  return invoke('initialize_remote_storage');
}

export async function getStorageStatus(): Promise<StorageStatusInfo> {
  return invoke('get_storage_status');
}

/** Set window always-on-top */
export async function setAlwaysOnTop(enabled: boolean): Promise<void> {
  return invoke('set_always_on_top', { enabled });
}

/** Copy entry to clipboard, hide window, and paste (Ctrl+V) to the active window */
export async function pasteToActiveWindow(id: number): Promise<boolean> {
  return invoke('paste_to_active_window', { id });
}

export async function createBackup(): Promise<BackupFileInfo> {
  return invoke('create_backup');
}

export async function listBackups(): Promise<BackupFileInfo[]> {
  return invoke('list_backups');
}

export async function restoreBackup(fileName: string): Promise<RestoreSummary> {
  return invoke('restore_backup', { fileName });
}

export async function openBackupFolder(): Promise<void> {
  return invoke('open_backup_folder');
}
