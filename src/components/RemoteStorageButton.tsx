import { useCallback, useEffect, useRef, useState } from 'react';
import {
  createBackup,
  getSetting,
  initializeRemoteStorage,
  listBackups,
  openBackupFolder,
  restoreBackup,
  setSetting,
  testRemoteStorage,
} from '../api/settings';
import type { BackupFileInfo } from '../api/settings';
import { useI18n } from '../i18n';

type StorageMode = 'local' | 'remote';
type ConnectionMode = 'url' | 'manual';

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
};

type StoredSettingsPayload = Record<string, string>;

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
  const [saveState, setSaveState] = useState<'idle' | 'saving' | 'saved' | 'failed'>('idle');
  const [testState, setTestState] = useState<'idle' | 'testing' | 'ready' | 'failed'>('idle');
  const [testMessage, setTestMessage] = useState('');
  const [backups, setBackups] = useState<BackupFileInfo[]>([]);
  const [selectedBackup, setSelectedBackup] = useState('');
  const [backupStatus, setBackupStatus] = useState<'idle' | 'working' | 'success' | 'failed'>('idle');
  const [backupMessage, setBackupMessage] = useState('');
  const [restoreConfirming, setRestoreConfirming] = useState(false);
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

  const loadSettings = useCallback(async () => {
    const [mode, connMode, savedUrl, savedHost, savedPort, savedDatabase, savedUsername, savedPassword, savedSslMode, ready] = await Promise.all([
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
    setStorageMode(loadedStorageMode);
    setConnectionMode(persistedPayload[SETTING_KEYS.connectionMode] === 'manual' ? 'manual' : 'url');
    setUrl(persistedPayload[SETTING_KEYS.url]);
    setHost(persistedPayload[SETTING_KEYS.host]);
    setPort(persistedPayload[SETTING_KEYS.port]);
    setDatabase(persistedPayload[SETTING_KEYS.database]);
    setUsername(persistedPayload[SETTING_KEYS.username]);
    setPassword(persistedPayload[SETTING_KEYS.password]);
    setSslMode(persistedPayload[SETTING_KEYS.sslMode]);
    setSaveState('idle');
    setTestState('idle');
    setTestMessage('');
  }, []);

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
    setBackupStatus('idle');
    setBackupMessage('');
    setRestoreConfirming(false);
  }, [open, loadSettings, refreshBackups, t.backupFailed]);

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
        setOpen(false);
        onStorageModeChange?.('local');
        return;
      }

      setTestState('testing');
      const pendingPayload = buildSettingsPayload(false);
      await writeSettings(pendingPayload);
      await testRemoteStorage();
      await initializeRemoteStorage();
      const readyPayload = { ...pendingPayload, [SETTING_KEYS.ready]: 'true' };
      await writeSettings(readyPayload);
      persistedSettingsRef.current = readyPayload;
      setActiveStorageMode('remote');
      setTestState('ready');
      setSaveState('saved');
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
  }, [buildSettingsPayload, getPayloadActiveMode, onStorageModeChange, storageMode, writeSettings]);

  const handleCreateBackup = useCallback(async () => {
    setBackupStatus('working');
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

    setBackupStatus('working');
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
    }
  }, [restoreConfirming, selectedBackup, t]);

  const handleOpenBackupFolder = useCallback(() => {
    openBackupFolder().catch((err) => {
      console.error('Failed to open backup folder:', err);
      setBackupStatus('failed');
      setBackupMessage(t.backupFailed);
    });
  }, [t.backupFailed]);

  return (
    <div style={styles.wrapper} ref={panelRef}>
      <button
        className="settings-gear-btn remote-storage-btn"
        style={{
          ...styles.iconBtn,
          ...(activeStorageMode === 'remote' ? styles.iconBtnActive : {}),
        }}
        onClick={(event) => {
          event.stopPropagation();
          setOpen((value) => !value);
        }}
        title={t.remoteStorage}
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <ellipse cx="12" cy="5" rx="8" ry="3" />
          <path d="M4 5v6c0 1.66 3.58 3 8 3s8-1.34 8-3V5" />
          <path d="M4 11v6c0 1.66 3.58 3 8 3s8-1.34 8-3v-6" />
        </svg>
      </button>

      {open && (
        <div style={styles.panel}>
          <div style={styles.panelTitle}>
            <span>{t.remoteStorage}</span>
            <span style={activeStorageMode === 'remote' ? styles.remoteBadge : styles.localBadge}>
              {activeStorageMode === 'remote' ? t.storageModeRemote : t.storageModeLocal}
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
              <div style={styles.notice}>{t.remoteStoragePending}</div>

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
            >
              {saveState === 'saving' || testState === 'testing'
                ? t.savingStorageConfig
                : storageMode === 'remote'
                ? t.saveAndUseRemote
                : t.saveAndUseLocal}
            </button>
          </div>

          {saveState === 'saved' && <div style={styles.successText}>{t.storageConfigSaved}</div>}
          {saveState === 'failed' && <div style={styles.errorText}>{t.storageConfigFailed}</div>}
          {testState === 'ready' && <div style={styles.successText}>{t.storageConnectionReady}: {testMessage}</div>}
          {testState === 'failed' && <div style={styles.errorText}>{t.storageConnectionFailed}: {testMessage}</div>}

          <div style={styles.divider} />

          <div style={styles.backupPanel} title={t.backupRestoreDesc}>
            <div style={styles.backupHeader}>
              <span style={styles.sectionTitle}>{t.backupRestore}</span>
              <button style={styles.miniTextBtn} onClick={handleOpenBackupFolder}>
                {t.openBackupFolder}
              </button>
            </div>
            <button
              style={styles.secondaryActionBtn}
              onClick={handleCreateBackup}
              disabled={backupStatus === 'working'}
            >
              {backupStatus === 'working' ? t.creatingBackup : t.createBackup}
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
                disabled={backups.length === 0}
              >
                {backups.length === 0 ? (
                  <option value="">{t.noBackups}</option>
                ) : (
                  backups.map((backup) => (
                    <option key={backup.fileName} value={backup.fileName}>
                      {formatBackupTime(backup.createdAt)} · {formatBackupSize(backup.sizeBytes)}
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
                disabled={!selectedBackup || backupStatus === 'working'}
              >
                {backupStatus === 'working' ? t.restoringBackup : t.restoreBackup}
              </button>
            </div>
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
        </div>
      )}
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  wrapper: {
    position: 'relative',
    zIndex: 105,
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
  iconBtnActive: {
    color: 'var(--accent)',
    background: 'var(--accent-bg)',
  },
  panel: {
    position: 'absolute',
    top: '36px',
    right: '0',
    width: '300px',
    maxHeight: 'calc(100vh - 52px)',
    overflowY: 'auto',
    background: 'var(--surface)',
    border: '1px solid var(--border)',
    borderRadius: '8px',
    padding: '10px',
    zIndex: 230,
    boxShadow: '0 12px 32px rgba(0, 0, 0, 0.16)',
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
  localBadge: {
    fontSize: '10px',
    color: 'var(--text-muted)',
    fontWeight: 600,
  },
  remoteBadge: {
    fontSize: '10px',
    color: 'var(--accent)',
    fontWeight: 700,
  },
  section: {
    display: 'flex',
    flexDirection: 'column',
    gap: '6px',
    marginBottom: '10px',
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
  notice: {
    padding: '7px 8px',
    marginBottom: '10px',
    border: '1px solid var(--border)',
    borderRadius: '6px',
    background: 'var(--bg)',
    color: 'var(--text-muted)',
    fontSize: '10px',
    lineHeight: 1.45,
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
  backupRestoreRow: {
    display: 'flex',
    alignItems: 'center',
    gap: '6px',
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
