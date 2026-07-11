import { useState, useRef, useEffect, useCallback } from 'react';
import { SlidersHorizontal } from 'lucide-react';
import { useI18n } from '../i18n';
import type { Locale } from '../i18n/translations';
import {
  getAutostartEnabled,
  setAutostartEnabled,
  getShortcut,
  setShortcut,
  setShortcutRecording,
  checkUpdate,
  openUrl,
  setSetting,
  setAlwaysOnTop,
} from '../api/settings';
import type { UpdateInfo } from '../api/settings';
import type { ThemeMode } from '../types';
import { listen } from '@tauri-apps/api/event';
import { getVersion } from '@tauri-apps/api/app';
import { formatShortcutLabel } from '../utils';
import { useAppSettings } from '../hooks/useAppSettings';
import { SettingRow, ToggleSettingRow } from './settings/SettingRow';

const LANGUAGES: { value: Locale; labelKey: 'langZhCN' | 'langEn' }[] = [
  { value: 'zh-CN', labelKey: 'langZhCN' },
  { value: 'en', labelKey: 'langEn' },
];

/** Convert a JS KeyboardEvent to a Tauri shortcut token */
function keyToTauri(key: string, code: string): string {
  if (/^Key[A-Z]$/.test(code) || /^Digit[0-9]$/.test(code) || /^Numpad/.test(code)) {
    return code;
  }

  switch (key) {
    case 'Control': return 'Ctrl';
    case 'Meta': return 'Super';
    case 'OS': return 'Super';
    case 'Win': return 'Super';
    case 'Windows': return 'Super';
    case 'Shift': return 'Shift';
    case 'Alt': return 'Alt';
    case ' ': return 'Space';
    default:
      if (key.length === 1) return key.toUpperCase();
      // F-keys, arrows, etc.
      return key;
  }
}

function isModifierKey(key: string): boolean {
  return key === 'Control' || key === 'Shift' || key === 'Alt' || key === 'Meta' || key === 'OS';
}

function addEventModifiers(event: KeyboardEvent, modifiers: Set<string>) {
  if (event.ctrlKey || event.key === 'Control') modifiers.add('Control');
  if (event.shiftKey || event.key === 'Shift') modifiers.add('Shift');
  if (event.altKey || event.key === 'Alt') modifiers.add('Alt');
  if (event.metaKey || event.key === 'Meta' || event.key === 'OS') modifiers.add('Meta');
}

function shortcutParts(modifiers: Set<string>, mainKey: string): string[] {
  const parts: string[] = [];
  if (modifiers.has('Control')) parts.push('Ctrl');
  if (modifiers.has('Meta')) parts.push('Super');
  if (modifiers.has('Alt')) parts.push('Alt');
  if (modifiers.has('Shift')) parts.push('Shift');
  parts.push(mainKey);
  return parts;
}

interface SettingsButtonProps {
  onShortcutChange?: (shortcut: string) => void;
  onVersionTitleTrigger?: (clickCount: number) => void;
}

export default function SettingsButton({
  onShortcutChange,
  onVersionTitleTrigger,
}: SettingsButtonProps) {
  const { t, locale, setLocale } = useI18n();
  const { settings, setAppSetting } = useAppSettings();
  const {
    memoEnabled,
    memoColor,
    alwaysOnTop,
    rawPreview,
    themeMode,
    themeAccent,
    autoUpdate,
    experimentalFeaturesEnabled,
    categoryTabSortingEnabled,
    archiveEnabled,
  } = settings;
  const [open, setOpen] = useState(false);
  const [autostart, setAutostart] = useState(false);
  const [appVersion, setAppVersion] = useState('');
  const [showColorPicker, setShowColorPicker] = useState(false);
  const [hexInput, setHexInput] = useState('');
  const [shortcut, setShortcutState] = useState('Alt+X');
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState('');
  const [updateStatus, setUpdateStatus] = useState<'idle' | 'checking' | 'upToDate' | 'hasUpdate' | 'failed'>('idle');
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const recorderRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const colorRef = useRef<HTMLDivElement>(null);
  const keysRef = useRef<{ modifiers: Set<string>; mainKey: string | null }>({
    modifiers: new Set(),
    mainKey: null,
  });
  const savingShortcutRef = useRef(false);

  const handleShortcutButtonClick = useCallback(async () => {
    if (recording) {
      setShortcutRecording(false).catch(console.error);
      setRecording(false);
      setError('');
      return;
    }

    try {
      keysRef.current = { modifiers: new Set(), mainKey: null };
      await setShortcutRecording(true);
      setError('');
      setRecording(true);
    } catch (err) {
      setError(String(err));
    }
  }, [recording]);

  const MEMO_PRESETS = [
    { color: '#ec5f9e', title: '樱花粉' },
    { color: '#2563eb', title: '少年蓝' },
    { color: '#8b5cf6', title: '友情紫' },
    { color: '#10b981' },
    { color: '#f59e0b' },
    { color: '#ef4444' },
    { color: '#14b8a6' },
    { color: '#6366f1' },
  ];

  // Listen for "open-settings" event from tray menu
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen('open-settings', () => {
      setOpen(true);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  // Load autostart state and shortcut when panel opens
  useEffect(() => {
    if (open) {
      getAutostartEnabled().then(setAutostart).catch(console.error);
      getShortcut().then((s) => {
        setShortcutState(s);
        onShortcutChange?.(s);
      }).catch(console.error);
      setHexInput(memoColor || '');
      setShowColorPicker(false);
      getVersion().then(setAppVersion).catch(console.error);
    }
  }, [memoColor, onShortcutChange, open]);

  // Close panel when clicking outside
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
        setOpen(false);
        setRecording(false);
        setError('');
        setUpdateStatus('idle');
        setShowColorPicker(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

  // Close color picker when clicking outside it (but inside the panel)
  useEffect(() => {
    if (!showColorPicker) return;
    const handler = (e: MouseEvent) => {
      if (colorRef.current && !colorRef.current.contains(e.target as Node)) {
        setShowColorPicker(false);
      }
    };
    // Delay to avoid the same click that opens it
    const timer = setTimeout(() => document.addEventListener('mousedown', handler), 0);
    return () => { clearTimeout(timer); document.removeEventListener('mousedown', handler); };
  }, [showColorPicker]);

  // Keyboard capture for shortcut recording
  useEffect(() => {
    if (!recording) return;

    keysRef.current = { modifiers: new Set(), mainKey: null };

    const restoreShortcut = () => {
      setShortcutRecording(false).catch(console.error);
    };

    const saveShortcut = async (combo: string) => {
      if (savingShortcutRef.current) return;
      savingShortcutRef.current = true;
      try {
        const saved = await setShortcut(combo);
        setShortcutState(saved);
        onShortcutChange?.(saved);
        setError('');
      } catch (err) {
        setError(String(err));
      } finally {
        savingShortcutRef.current = false;
        setRecording(false);
      }
    };

    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (e.key === 'Escape') {
        setRecording(false);
        setError('');
        return;
      }

      addEventModifiers(e, keysRef.current.modifiers);
      if (isModifierKey(e.key)) {
        keysRef.current.modifiers.add(e.key);
      } else {
        const mainKey = keyToTauri(e.key, e.code);
        keysRef.current.mainKey = mainKey;
        const { modifiers } = keysRef.current;
        if (modifiers.size === 0) {
          setError(t.shortcutInvalid);
          setRecording(false);
          return;
        }
        void saveShortcut(shortcutParts(modifiers, mainKey).join('+'));
      }
    };

    const onKeyUp = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
    };

    window.addEventListener('keydown', onKeyDown, true);
    window.addEventListener('keyup', onKeyUp, true);
    return () => {
      window.removeEventListener('keydown', onKeyDown, true);
      window.removeEventListener('keyup', onKeyUp, true);
      restoreShortcut();
    };
  }, [recording, t, onShortcutChange]);

  const handleAutostartToggle = useCallback(async () => {
    try {
      const newValue = !autostart;
      await setAutostartEnabled(newValue);
      await setSetting('autostart', newValue ? 'true' : 'false');
      setAutostart(newValue);
    } catch (err) {
      console.error('Failed to toggle autostart:', err);
    }
  }, [autostart]);

  const handleMemoToggle = useCallback(async () => {
    await setAppSetting('memoEnabled', !memoEnabled);
  }, [memoEnabled, setAppSetting]);

  const handleMemoColorChange = useCallback(async (color: string) => {
    if (!/^#[0-9a-fA-F]{6}$/.test(color)) return;
    await setAppSetting('memoColor', color);
    setHexInput(color);
  }, [setAppSetting]);

  const handleMemoColorReset = useCallback(async () => {
    await setAppSetting('memoColor', null);
    setHexInput('');
  }, [setAppSetting]);

  const handleAlwaysOnTopToggle = useCallback(async () => {
    const newValue = !alwaysOnTop;
    await setAlwaysOnTop(newValue);
    await setAppSetting('alwaysOnTop', newValue);
  }, [alwaysOnTop, setAppSetting]);

  const handleRawPreviewToggle = useCallback(async () => {
    await setAppSetting('rawPreview', !rawPreview);
  }, [rawPreview, setAppSetting]);

  const handleThemeModeChange = useCallback(async (mode: ThemeMode) => {
    await setAppSetting('themeMode', mode);
  }, [setAppSetting]);

  const handleThemeAccentChange = useCallback(async (accent: string) => {
    await setAppSetting('themeAccent', accent === 'sakura' ? 'sakura' : 'default');
  }, [setAppSetting]);

  const handleAutoUpdateToggle = useCallback(async () => {
    await setAppSetting('autoUpdate', !autoUpdate);
  }, [autoUpdate, setAppSetting]);

  const handleExperimentalFeaturesToggle = useCallback(async () => {
    await setAppSetting('experimentalFeaturesEnabled', !experimentalFeaturesEnabled);
  }, [experimentalFeaturesEnabled, setAppSetting]);

  const handleArchiveToggle = useCallback(async () => {
    await setAppSetting('archiveEnabled', !archiveEnabled);
  }, [archiveEnabled, setAppSetting]);

  const handleCategoryTabSortingToggle = useCallback(async () => {
    await setAppSetting('categoryTabSortingEnabled', !categoryTabSortingEnabled);
  }, [categoryTabSortingEnabled, setAppSetting]);

  const handleCheckUpdate = useCallback(async () => {
    setUpdateStatus('checking');
    try {
      const info = await checkUpdate();
      setUpdateInfo(info);
      setUpdateStatus(info.hasUpdate ? 'hasUpdate' : 'upToDate');
    } catch (err) {
      console.error('Failed to check update:', err);
      setUpdateStatus('failed');
    }
  }, []);

  const handleDownload = useCallback(() => {
    if (updateInfo?.downloadUrl) {
      openUrl(updateInfo.downloadUrl);
    }
  }, [updateInfo]);

  return (
    <div style={styles.wrapper} ref={panelRef}>
      {/* Gear button */}
      <button
        className="settings-gear-btn"
        style={styles.gearBtn}
        onClick={(e) => {
          e.stopPropagation();
          setOpen((prev) => !prev);
        }}
        title={t.settings}
      >
        <SlidersHorizontal size={16} strokeWidth={2.15} />
      </button>

      {/* Settings dropdown panel */}
      {open && (
        <div className="glass-menu-panel" style={{ ...styles.panel, width: '260px' }}>
          {/* Title row with version */}
          <div style={styles.panelTitle}>
            <span>{t.settings}</span>
            {appVersion && (
              <span
                style={styles.versionBadge}
                onClick={(event) => {
                  event.stopPropagation();
                  // Undocumented title easter egg hook; App owns the title state.
                  onVersionTitleTrigger?.(event.detail);
                }}
              >
                v{appVersion}
              </span>
            )}
          </div>

          {/* Language section */}
          <div style={styles.section}>
            <div style={styles.langOptions}>
              {LANGUAGES.map(({ value, labelKey }) => (
                <button
                  key={value}
                  style={{
                    ...styles.langBtn,
                    ...(locale === value ? styles.langBtnActive : {}),
                  }}
                  onClick={() => setLocale(value)}
                >
                  {t[labelKey]}
                </button>
              ))}
            </div>
          </div>

          {/* System Settings header */}
          <div style={styles.sectionHeader}>{t.systemSettings}</div>

          {/* Shortcut section */}
          <SettingRow label={t.shortcut} title={t.shortcutDesc}>
            <button
              ref={recorderRef}
              style={{
                ...styles.shortcutBtn,
                ...(recording ? styles.shortcutBtnRecording : {}),
                ...(error ? styles.shortcutBtnError : {}),
              }}
              onClick={handleShortcutButtonClick}
            >
              {recording ? t.shortcutRecording : formatShortcutLabel(shortcut)}
            </button>
          </SettingRow>
          {error && <span style={styles.errorText}>{error}</span>}

          {/* Theme mode */}
          <SettingRow label={t.themeMode} title={t.themeModeDesc}>
            <div style={styles.themeSegmented}>
              <button
                style={{
                  ...styles.themeSegBtn,
                  ...(themeMode === 'light' ? styles.themeSegBtnActive : {}),
                }}
                onClick={() => handleThemeModeChange('light')}
              >
                {t.themeLight}
              </button>
              <button
                style={{
                  ...styles.themeSegBtn,
                  ...(themeMode === 'dark' ? styles.themeSegBtnActive : {}),
                }}
                onClick={() => handleThemeModeChange('dark')}
              >
                {t.themeDark}
              </button>
              <button
                style={{
                  ...styles.themeSegBtn,
                  ...(themeMode === 'system' ? styles.themeSegBtnActive : {}),
                }}
                onClick={() => handleThemeModeChange('system')}
              >
                {t.themeSystem}
              </button>
            </div>
          </SettingRow>

          {/* Theme accent */}
          <SettingRow label={t.themeColor} title={t.themeColorDesc}>
            <div style={styles.colorOptions}>
              <button
                style={{
                  ...styles.colorBtn,
                  ...(themeAccent === 'default' ? styles.colorBtnActive : {}),
                }}
                onClick={() => handleThemeAccentChange('default')}
                title={t.themeDefault}
              >
                <span style={{ ...styles.colorSwatch, background: '#2563eb' }} />
                <span>{t.themeDefault}</span>
              </button>
              <button
                style={{
                  ...styles.colorBtn,
                  ...(themeAccent === 'sakura' ? styles.colorBtnActive : {}),
                }}
                onClick={() => handleThemeAccentChange('sakura')}
                title={t.themeSakura}
              >
                <span style={{ ...styles.colorSwatch, background: '#ec5f9e' }} />
                <span>{t.themeSakura}</span>
              </button>
            </div>
          </SettingRow>

          {/* Autostart */}
          <ToggleSettingRow label={t.autostart} title={t.autostartDesc} checked={autostart} onChange={handleAutostartToggle} />

          {/* Always on top */}
          <ToggleSettingRow label={t.alwaysOnTop} title={t.alwaysOnTopDesc} checked={alwaysOnTop} onChange={handleAlwaysOnTopToggle} />

          {/* Raw preview */}
          <ToggleSettingRow label={t.rawPreview} title={t.rawPreviewDesc} checked={rawPreview} onChange={handleRawPreviewToggle} />

          {/* Auto update */}
          <ToggleSettingRow label={t.autoUpdate} title={t.autoUpdateDesc} checked={autoUpdate} onChange={handleAutoUpdateToggle} />

          {/* Experimental features */}
          <ToggleSettingRow label={t.experimentalFeatures} title={t.experimentalFeaturesDesc} checked={experimentalFeaturesEnabled} onChange={handleExperimentalFeaturesToggle} />

          {/* Feature Settings header */}
          <div style={styles.sectionHeader}>{t.featureSettings}</div>

          {/* Category tab sorting */}
          <ToggleSettingRow label={t.categoryTabSorting} title={t.categoryTabSortingDesc} checked={categoryTabSortingEnabled} onChange={handleCategoryTabSortingToggle} />

          {/* Memo */}
          <ToggleSettingRow label={t.memoSetting} title={t.memoSettingDesc} checked={memoEnabled} onChange={handleMemoToggle} />

          {/* Memo color picker (only when memo is enabled) */}
          {memoEnabled && (
            <SettingRow label={t.memoColor} title={t.memoColorDesc}>
              <div ref={colorRef} style={{ position: 'relative' }}>
                <button
                  style={styles.memoColorBtn}
                  onClick={(e) => { e.stopPropagation(); setShowColorPicker(!showColorPicker); }}
                >
                  <span style={{
                    ...styles.memoColorSwatch,
                    background: memoColor || 'var(--memo-contrast)',
                  }} />
                  {memoColor && <span style={styles.memoColorResetMini} onClick={(e) => { e.stopPropagation(); handleMemoColorReset(); }}>{'\u2715'}</span>}
                </button>
                {showColorPicker && (
                  <div style={styles.colorPicker} onClick={(e) => e.stopPropagation()}>
                    <div style={styles.colorGrid}>
                      {MEMO_PRESETS.map((preset) => (
                        <button
                          key={preset.color}
                          style={{
                            ...styles.colorPreset,
                            background: preset.color,
                            ...(memoColor === preset.color ? styles.colorPresetActive : {}),
                          }}
                          title={preset.title}
                          onClick={() => handleMemoColorChange(preset.color)}
                        />
                      ))}
                    </div>
                    <div style={styles.colorInputRow}>
                      <span style={styles.colorHash}>#</span>
                      <input
                        style={styles.colorHexInput}
                        value={hexInput.replace('#', '')}
                        onChange={(e) => {
                          const val = e.target.value.replace('#', '').slice(0, 6);
                          setHexInput(val);
                          if (/^[0-9a-fA-F]{6}$/.test(val)) {
                            handleMemoColorChange('#' + val);
                          }
                        }}
                        placeholder="ec5f9e"
                        maxLength={6}
                      />
                      <button style={styles.colorResetBtn} onClick={handleMemoColorReset}>
                        {t.memoColorReset}
                      </button>
                    </div>
                  </div>
                )}
              </div>
            </SettingRow>
          )}

          {/* Archive */}
          <ToggleSettingRow label={t.archiveSetting} title={t.archiveSettingDesc} checked={archiveEnabled} onChange={handleArchiveToggle} />

          {/* Divider */}
          <div style={styles.divider} />

          {/* Check for updates */}
          <div style={styles.section}>
            {updateStatus === 'idle' && (
              <button style={styles.updateBtn} onClick={handleCheckUpdate}>
                {t.checkUpdate}
              </button>
            )}
            {updateStatus === 'checking' && (
              <button style={{ ...styles.updateBtn, ...styles.updateBtnDisabled }} disabled>
                <span style={styles.spinner} />
                {t.checking}
              </button>
            )}
            {updateStatus === 'upToDate' && (
              <div style={styles.updateResultStack}>
                <div style={styles.updateResultHeader}>
                  <span style={styles.updateOkIcon}>&#10003;</span>
                  <span style={styles.updateOkText}>{t.upToDate}</span>
                </div>
                {updateInfo && (
                  <div style={styles.updateMeta}>
                    {t.updateCurrent(updateInfo.currentVersion)} · {t.updateLatest(updateInfo.latestVersion)}
                  </div>
                )}
              </div>
            )}
            {updateStatus === 'hasUpdate' && updateInfo && (
              <div style={styles.updateResultStack}>
                <div style={styles.updateResultHeader}>
                  <span style={styles.updateNewText}>{t.hasUpdate(updateInfo.latestVersion)}</span>
                  <button style={styles.updateDownloadBtn} onClick={handleDownload}>
                    {t.downloadUpdate}
                  </button>
                </div>
                <div style={styles.updateMeta}>
                  {t.updateCurrent(updateInfo.currentVersion)} · {updateInfo.releaseName || t.updateLatest(updateInfo.latestVersion)}
                </div>
                <div style={styles.releaseNotesBox}>
                  <div style={styles.releaseNotesTitle}>{t.releaseNotes}</div>
                  <pre style={styles.releaseNotesText}>
                    {updateInfo.releaseNotes || t.noReleaseNotes}
                  </pre>
                </div>
              </div>
            )}
            {updateStatus === 'failed' && (
              <div style={styles.updateResult}>
                <span style={styles.errorText}>{t.updateFailed}</span>
                <button style={styles.updateRetryBtn} onClick={handleCheckUpdate}>
                  {t.checkUpdate}
                </button>
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
    zIndex: 1200,
  },
  gearBtn: {
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
    width: '220px',
    maxHeight: 'calc(100vh - 48px)',
    overflowY: 'auto',
    background: 'var(--panel-glass)',
    border: '1px solid var(--apple-separator)',
    borderRadius: '12px',
    padding: '12px',
    zIndex: 1300,
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
  versionBadge: {
    fontSize: '10px',
    fontWeight: 500,
    color: 'var(--text-muted)',
    fontFamily: 'monospace',
    cursor: 'default',
    userSelect: 'none',
  },
  section: {
    display: 'flex',
    flexDirection: 'column',
    gap: '4px',
  },
  divider: {
    height: '1px',
    background: 'var(--border)',
    margin: '6px 0',
  },
  sectionHeader: {
    fontSize: '10px',
    fontWeight: 600,
    color: 'var(--text-muted)',
    textTransform: 'uppercase' as const,
    letterSpacing: '0.5px',
    marginTop: '6px',
    marginBottom: '2px',
    paddingBottom: '4px',
    borderBottom: '1px solid var(--border)',
  },
  langOptions: {
    display: 'flex',
    gap: '4px',
  },
  langBtn: {
    flex: 1,
    padding: '5px 0',
    border: '1px solid var(--border)',
    borderRadius: '6px',
    background: 'transparent',
    color: 'var(--text-secondary)',
    fontSize: '11px',
    fontWeight: 500,
    cursor: 'pointer',
    transition: 'all 0.15s',
  },
  langBtnActive: {
    background: 'var(--accent)',
    border: '1px solid var(--accent)',
    color: '#ffffff',
  },
  themeSegmented: {
    display: 'flex',
    border: '1px solid var(--border)',
    borderRadius: '6px',
    overflow: 'hidden',
    flexShrink: 0,
  },
  themeSegBtn: {
    flex: 1,
    padding: '4px 8px',
    border: 'none',
    background: 'transparent',
    color: 'var(--text-secondary)',
    fontSize: '10px',
    fontWeight: 500,
    cursor: 'pointer',
    transition: 'all 0.15s',
    whiteSpace: 'nowrap',
  },
  themeSegBtnActive: {
    background: 'var(--accent)',
    color: '#ffffff',
  },
  colorOptions: {
    display: 'flex',
    gap: '4px',
    flexShrink: 0,
  },
  colorBtn: {
    display: 'flex',
    alignItems: 'center',
    gap: '4px',
    padding: '3px 6px',
    border: '1px solid var(--border)',
    borderRadius: '6px',
    background: 'transparent',
    color: 'var(--text-secondary)',
    fontSize: '10px',
    fontWeight: 500,
    cursor: 'pointer',
    transition: 'all 0.15s',
    whiteSpace: 'nowrap',
  },
  colorBtnActive: {
    border: '1px solid var(--accent)',
    color: 'var(--accent)',
    background: 'var(--accent-bg)',
  },
  colorSwatch: {
    width: '9px',
    height: '9px',
    borderRadius: '50%',
    flexShrink: 0,
  },
  shortcutBtn: {
    padding: '3px 8px',
    border: '1px solid var(--border)',
    borderRadius: '6px',
    background: 'transparent',
    color: 'var(--text-primary)',
    fontSize: '11px',
    fontWeight: 600,
    fontFamily: 'monospace',
    cursor: 'pointer',
    transition: 'all 0.15s',
    whiteSpace: 'nowrap',
    flexShrink: 0,
  },
  shortcutBtnRecording: {
    border: '1px solid var(--accent)',
    color: 'var(--accent)',
    animation: 'pulse 1s infinite',
  },
  shortcutBtnError: {
    border: '1px solid #e74c3c',
    color: '#e74c3c',
  },
  errorText: {
    fontSize: '10px',
    color: '#e74c3c',
    marginTop: '2px',
  },
  updateBtn: {
    width: '100%',
    padding: '6px 0',
    border: '1px solid var(--border)',
    borderRadius: '6px',
    background: 'transparent',
    color: 'var(--text-primary)',
    fontSize: '11px',
    fontWeight: 500,
    cursor: 'pointer',
    transition: 'all 0.15s',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    gap: '6px',
  },
  updateBtnDisabled: {
    opacity: 0.6,
    cursor: 'default',
  },
  spinner: {
    display: 'inline-block',
    width: '12px',
    height: '12px',
    border: '2px solid var(--border)',
    borderTopColor: 'var(--accent)',
    borderRadius: '50%',
    animation: 'spin 0.6s linear infinite',
  },
  updateResult: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: '8px',
  },
  updateResultStack: {
    display: 'flex',
    flexDirection: 'column',
    gap: '6px',
  },
  updateResultHeader: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: '8px',
  },
  updateMeta: {
    fontSize: '10px',
    color: 'var(--text-muted)',
    lineHeight: 1.35,
  },
  releaseNotesBox: {
    padding: '6px',
    border: '1px solid var(--border)',
    borderRadius: '6px',
    background: 'var(--bg)',
  },
  releaseNotesTitle: {
    fontSize: '10px',
    fontWeight: 600,
    color: 'var(--text-secondary)',
    marginBottom: '4px',
  },
  releaseNotesText: {
    margin: 0,
    maxHeight: '96px',
    overflowY: 'auto',
    whiteSpace: 'pre-wrap',
    wordBreak: 'break-word',
    fontFamily: 'inherit',
    fontSize: '10px',
    lineHeight: 1.45,
    color: 'var(--text-secondary)',
  },
  updateOkIcon: {
    color: 'var(--success)',
    fontSize: '14px',
    fontWeight: 700,
    flexShrink: 0,
  },
  updateOkText: {
    fontSize: '11px',
    color: 'var(--success)',
    flex: 1,
  },
  updateNewText: {
    fontSize: '11px',
    color: 'var(--accent)',
    fontWeight: 500,
    flex: 1,
  },
  updateDownloadBtn: {
    padding: '4px 12px',
    border: 'none',
    borderRadius: '6px',
    background: 'var(--accent)',
    color: '#ffffff',
    fontSize: '11px',
    fontWeight: 600,
    cursor: 'pointer',
    transition: 'opacity 0.15s',
    flexShrink: 0,
  },
  updateRetryBtn: {
    padding: '4px 10px',
    border: '1px solid var(--border)',
    borderRadius: '6px',
    background: 'transparent',
    color: 'var(--text-secondary)',
    fontSize: '11px',
    fontWeight: 500,
    cursor: 'pointer',
    flexShrink: 0,
  },
  memoColorBtn: {
    display: 'flex',
    alignItems: 'center',
    gap: '2px',
    padding: '2px',
    border: '1px solid var(--border)',
    borderRadius: '6px',
    background: 'transparent',
    cursor: 'pointer',
    transition: 'all 0.15s',
    position: 'relative',
  },
  memoColorSwatch: {
    width: '18px',
    height: '18px',
    borderRadius: '4px',
    flexShrink: 0,
  },
  memoColorResetMini: {
    fontSize: '9px',
    color: 'var(--text-muted)',
    lineHeight: 1,
    padding: '0 2px',
  },
  colorPicker: {
    position: 'absolute',
    top: '28px',
    right: '0',
    background: 'var(--surface)',
    border: '1px solid var(--border)',
    borderRadius: '8px',
    padding: '10px',
    zIndex: 300,
    width: '180px',
  },
  colorGrid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(4, 1fr)',
    gap: '6px',
    marginBottom: '8px',
  },
  colorPreset: {
    width: '100%',
    aspectRatio: '1',
    border: '2px solid transparent',
    borderRadius: '6px',
    cursor: 'pointer',
    transition: 'all 0.15s',
    padding: 0,
  },
  colorPresetActive: {
    border: '2px solid var(--text-primary)',
  },
  colorInputRow: {
    display: 'flex',
    alignItems: 'center',
    gap: '4px',
  },
  colorHash: {
    fontSize: '11px',
    color: 'var(--text-muted)',
    fontFamily: 'monospace',
  },
  colorHexInput: {
    flex: 1,
    padding: '3px 4px',
    border: '1px solid var(--border)',
    borderRadius: '4px',
    background: 'var(--bg)',
    color: 'var(--text-primary)',
    fontSize: '11px',
    fontFamily: 'monospace',
    outline: 'none',
    minWidth: 0,
  },
  colorResetBtn: {
    padding: '3px 8px',
    border: '1px solid var(--border)',
    borderRadius: '4px',
    background: 'transparent',
    color: 'var(--text-secondary)',
    fontSize: '10px',
    cursor: 'pointer',
    whiteSpace: 'nowrap',
    flexShrink: 0,
  },
};
