import { useCallback, useEffect, useRef, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { Database, Network, Trash2 } from 'lucide-react';
import {
  createBackup,
  getSetting,
  getStorageStatus,
  initializeRemoteStorage,
  listBackups,
  openBackupFolder,
  restoreBackup,
  setSetting,
  testRemoteStorage,
} from '../api/settings';
import type { BackupFileInfo, StorageStatusInfo } from '../api/settings';
import { useI18n } from '../i18n';
import ConfirmDialog, { type ConfirmDialogState } from './ConfirmDialog';

type StorageMode = 'local' | 'remote';
type ConnectionMode = 'url' | 'manual';
type BackupAction = 'idle' | 'creating' | 'restoring';

const SETTING_KEYS = {
  storageMode: 'storage_mode',
  connectionMode: 'remote_db_connection_mode',
  url: 'remote_db_url',
  host: 'remote_db_host',
  port: 'remote_db_port',
  database: 'remote_db_database',
  username: 'remote_db_username',
  password: 'remote_db_password',
  sslMode: 'remote_db_ssl_mode',
  ready: 'remote_db_ready',
  profiles: 'remote_db_profiles',
};

type StoredSettingsPayload = Record<string, string>;

interface RemoteDbProfile {
  id: string;
  name: string;
  connectionMode: ConnectionMode;
  url: string;
  host: string;
  port: string;
  database: string;
  username: string;
  password: string;
  sslMode: string;
  lastUsedAt: string;
}

function formatBackupSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatBackupTime(value: string): string {
  if (!value) return '';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  const month = date.getMonth() + 1;
  const day = date.getDate();
  const hours = String(date.getHours()).padStart(2, '0');
  const minutes = String(date.getMinutes()).padStart(2, '0');
  return `${month}/${day} ${hours}:${minutes}`;
}

function hashString(value: string): string {
  let hash = 2166136261;
  for (let i = 0; i < value.length; i += 1) {
    hash ^= value.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(36);
}

function withoutUrlPassword(value: string): string {
  try {
    const parsed = new URL(value);
    parsed.password = '';
    return parsed.toString();
  } catch {
    return value.replace(/:\/\/([^:@/]+):[^@/]*@/, '://$1@');
  }
}

function profileSignature(payload: StoredSettingsPayload): string {
  const mode = payload[SETTING_KEYS.connectionMode] === 'manual' ? 'manual' : 'url';
  const sslMode = payload[SETTING_KEYS.sslMode] || 'prefer';
  if (mode === 'manual') {
    return [
      mode,
      payload[SETTING_KEYS.host]?.trim().toLowerCase() ?? '',
      payload[SETTING_KEYS.port]?.trim() || '5432',
      payload[SETTING_KEYS.database]?.trim().toLowerCase() ?? '',
      payload[SETTING_KEYS.username]?.trim().toLowerCase() ?? '',
      sslMode,
    ].join('|');
  }
  return [mode, withoutUrlPassword(payload[SETTING_KEYS.url]?.trim() ?? '').toLowerCase(), sslMode].join('|');
}

function profileId(payload: StoredSettingsPayload): string {
  return `remote-${hashString(profileSignature(payload))}`;
}

function profileLabel(payload: StoredSettingsPayload): string {
  const mode = payload[SETTING_KEYS.connectionMode] === 'manual' ? 'manual' : 'url';
  if (mode === 'manual') {
    const host = payload[SETTING_KEYS.host]?.trim();
    const port = payload[SETTING_KEYS.port]?.trim() || '5432';
    const database = payload[SETTING_KEYS.database]?.trim();
    const username = payload[SETTING_KEYS.username]?.trim();
    return [username && `${username}@`, host, host && `:${port}`, database && `/${database}`]
      .filter(Boolean)
      .join('') || 'PostgreSQL';
  }

  const rawUrl = payload[SETTING_KEYS.url]?.trim() ?? '';
  try {
    const parsed = new URL(rawUrl);
    const database = parsed.pathname.replace(/^\//, '');
    return [parsed.username && `${parsed.username}@`, parsed.host, database && `/${database}`]
      .filter(Boolean)
      .join('') || parsed.host || 'PostgreSQL';
  } catch {
    return withoutUrlPassword(rawUrl) || 'PostgreSQL';
  }
}

function profileFromPayload(payload: StoredSettingsPayload, existingId?: string): RemoteDbProfile {
  return {
    id: existingId || profileId(payload),
    name: profileLabel(payload),
    connectionMode: payload[SETTING_KEYS.connectionMode] === 'manual' ? 'manual' : 'url',
    url: payload[SETTING_KEYS.url] ?? '',
    host: payload[SETTING_KEYS.host] ?? '',
    port: payload[SETTING_KEYS.port] || '5432',
    database: payload[SETTING_KEYS.database] ?? '',
    username: payload[SETTING_KEYS.username] ?? '',
    password: payload[SETTING_KEYS.password] ?? '',
    sslMode: payload[SETTING_KEYS.sslMode] || 'prefer',
    lastUsedAt: new Date().toISOString(),
  };
}

function payloadFromProfile(profile: RemoteDbProfile, ready: boolean): StoredSettingsPayload {
  return {
    [SETTING_KEYS.storageMode]: 'remote',
    [SETTING_KEYS.connectionMode]: profile.connectionMode,
    [SETTING_KEYS.url]: profile.url,
    [SETTING_KEYS.host]: profile.host,
    [SETTING_KEYS.port]: profile.port || '5432',
    [SETTING_KEYS.database]: profile.database,
    [SETTING_KEYS.username]: profile.username,
    [SETTING_KEYS.password]: profile.password,
    [SETTING_KEYS.sslMode]: profile.sslMode || 'prefer',
    [SETTING_KEYS.ready]: ready ? 'true' : 'false',
  };
}

function parseProfiles(value: string | null): RemoteDbProfile[] {
  if (!value) return [];
  try {
    const parsed = JSON.parse(value);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .map((item): RemoteDbProfile | null => {
        if (!item || typeof item !== 'object') return null;
        const profile = item as Partial<RemoteDbProfile>;
        const payload = payloadFromProfile({
          id: typeof profile.id === 'string' ? profile.id : '',
          name: typeof profile.name === 'string' ? profile.name : '',
          connectionMode: profile.connectionMode === 'manual' ? 'manual' : 'url',
          url: typeof profile.url === 'string' ? profile.url : '',
          host: typeof profile.host === 'string' ? profile.host : '',
          port: typeof profile.port === 'string' ? profile.port : '5432',
          database: typeof profile.database === 'string' ? profile.database : '',
          username: typeof profile.username === 'string' ? profile.username : '',
          password: typeof profile.password === 'string' ? profile.password : '',
          sslMode: typeof profile.sslMode === 'string' ? profile.sslMode : 'prefer',
          lastUsedAt: typeof profile.lastUsedAt === 'string' ? profile.lastUsedAt : '',
        }, false);
        const id = profile.id || profileId(payload);
        return {
          ...profileFromPayload(payload, id),
          name: profile.name || profileLabel(payload),
          lastUsedAt: profile.lastUsedAt || '',
        };
      })
      .filter((profile): profile is RemoteDbProfile => Boolean(profile))
      .slice(0, 12);
  } catch {
    return [];
  }
}

function upsertProfile(profiles: RemoteDbProfile[], nextProfile: RemoteDbProfile): RemoteDbProfile[] {
  const nextSignature = profileSignature(payloadFromProfile(nextProfile, false));
  return [
    nextProfile,
    ...profiles.filter((profile) => (
      profile.id !== nextProfile.id
      && profileSignature(payloadFromProfile(profile, false)) !== nextSignature
    )),
  ].slice(0, 12);
}

function profileIdForPayload(profiles: RemoteDbProfile[], payload: StoredSettingsPayload): string {
  const signature = profileSignature(payload);
  return profiles.find((profile) => profileSignature(payloadFromProfile(profile, false)) === signature)?.id
    || profileId(payload);
}

function formatProfileTime(value: string): string {
  return formatBackupTime(value);
}

interface RemoteStorageButtonProps {
  onStorageModeChange?: (mode: StorageMode) => void;
}

export default function RemoteStorageButton({ onStorageModeChange }: RemoteStorageButtonProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [activeStorageMode, setActiveStorageMode] = useState<StorageMode>('local');
  const [storageMode, setStorageMode] = useState<StorageMode>('local');
  const [connectionMode, setConnectionMode] = useState<ConnectionMode>('url');
  const [url, setUrl] = useState('');
  const [host, setHost] = useState('');
  const [port, setPort] = useState('5432');
  const [database, setDatabase] = useState('');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [sslMode, setSslMode] = useState('prefer');
  const [storageStatus, setStorageStatus] = useState<StorageStatusInfo | null>(null);
  const [saveState, setSaveState] = useState<'idle' | 'saving' | 'saved' | 'failed'>('idle');
  const [testState, setTestState] = useState<'idle' | 'testing' | 'ready' | 'failed'>('idle');
  const [testMessage, setTestMessage] = useState('');
  const [backups, setBackups] = useState<BackupFileInfo[]>([]);
  const [selectedBackup, setSelectedBackup] = useState('');
  const [appVersion, setAppVersion] = useState('');
  const [backupAction, setBackupAction] = useState<BackupAction>('idle');
  const [backupStatus, setBackupStatus] = useState<'idle' | 'success' | 'failed'>('idle');
  const [backupMessage, setBackupMessage] = useState('');
  const [restoreConfirming, setRestoreConfirming] = useState(false);
  const [profiles, setProfiles] = useState<RemoteDbProfile[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState('');
  const [deleteProfileTarget, setDeleteProfileTarget] = useState<RemoteDbProfile | null>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const persistedSettingsRef = useRef<StoredSettingsPayload | null>(null);

  const writeSettings = useCallback(async (payload: StoredSettingsPayload) => {
    await Promise.all(Object.entries(payload).map(([key, value]) => setSetting(key, value)));
  }, []);

  const refreshBackups = useCallback(async () => {
    const items = await listBackups();
    setBackups(items);
    setSelectedBackup((current) => (
      current && items.some((item) => item.fileName === current)
        ? current
        : items[0]?.fileName ?? ''
    ));
    return items;
  }, []);

  const selectedBackupInfo = backups.find((backup) => backup.fileName === selectedBackup);

  const refreshStorageStatus = useCallback(async () => {
    const status = await getStorageStatus();
    setStorageStatus(status);
    return status;
  }, []);

  const persistProfiles = useCallback(async (items: RemoteDbProfile[]) => {
    setProfiles(items);
    await setSetting(SETTING_KEYS.profiles, JSON.stringify(items));
  }, []);

  const readProfiles = useCallback(async () => {
    return parseProfiles(await getSetting(SETTING_KEYS.profiles));
  }, []);

  const buildSettingsPayload = useCallback((ready: boolean): StoredSettingsPayload => ({
    [SETTING_KEYS.storageMode]: storageMode,
    [SETTING_KEYS.connectionMode]: connectionMode,
    [SETTING_KEYS.url]: url.trim(),
    [SETTING_KEYS.host]: host.trim(),
    [SETTING_KEYS.port]: port.trim() || '5432',
    [SETTING_KEYS.database]: database.trim(),
    [SETTING_KEYS.username]: username.trim(),
    [SETTING_KEYS.password]: password,
    [SETTING_KEYS.sslMode]: sslMode,
    [SETTING_KEYS.ready]: ready ? 'true' : 'false',
  }), [connectionMode, database, host, password, port, sslMode, storageMode, url, username]);

  const getPayloadActiveMode = useCallback((payload: StoredSettingsPayload | null): StorageMode => {
    return payload?.[SETTING_KEYS.storageMode] === 'remote' && payload?.[SETTING_KEYS.ready] === 'true'
      ? 'remote'
      : 'local';
  }, []);

  const applyPayloadToForm = useCallback((payload: StoredSettingsPayload) => {
    setStorageMode(payload[SETTING_KEYS.storageMode] === 'remote' ? 'remote' : 'local');
    setConnectionMode(payload[SETTING_KEYS.connectionMode] === 'manual' ? 'manual' : 'url');
    setUrl(payload[SETTING_KEYS.url] ?? '');
    setHost(payload[SETTING_KEYS.host] ?? '');
    setPort(payload[SETTING_KEYS.port] || '5432');
    setDatabase(payload[SETTING_KEYS.database] ?? '');
    setUsername(payload[SETTING_KEYS.username] ?? '');
    setPassword(payload[SETTING_KEYS.password] ?? '');
    setSslMode(payload[SETTING_KEYS.sslMode] || 'prefer');
  }, []);

  const loadSettings = useCallback(async () => {
    const [mode, connMode, savedUrl, savedHost, savedPort, savedDatabase, savedUsername, savedPassword, savedSslMode, ready, savedProfiles] = await Promise.all([
      getSetting(SETTING_KEYS.storageMode),
      getSetting(SETTING_KEYS.connectionMode),
      getSetting(SETTING_KEYS.url),
      getSetting(SETTING_KEYS.host),
      getSetting(SETTING_KEYS.port),
      getSetting(SETTING_KEYS.database),
      getSetting(SETTING_KEYS.username),
      getSetting(SETTING_KEYS.password),
      getSetting(SETTING_KEYS.sslMode),
      getSetting(SETTING_KEYS.ready),
      getSetting(SETTING_KEYS.profiles),
    ]);
    const loadedStorageMode = mode === 'remote' ? 'remote' : 'local';
    const loadedRemoteActive = loadedStorageMode === 'remote' && ready === 'true';
    const persistedPayload: StoredSettingsPayload = {
      [SETTING_KEYS.storageMode]: loadedStorageMode,
      [SETTING_KEYS.connectionMode]: connMode === 'manual' ? 'manual' : 'url',
      [SETTING_KEYS.url]: savedUrl ?? '',
      [SETTING_KEYS.host]: savedHost ?? '',
      [SETTING_KEYS.port]: savedPort || '5432',
      [SETTING_KEYS.database]: savedDatabase ?? '',
      [SETTING_KEYS.username]: savedUsername ?? '',
      [SETTING_KEYS.password]: savedPassword ?? '',
      [SETTING_KEYS.sslMode]: savedSslMode || 'prefer',
      [SETTING_KEYS.ready]: loadedRemoteActive ? 'true' : 'false',
    };

    persistedSettingsRef.current = persistedPayload;
    setActiveStorageMode(loadedRemoteActive ? 'remote' : 'local');
    applyPayloadToForm(persistedPayload);
    const loadedProfiles = parseProfiles(savedProfiles);
    setProfiles(loadedProfiles);
    setSelectedProfileId(loadedRemoteActive ? profileIdForPayload(loadedProfiles, persistedPayload) : '');
    setSaveState('idle');
    setTestState('idle');
    setTestMessage('');
  }, [applyPayloadToForm]);

  useEffect(() => {
    loadSettings().catch((err) => {
      console.error('Failed to load remote storage settings:', err);
      setSaveState('failed');
    });
  }, [loadSettings]);

  useEffect(() => {
    if (!open) return;
    loadSettings().catch((err) => {
      console.error('Failed to load remote storage settings:', err);
      setSaveState('failed');
    });
    refreshBackups().catch((err) => {
      console.error('Failed to load backups:', err);
      setBackupStatus('failed');
      setBackupMessage(t.backupFailed);
    });
    getVersion().then(setAppVersion).catch(console.error);
    refreshStorageStatus().catch(console.error);
    setBackupAction('idle');
    setBackupStatus('idle');
    setBackupMessage('');
    setRestoreConfirming(false);
  }, [open, loadSettings, refreshBackups, refreshStorageStatus, t.backupFailed]);

  useEffect(() => {
    if (!open) return;
    const handler = (event: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

  const handleSave = useCallback(async () => {
    const previousPayload = persistedSettingsRef.current;
    setSaveState('saving');
    setTestMessage('');
    try {
      if (storageMode === 'local') {
        const localPayload = buildSettingsPayload(false);
        await writeSettings(localPayload);
        persistedSettingsRef.current = localPayload;
        setActiveStorageMode('local');
        setSaveState('saved');
        await refreshStorageStatus();
        setOpen(false);
        onStorageModeChange?.('local');
        return;
      }

      setTestState('testing');
      const pendingPayload = buildSettingsPayload(false);
      await writeSettings(pendingPayload);
      const message = await testRemoteStorage();
      await initializeRemoteStorage();
      const readyPayload = { ...pendingPayload, [SETTING_KEYS.ready]: 'true' };
      await writeSettings(readyPayload);
      const savedProfile = profileFromPayload(readyPayload);
      const currentProfiles = await readProfiles();
      const nextProfiles = upsertProfile(currentProfiles, savedProfile);
      await persistProfiles(nextProfiles);
      persistedSettingsRef.current = readyPayload;
      setActiveStorageMode('remote');
      setSelectedProfileId(savedProfile.id);
      setTestState('ready');
      setTestMessage(message);
      setSaveState('saved');
      await refreshStorageStatus();
      setOpen(false);
      onStorageModeChange?.('remote');
    } catch (err) {
      console.error('Failed to save remote storage settings:', err);
      if (previousPayload) {
        await writeSettings(previousPayload).catch((restoreErr) => {
          console.error('Failed to restore previous remote storage settings:', restoreErr);
        });
        setActiveStorageMode(getPayloadActiveMode(previousPayload));
      }
      setSaveState('failed');
      setTestState(storageMode === 'remote' ? 'failed' : 'idle');
      setTestMessage(String(err));
    }
  }, [buildSettingsPayload, getPayloadActiveMode, onStorageModeChange, persistProfiles, readProfiles, refreshStorageStatus, storageMode, writeSettings]);

  const handleSelectProfile = useCallback((profile: RemoteDbProfile) => {
    setSelectedProfileId(profile.id);
    applyPayloadToForm(payloadFromProfile(profile, false));
    setSaveState('idle');
    setTestState('idle');
    setTestMessage('');
  }, [applyPayloadToForm]);

  const handleUseProfile = useCallback(async (profile: RemoteDbProfile) => {
    const previousPayload = persistedSettingsRef.current;
    setSelectedProfileId(profile.id);
    setSaveState('saving');
    setTestState('testing');
    setTestMessage('');
    applyPayloadToForm(payloadFromProfile(profile, false));
    try {
      const pendingPayload = payloadFromProfile(profile, false);
      await writeSettings(pendingPayload);
      const message = await testRemoteStorage();
      await initializeRemoteStorage();
      const readyPayload = payloadFromProfile(profile, true);
      await writeSettings(readyPayload);
      const activatedProfile = {
        ...profile,
        name: profile.name || profileLabel(readyPayload),
        lastUsedAt: new Date().toISOString(),
      };
      const currentProfiles = await readProfiles();
      const nextProfiles = upsertProfile(currentProfiles, activatedProfile);
      await persistProfiles(nextProfiles);
      persistedSettingsRef.current = readyPayload;
      setActiveStorageMode('remote');
      setSaveState('saved');
      setTestState('ready');
      setTestMessage(message);
      await refreshStorageStatus();
      setOpen(false);
      onStorageModeChange?.('remote');
    } catch (err) {
      console.error('Failed to switch remote storage profile:', err);
      if (previousPayload) {
        await writeSettings(previousPayload).catch((restoreErr) => {
          console.error('Failed to restore previous remote storage settings:', restoreErr);
        });
        applyPayloadToForm(previousPayload);
        setActiveStorageMode(getPayloadActiveMode(previousPayload));
        setSelectedProfileId(getPayloadActiveMode(previousPayload) === 'remote' ? profileIdForPayload(profiles, previousPayload) : '');
      }
      setSaveState('failed');
      setTestState('failed');
      setTestMessage(String(err));
    }
  }, [applyPayloadToForm, getPayloadActiveMode, onStorageModeChange, persistProfiles, profiles, readProfiles, refreshStorageStatus, writeSettings]);

  const deleteProfile = useCallback(async (profileIdToDelete: string) => {
    const currentProfiles = await readProfiles();
    const nextProfiles = currentProfiles.filter((profile) => profile.id !== profileIdToDelete);
    await persistProfiles(nextProfiles);
    if (selectedProfileId === profileIdToDelete) {
      setSelectedProfileId('');
    }
  }, [persistProfiles, readProfiles, selectedProfileId]);

  const deleteProfileDialog: ConfirmDialogState | null = deleteProfileTarget
    ? {
        title: t.deleteRemoteProfile,
        message: t.deleteRemoteProfileConfirm(deleteProfileTarget.name),
        confirmLabel: t.delete,
        tone: 'danger',
        resolve: (confirmed) => {
          const profileId = deleteProfileTarget.id;
          setDeleteProfileTarget(null);
          if (confirmed) {
            deleteProfile(profileId).catch((err) => {
              console.error('Failed to delete remote storage profile:', err);
              setSaveState('failed');
              setTestMessage(String(err));
            });
          }
        },
      }
    : null;

  const handleCreateBackup = useCallback(async () => {
    setBackupAction('creating');
    setBackupStatus('idle');
    setBackupMessage('');
    setRestoreConfirming(false);
    try {
      const backup = await createBackup();
      const items = await refreshBackups();
      setSelectedBackup(backup.fileName || items[0]?.fileName || '');
      setBackupStatus('success');
      setBackupMessage(`${t.backupCreated}: ${backup.fileName}`);
    } catch (err) {
      console.error('Failed to create backup:', err);
      setBackupStatus('failed');
      setBackupMessage(t.backupFailed);
    } finally {
      setBackupAction('idle');
    }
  }, [refreshBackups, t.backupCreated, t.backupFailed]);

  const handleRestoreBackup = useCallback(async () => {
    if (!selectedBackup) return;

    if (!restoreConfirming) {
      setRestoreConfirming(true);
      setBackupStatus('idle');
      setBackupMessage(t.restoreBackupConfirm);
      return;
    }

    setBackupAction('restoring');
    setBackupStatus('idle');
    setBackupMessage('');
    try {
      const summary = await restoreBackup(selectedBackup);
      setBackupStatus('success');
      setRestoreConfirming(false);
      setBackupMessage(t.restoreBackupDone(summary.clipboardEntries, summary.memos, summary.settings));
      setTimeout(() => window.location.reload(), 900);
    } catch (err) {
      console.error('Failed to restore backup:', err);
      setBackupStatus('failed');
      setRestoreConfirming(false);
      setBackupMessage(t.backupFailed);
    } finally {
      setBackupAction('idle');
    }
  }, [restoreConfirming, selectedBackup, t]);

  const handleOpenBackupFolder = useCallback(() => {
    openBackupFolder().catch((err) => {
      console.error('Failed to open backup folder:', err);
      setBackupStatus('failed');
      setBackupMessage(t.backupFailed);
    });
  }, [t.backupFailed]);

  const headerStorageMode: StorageMode = storageStatus?.mode === 'remote' || activeStorageMode === 'remote'
    ? 'remote'
    : 'local';
  const headerStorageHealth = storageStatus?.health ?? (headerStorageMode === 'remote' ? 'notReady' : 'local');
  const headerStorageLabel = headerStorageMode === 'remote'
    ? t.storageStatusConnected
    : t.storageStatusLocal;

  return (
    <div style={styles.wrapper} ref={panelRef}>
      <button
        className="settings-gear-btn remote-storage-btn"
        style={styles.iconBtn}
        onClick={(event) => {
          event.stopPropagation();
          setOpen((value) => !value);
        }}
        title={t.remoteStorage}
      >
        {activeStorageMode === 'remote'
          ? <Network size={16} strokeWidth={2.15} />
          : <Database size={16} strokeWidth={2.15} />}
      </button>

      {open && (
        <div className="glass-menu-panel" style={styles.panel}>
          <div style={styles.panelTitle}>
            <span>{t.remoteStorage}</span>
            <span style={styles.headerStatus}>
              <span style={{
                ...styles.statusDot,
                ...(headerStorageHealth === 'connected' ? styles.statusDotOk : {}),
                ...(headerStorageHealth === 'failed' ? styles.statusDotError : {}),
                ...(headerStorageHealth === 'notReady' ? styles.statusDotWarn : {}),
              }} />
              {headerStorageLabel}
            </span>
          </div>

          <div style={styles.section}>
            <span style={styles.label}>{t.storageMode}</span>
            <div style={styles.segmented}>
              <button
                style={{
                  ...styles.segmentBtn,
                  ...(storageMode === 'local' ? styles.segmentBtnActive : {}),
                }}
                onClick={() => setStorageMode('local')}
              >
                {t.storageModeLocal}
              </button>
              <button
                style={{
                  ...styles.segmentBtn,
                  ...(storageMode === 'remote' ? styles.segmentBtnActive : {}),
                }}
                onClick={() => setStorageMode('remote')}
              >
                {t.storageModeRemote}
              </button>
            </div>
            <div style={styles.hint}>
              {storageMode === 'remote' ? t.remoteModeHint : t.localModeHint}
            </div>
          </div>

          {storageMode === 'remote' && (
            <>
              <div style={styles.section}>
                <span style={styles.label}>{t.connectionMode}</span>
                <div style={styles.segmented}>
                  <button
                    style={{
                      ...styles.segmentBtn,
                      ...(connectionMode === 'url' ? styles.segmentBtnActive : {}),
                    }}
                    onClick={() => setConnectionMode('url')}
                  >
                    {t.connectionUrl}
                  </button>
                  <button
                    style={{
                      ...styles.segmentBtn,
                      ...(connectionMode === 'manual' ? styles.segmentBtnActive : {}),
                    }}
                    onClick={() => setConnectionMode('manual')}
                  >
                    {t.connectionManual}
                  </button>
                </div>
              </div>

              {connectionMode === 'url' ? (
                <>
                  <label style={styles.field}>
                    <span style={styles.label}>{t.databaseUrl}</span>
                    <input
                      style={styles.input}
                      value={url}
                      onChange={(event) => setUrl(event.target.value)}
                      placeholder="postgres://user:password@host:5432/db"
                    />
                  </label>
                  <label style={styles.field}>
                    <span style={styles.label}>{t.databaseSsl}</span>
                    <select style={styles.input} value={sslMode} onChange={(event) => setSslMode(event.target.value)}>
                      <option value="require">require</option>
                      <option value="prefer">prefer</option>
                      <option value="verify-full">verify-full</option>
                      <option value="disable">disable</option>
                    </select>
                  </label>
                </>
              ) : (
                <div style={styles.grid}>
                  <label style={styles.field}>
                    <span style={styles.label}>{t.databaseHost}</span>
                    <input style={styles.input} value={host} onChange={(event) => setHost(event.target.value)} placeholder="db.example.com" />
                  </label>
                  <label style={styles.field}>
                    <span style={styles.label}>{t.databasePort}</span>
                    <input style={styles.input} value={port} onChange={(event) => setPort(event.target.value)} placeholder="5432" />
                  </label>
                  <label style={styles.field}>
                    <span style={styles.label}>{t.databaseUser}</span>
                    <input style={styles.input} value={username} onChange={(event) => setUsername(event.target.value)} placeholder="superclipboard_user" />
                  </label>
                  <label style={styles.field}>
                    <span style={styles.label}>{t.databaseName}</span>
                    <input style={styles.input} value={database} onChange={(event) => setDatabase(event.target.value)} placeholder="superclipboard" />
                  </label>
                  <label style={styles.field}>
                    <span style={styles.label}>{t.databasePassword}</span>
                    <input style={styles.input} type="password" value={password} onChange={(event) => setPassword(event.target.value)} />
                  </label>
                  <label style={styles.field}>
                    <span style={styles.label}>{t.databaseSsl}</span>
                    <select style={styles.input} value={sslMode} onChange={(event) => setSslMode(event.target.value)}>
                      <option value="require">require</option>
                      <option value="prefer">prefer</option>
                      <option value="verify-full">verify-full</option>
                      <option value="disable">disable</option>
                    </select>
                  </label>
                </div>
              )}
            </>
          )}

          <div style={styles.actions}>
            <button
              style={styles.primaryBtn}
              disabled={saveState === 'saving' || testState === 'testing'}
              onClick={handleSave}
              title={storageMode === 'remote' ? t.remoteStoragePending : undefined}
            >
              {saveState === 'saving' || testState === 'testing'
                ? t.savingStorageConfig
                : storageMode === 'remote'
                ? t.saveAndUseRemote
                : t.saveAndUseLocal}
            </button>
          </div>

          {storageMode === 'remote' && (
            <div style={styles.profileSection}>
              <div style={styles.profileHeader}>
                <span style={styles.label}>{t.remoteProfiles}</span>
              </div>
              {profiles.length === 0 ? (
                <div style={styles.hint}>{t.noRemoteProfiles}</div>
              ) : (
                <div style={styles.profileList}>
                  {profiles.map((profile) => {
                    const isSelected = profile.id === selectedProfileId;
                    return (
                      <div
                        key={profile.id}
                        style={{
                          ...styles.profileItem,
                          ...(isSelected ? styles.profileItemSelected : {}),
                        }}
                      >
                        <button
                          type="button"
                          style={styles.profileMain}
                          onClick={() => handleSelectProfile(profile)}
                          title={profile.name}
                        >
                          <span style={styles.profileName}>{profile.name}</span>
                          <span style={styles.profileMeta}>
                            {profile.connectionMode === 'manual' ? t.connectionManual : t.connectionUrl}
                            {profile.lastUsedAt ? ` · ${t.lastUsedRemoteProfile(formatProfileTime(profile.lastUsedAt))}` : ''}
                          </span>
                        </button>
                        <button
                          type="button"
                          style={styles.profileUseBtn}
                          onClick={() => handleUseProfile(profile)}
                          disabled={saveState === 'saving' || testState === 'testing'}
                        >
                          {t.useRemoteProfile}
                        </button>
                        <button
                          type="button"
                          style={styles.profileDeleteBtn}
                          onClick={() => setDeleteProfileTarget(profile)}
                          title={t.deleteRemoteProfile}
                        >
                          <Trash2 size={12} strokeWidth={2.1} />
                        </button>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          )}

          {saveState === 'saved' && <div style={styles.successText}>{t.storageConfigSaved}</div>}
          {saveState === 'failed' && <div style={styles.errorText}>{t.storageConfigFailed}</div>}
          {testState === 'ready' && <div style={styles.successText}>{t.storageConnectionReady}: {testMessage}</div>}
          {testState === 'failed' && <div style={styles.errorText}>{t.storageConnectionFailed}: {testMessage}</div>}

          {storageMode === 'local' && (
            <>
              <div style={styles.divider} />

              <div style={styles.backupPanel} title={t.backupRestoreDesc}>
                <div style={styles.backupHeader}>
                  <div style={styles.backupTitleGroup}>
                    <span style={styles.sectionTitle}>{t.backupRestore}</span>
                    <span style={styles.betaBadge}>{t.backupBeta}</span>
                  </div>
                  <button style={styles.miniTextBtn} onClick={handleOpenBackupFolder}>
                    {t.openBackupFolder}
                  </button>
                </div>
                <div style={styles.backupNotice}>{t.backupCompatibilityNotice}</div>
                <button
                  style={styles.secondaryActionBtn}
                  onClick={handleCreateBackup}
                  disabled={backupAction !== 'idle'}
                >
                  {backupAction === 'creating' ? t.creatingBackup : t.createBackup}
                </button>
                <div style={styles.backupRestoreRow}>
                  <select
                    style={styles.backupSelect}
                    value={selectedBackup}
                    onChange={(event) => {
                      setSelectedBackup(event.target.value);
                      setRestoreConfirming(false);
                      setBackupMessage('');
                      setBackupStatus('idle');
                    }}
                    disabled={backups.length === 0 || backupAction !== 'idle'}
                  >
                    {backups.length === 0 ? (
                      <option value="">{t.noBackups}</option>
                    ) : (
                      backups.map((backup) => (
                        <option key={backup.fileName} value={backup.fileName}>
                          {formatBackupTime(backup.createdAt)} · v{backup.appVersion || t.unknownVersion} · {formatBackupSize(backup.sizeBytes)}
                        </option>
                      ))
                    )}
                  </select>
                  <button
                    style={{
                      ...styles.restoreBtn,
                      ...(restoreConfirming ? styles.restoreBtnConfirm : {}),
                    }}
                    onClick={handleRestoreBackup}
                    disabled={!selectedBackup || backupAction !== 'idle'}
                  >
                    {backupAction === 'restoring' ? t.restoringBackup : t.restoreBackup}
                  </button>
                </div>
                {selectedBackupInfo && (
                  <div style={styles.backupVersionLine}>
                    {t.backupVersionMeta(selectedBackupInfo.appVersion || t.unknownVersion, appVersion || t.unknownVersion)}
                  </div>
                )}
                {backupMessage && (
                  <div style={{
                    ...styles.backupMessage,
                    ...(backupStatus === 'failed' ? styles.backupMessageError : {}),
                    ...(backupStatus === 'success' ? styles.backupMessageSuccess : {}),
                  }}>
                    {backupMessage}
                  </div>
                )}
              </div>
            </>
          )}

          {deleteProfileDialog && (
            <ConfirmDialog
              dialog={deleteProfileDialog}
              onClose={() => setDeleteProfileTarget(null)}
            />
          )}
        </div>
      )}
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  wrapper: {
    position: 'relative',
    zIndex: 1210,
  },
  iconBtn: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    width: '28px',
    height: '28px',
    border: 'none',
    borderRadius: '6px',
    background: 'transparent',
    color: 'var(--text-secondary)',
    cursor: 'pointer',
    transition: 'background 0.15s, color 0.15s',
  },
  panel: {
    position: 'absolute',
    top: '36px',
    right: '0',
    width: '300px',
    maxHeight: 'calc(100vh - 48px)',
    overflowY: 'auto',
    background: 'var(--panel-glass)',
    border: '1px solid var(--apple-separator)',
    borderRadius: '12px',
    padding: '12px',
    zIndex: 1320,
    boxShadow: '0 18px 46px rgba(15, 23, 42, 0.2), inset 0 1px 0 var(--hairline-highlight)',
    backdropFilter: 'blur(44px) saturate(1.9)',
    WebkitBackdropFilter: 'blur(44px) saturate(1.9)',
  },
  panelTitle: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    fontSize: '13px',
    fontWeight: 600,
    color: 'var(--text-primary)',
    marginBottom: '10px',
    paddingBottom: '8px',
    borderBottom: '1px solid var(--border)',
  },
  headerStatus: {
    display: 'flex',
    alignItems: 'center',
    gap: '6px',
    color: 'var(--text-muted)',
    fontSize: '11px',
    fontWeight: 600,
  },
  statusDot: {
    width: '6px',
    height: '6px',
    borderRadius: '999px',
    background: 'var(--text-muted)',
    flexShrink: 0,
  },
  statusDotOk: {
    background: 'var(--success)',
  },
  statusDotError: {
    background: 'var(--danger)',
  },
  statusDotWarn: {
    background: '#f59e0b',
  },
  section: {
    display: 'flex',
    flexDirection: 'column',
    gap: '6px',
    marginBottom: '10px',
  },
  profileSection: {
    display: 'flex',
    flexDirection: 'column',
    gap: '6px',
    marginTop: '10px',
    marginBottom: '6px',
  },
  label: {
    fontSize: '11px',
    color: 'var(--text-secondary)',
    fontWeight: 600,
  },
  sectionTitle: {
    fontSize: '12px',
    color: 'var(--text-primary)',
    fontWeight: 700,
  },
  segmented: {
    display: 'flex',
    border: '1px solid var(--border)',
    borderRadius: '6px',
    overflow: 'hidden',
  },
  segmentBtn: {
    flex: 1,
    padding: '6px 8px',
    border: 'none',
    background: 'transparent',
    color: 'var(--text-secondary)',
    fontSize: '11px',
    fontWeight: 600,
    cursor: 'pointer',
  },
  segmentBtnActive: {
    background: 'var(--accent)',
    color: '#ffffff',
  },
  hint: {
    fontSize: '10px',
    lineHeight: 1.45,
    color: 'var(--text-muted)',
  },
  profileHeader: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: '8px',
  },
  profileList: {
    display: 'flex',
    flexDirection: 'column',
    gap: '6px',
  },
  profileItem: {
    display: 'grid',
    gridTemplateColumns: 'minmax(0, 1fr) auto 24px',
    alignItems: 'center',
    gap: '6px',
    padding: '6px',
    border: '1px solid var(--border)',
    borderRadius: '7px',
    background: 'rgba(255, 255, 255, 0.36)',
  },
  profileItemSelected: {
    borderColor: 'var(--accent)',
    boxShadow: '0 0 0 1px color-mix(in srgb, var(--accent) 22%, transparent)',
  },
  profileMain: {
    display: 'flex',
    minWidth: 0,
    flexDirection: 'column',
    alignItems: 'flex-start',
    gap: '2px',
    border: 'none',
    background: 'transparent',
    color: 'var(--text-primary)',
    padding: 0,
    cursor: 'pointer',
    textAlign: 'left',
  },
  profileName: {
    maxWidth: '100%',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
    fontSize: '11px',
    fontWeight: 700,
  },
  profileMeta: {
    maxWidth: '100%',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
    color: 'var(--text-muted)',
    fontSize: '9px',
    fontWeight: 600,
  },
  profileUseBtn: {
    border: '1px solid var(--accent)',
    borderRadius: '6px',
    background: 'var(--accent)',
    color: '#fff',
    fontSize: '10px',
    fontWeight: 700,
    padding: '4px 6px',
    cursor: 'pointer',
    whiteSpace: 'nowrap',
  },
  profileDeleteBtn: {
    width: '24px',
    height: '24px',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    border: '1px solid var(--border)',
    borderRadius: '6px',
    background: 'transparent',
    color: 'var(--text-muted)',
    cursor: 'pointer',
    padding: 0,
  },
  grid: {
    display: 'grid',
    gridTemplateColumns: '1fr 84px',
    gap: '8px',
  },
  field: {
    display: 'flex',
    flexDirection: 'column',
    gap: '4px',
    marginBottom: '8px',
  },
  input: {
    width: '100%',
    minWidth: 0,
    boxSizing: 'border-box',
    padding: '6px 7px',
    border: '1px solid var(--border)',
    borderRadius: '6px',
    background: 'var(--bg)',
    color: 'var(--text-primary)',
    fontSize: '11px',
    outline: 'none',
  },
  actions: {
    display: 'flex',
    gap: '6px',
    marginTop: '8px',
  },
  divider: {
    height: '1px',
    background: 'var(--border)',
    margin: '10px 0',
  },
  primaryBtn: {
    flex: 1,
    padding: '7px 0',
    border: '1px solid var(--accent)',
    borderRadius: '6px',
    background: 'var(--accent)',
    color: '#ffffff',
    fontSize: '11px',
    fontWeight: 700,
    cursor: 'pointer',
  },
  secondaryActionBtn: {
    width: '100%',
    padding: '7px 0',
    border: '1px solid var(--border)',
    borderRadius: '6px',
    background: 'transparent',
    color: 'var(--text-primary)',
    fontSize: '11px',
    fontWeight: 600,
    cursor: 'pointer',
  },
  successText: {
    marginTop: '8px',
    fontSize: '10px',
    color: 'var(--success)',
  },
  errorText: {
    marginTop: '8px',
    fontSize: '10px',
    color: 'var(--danger)',
  },
  backupPanel: {
    display: 'flex',
    flexDirection: 'column',
    gap: '8px',
  },
  backupHeader: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: '8px',
  },
  backupTitleGroup: {
    display: 'flex',
    alignItems: 'center',
    gap: '6px',
    minWidth: 0,
  },
  betaBadge: {
    padding: '1px 5px',
    borderRadius: '999px',
    background: 'rgba(245, 158, 11, 0.12)',
    color: '#b45309',
    fontSize: '9px',
    fontWeight: 700,
    whiteSpace: 'nowrap',
  },
  backupNotice: {
    color: 'var(--text-muted)',
    fontSize: '10px',
    lineHeight: 1.45,
  },
  backupRestoreRow: {
    display: 'flex',
    alignItems: 'center',
    gap: '6px',
  },
  backupVersionLine: {
    color: 'var(--text-muted)',
    fontSize: '10px',
    lineHeight: 1.4,
  },
  backupSelect: {
    flex: 1,
    minWidth: 0,
    padding: '6px 7px',
    border: '1px solid var(--border)',
    borderRadius: '6px',
    background: 'var(--bg)',
    color: 'var(--text-primary)',
    fontSize: '10px',
    outline: 'none',
  },
  miniTextBtn: {
    border: 'none',
    background: 'transparent',
    color: 'var(--accent)',
    fontSize: '10px',
    fontWeight: 600,
    cursor: 'pointer',
    padding: 0,
    whiteSpace: 'nowrap',
  },
  restoreBtn: {
    padding: '6px 8px',
    border: '1px solid var(--border)',
    borderRadius: '6px',
    background: 'transparent',
    color: 'var(--text-primary)',
    fontSize: '10px',
    fontWeight: 700,
    cursor: 'pointer',
    whiteSpace: 'nowrap',
    flexShrink: 0,
  },
  restoreBtnConfirm: {
    border: '1px solid #f59e0b',
    color: '#f59e0b',
    background: 'rgba(245, 158, 11, 0.08)',
  },
  backupMessage: {
    fontSize: '10px',
    color: 'var(--text-muted)',
    lineHeight: 1.4,
    wordBreak: 'break-word',
  },
  backupMessageError: {
    color: 'var(--danger)',
  },
  backupMessageSuccess: {
    color: 'var(--success)',
  },
};
