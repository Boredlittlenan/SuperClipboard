import { useCallback, useEffect, useRef, useState } from 'react';
import { Database, Network, Trash2 } from 'lucide-react';
import { getStorageStatus, type StorageStatusInfo } from '../api/settings';
import { useI18n } from '../i18n';
import { useClickOutside } from '../hooks/useClickOutside';
import {
  formatProfileTime, parseProfiles, payloadFromProfile, profileFromPayload, upsertProfile,
  REMOTE_SETTING_KEYS as SETTING_KEYS, type ConnectionMode, type RemoteDbProfile,
  type StoredSettingsPayload,
} from '../storage/remoteProfiles';
import { getClassicRemoteSettings, replaceClassicRemoteProfiles, switchWorkspace } from '../workspace/bridge';
import { StorageView } from '../workspace/settings/StorageView';
import ConfirmDialog, { type ConfirmDialogState } from './ConfirmDialog';
import FloatingMenuPanel from './FloatingMenuPanel';
import '../workspace/workspace.css';

type StorageMode = 'local' | 'remote';

function connectionUrl(profile: RemoteDbProfile) {
  const enc = encodeURIComponent;
  if (profile.connectionMode === 'manual') {
    const host = profile.host.includes(':') && !profile.host.startsWith('[') ? `[${profile.host}]` : profile.host;
    const authority = `${enc(profile.username)}:${enc(profile.password)}@${host}:${profile.port}`;
    return new URL(`postgresql://${authority}/${enc(profile.database)}?sslmode=${enc(profile.sslMode)}`).toString();
  }
  const url = new URL(profile.url);
  if (!url.searchParams.has('sslmode')) url.searchParams.set('sslmode', profile.sslMode);
  return url.toString();
}

export default function RemoteStorageButton({ onStorageModeChange, theme }: {
  onStorageModeChange: () => void; theme: 'light' | 'dark';
}) {
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
  const [testMessage, setTestMessage] = useState('');
  const [transferMessage, setTransferMessage] = useState<{ text: string; error: boolean }>();
  const [workspace, setWorkspace] = useState(0);
  const [profiles, setProfiles] = useState<RemoteDbProfile[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState('');
  const [deleteProfileTarget, setDeleteProfileTarget] = useState<RemoteDbProfile | null>(null);
  const anchorRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const switching = saveState === 'saving';

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
    const saved = await getClassicRemoteSettings();
    const loaded = parseProfiles(saved.profiles);
    setProfiles(loaded);
    const mode = saved.isRemote ? 'remote' : 'local';
    const activeProfile = loaded.find((profile) => profile.id === saved.activeProfile);
    applyPayloadToForm({
      ...(activeProfile ? payloadFromProfile(activeProfile, true) : { [SETTING_KEYS.url]: saved.url }),
      [SETTING_KEYS.storageMode]: mode,
    });
    setActiveStorageMode(mode);
    setSelectedProfileId(activeProfile?.id ?? '');
  }, [applyPayloadToForm]);

  useEffect(() => {
    void loadSettings().catch((error) => { setSaveState('failed'); setTestMessage(String(error)); });
  }, [loadSettings]);

  const refreshStorageStatus = () => {
    void getStorageStatus().then(setStorageStatus, (error) => {
      setSaveState('failed');
      setTestMessage(String(error));
    });
  };

  useClickOutside(anchorRef, open && !switching, () => setOpen(false), false, panelRef);

  const activate = async (profile?: RemoteDbProfile) => {
    if (switching) return;
    setSaveState('saving');
    setTestMessage('');
    setTransferMessage(undefined);
    try {
      if (profile) {
        const activated = { ...profile, lastUsedAt: new Date().toISOString() };
        const next = upsertProfile(profiles, activated);
        await switchWorkspace({ mode: 'remote', url: connectionUrl(activated) }, {
          profiles: JSON.stringify(next), activeId: activated.id,
        });
      } else {
        await switchWorkspace({ mode: 'local' });
      }
      await loadSettings();
      setWorkspace((value) => value + 1);
      refreshStorageStatus();
      setSaveState('saved');
      onStorageModeChange();
    } catch (error) {
      setSaveState('failed');
      setTestMessage(String(error));
    }
  };

  const handleSave = () => {
    if (storageMode === 'local') return void activate();
    const profile = profileFromPayload({
      [SETTING_KEYS.storageMode]: 'remote',
      [SETTING_KEYS.connectionMode]: connectionMode,
      [SETTING_KEYS.url]: url.trim(), [SETTING_KEYS.host]: host.trim(), [SETTING_KEYS.port]: port.trim() || '5432',
      [SETTING_KEYS.database]: database.trim(), [SETTING_KEYS.username]: username.trim(),
      [SETTING_KEYS.password]: password, [SETTING_KEYS.sslMode]: sslMode,
    });
    void activate(profile);
  };

  const handleSelectProfile = (profile: RemoteDbProfile) => {
    setSelectedProfileId(profile.id);
    applyPayloadToForm(payloadFromProfile(profile, false));
    setSaveState('idle');
    setTestMessage('');
  };

  const deleteProfile = async (id: string) => {
    const next = profiles.filter((profile) => profile.id !== id);
    try {
      await replaceClassicRemoteProfiles(JSON.stringify(next));
      setProfiles(next);
      if (selectedProfileId === id) setSelectedProfileId('');
    } catch (error) {
      setSaveState('failed');
      setTestMessage(String(error));
    }
  };
  const deleteProfileDialog: ConfirmDialogState | null = deleteProfileTarget ? {
    title: t.deleteRemoteProfile,
    message: t.deleteRemoteProfileConfirm(deleteProfileTarget.name),
    confirmLabel: t.delete,
    tone: 'danger',
    resolve: (confirmed) => {
      if (confirmed) void deleteProfile(deleteProfileTarget.id);
      setDeleteProfileTarget(null);
    },
  } : null;

  const headerStorageMode: StorageMode = storageStatus?.mode === 'remote' || activeStorageMode === 'remote'
    ? 'remote'
    : 'local';
  const headerStorageHealth = storageStatus?.health ?? (headerStorageMode === 'remote' ? 'notReady' : 'local');
  const headerStorageLabel = headerStorageMode === 'remote'
    ? t.storageStatusConnected
    : t.storageStatusLocal;

  return (
    <div style={styles.wrapper} ref={anchorRef}>
      <button
        className="settings-gear-btn remote-storage-btn"
        style={styles.iconBtn}
        onClick={(event) => {
          event.stopPropagation();
          if (!open) refreshStorageStatus();
          setOpen(!open);
        }}
        title={t.remoteStorage}
        aria-label={t.remoteStorage}
        aria-expanded={open}
        disabled={switching}
      >
        {activeStorageMode === 'remote'
          ? <Network size={16} strokeWidth={2.15} />
          : <Database size={16} strokeWidth={2.15} />}
      </button>

      {open && (
        <FloatingMenuPanel anchorRef={anchorRef} panelRef={panelRef} style={styles.panel}>
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
                      type="password"
                      autoComplete="off"
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
              disabled={switching}
              onClick={handleSave}
              title={storageMode === 'remote' ? t.remoteStoragePending : undefined}
            >
              {switching
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
                          disabled={switching}
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
                          onClick={() => void activate(profile)}
                          disabled={switching}
                        >
                          {t.useRemoteProfile}
                        </button>
                        <button
                          type="button"
                          style={styles.profileDeleteBtn}
                          onClick={() => setDeleteProfileTarget(profile)}
                          disabled={switching}
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

          {saveState === 'saved' && <div role="status" style={styles.successText}>{t.storageConfigSaved}</div>}
          {saveState === 'failed' && <div role="alert" style={styles.errorText}>{t.storageConfigFailed} {testMessage}</div>}

          <div style={styles.divider}/>
          <div className="classic-feature" data-theme={theme} inert={switching}>
            {transferMessage && <p role={transferMessage.error ? 'alert' : 'status'}
              className={transferMessage.error ? 'inline-error' : 'import-result'}>{transferMessage.text}</p>}
            <StorageView key={workspace} workspace={workspace} modeOverride={activeStorageMode}
              onImported={onStorageModeChange} onSwitching={() => {}}
              onNotice={(text) => setTransferMessage({ text, error: false })}
              onError={(text) => setTransferMessage({ text, error: true })}/>
          </div>
        </FloatingMenuPanel>
      )}

      {deleteProfileDialog && (
        <ConfirmDialog
          dialog={deleteProfileDialog}
          onClose={() => setDeleteProfileTarget(null)}
        />
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
    border: '1px solid var(--apple-separator)',
    borderRadius: '12px',
    padding: '12px',
    zIndex: 1320,
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
    fontSize: '12px',
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
    fontSize: '12px',
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
    fontSize: '12px',
    fontWeight: 600,
    cursor: 'pointer',
  },
  segmentBtnActive: {
    background: 'var(--accent)',
    color: '#ffffff',
  },
  hint: {
    fontSize: '12px',
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
    borderWidth: '1px',
    borderStyle: 'solid',
    borderColor: 'var(--border)',
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
    fontSize: '12px',
    fontWeight: 700,
  },
  profileMeta: {
    maxWidth: '100%',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
    color: 'var(--text-muted)',
    fontSize: '12px',
    fontWeight: 600,
  },
  profileUseBtn: {
    border: '1px solid var(--accent)',
    borderRadius: '6px',
    background: 'var(--accent)',
    color: '#fff',
    fontSize: '12px',
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
    fontSize: '12px',
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
    fontSize: '12px',
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
    fontSize: '12px',
    fontWeight: 600,
    cursor: 'pointer',
  },
  successText: {
    marginTop: '8px',
    fontSize: '12px',
    color: 'var(--success)',
  },
  errorText: {
    marginTop: '8px',
    fontSize: '12px',
    color: 'var(--danger)',
  },

};
