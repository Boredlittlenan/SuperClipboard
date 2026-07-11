import { useCallback, useEffect, useRef, useState } from 'react';
import type React from 'react';
import { FlaskConical } from 'lucide-react';
import { useI18n } from '../i18n';
import { useAppSettings } from '../hooks/useAppSettings';
import { ToggleSettingRow } from './settings/SettingRow';

export default function ExperimentalFeaturesButton() {
  const { t } = useI18n();
  const { settings, setAppSetting } = useAppSettings();
  const {
    clipboardMultiTagEnabled,
    hideEntryColorStripEnabled,
    categoryTabSelectedColorsEnabled,
    modernUiEnabled,
  } = settings;
  const [open, setOpen] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);

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

  const handleClipboardMultiTagToggle = useCallback(async () => {
    await setAppSetting('clipboardMultiTagEnabled', !clipboardMultiTagEnabled);
  }, [clipboardMultiTagEnabled, setAppSetting]);

  const handleModernUiToggle = useCallback(async () => {
    await setAppSetting('modernUiEnabled', !modernUiEnabled);
  }, [modernUiEnabled, setAppSetting]);

  const handleHideEntryColorStripToggle = useCallback(async () => {
    await setAppSetting('hideEntryColorStripEnabled', !hideEntryColorStripEnabled);
  }, [hideEntryColorStripEnabled, setAppSetting]);

  const handleCategoryTabSelectedColorsToggle = useCallback(async () => {
    await setAppSetting('categoryTabSelectedColorsEnabled', !categoryTabSelectedColorsEnabled);
  }, [categoryTabSelectedColorsEnabled, setAppSetting]);

  return (
    <div style={styles.wrapper} ref={panelRef}>
      <button
        className="settings-gear-btn experimental-features-btn"
        style={styles.iconBtn}
        onClick={(event) => {
          event.stopPropagation();
          setOpen((value) => !value);
        }}
        title={t.experimentalFeatures}
      >
        <FlaskConical size={16} strokeWidth={2.15} />
      </button>

      {open && (
        <div className="glass-menu-panel" style={styles.panel}>
          <div style={styles.panelTitle}>{t.experimentalFeatures}</div>
          <ToggleSettingRow label={t.modernUi} title={t.modernUiDesc} checked={modernUiEnabled} onChange={handleModernUiToggle} />
          <ToggleSettingRow label={t.clipboardMultiTag} title={t.clipboardMultiTagDesc} checked={clipboardMultiTagEnabled} onChange={handleClipboardMultiTagToggle} />
          <ToggleSettingRow label={t.hideEntryColorStrip} title={t.hideEntryColorStripDesc} checked={hideEntryColorStripEnabled} onChange={handleHideEntryColorStripToggle} />
          <ToggleSettingRow label={t.categoryTabSelectedColors} title={t.categoryTabSelectedColorsDesc} checked={categoryTabSelectedColorsEnabled} onChange={handleCategoryTabSelectedColorsToggle} />
        </div>
      )}
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  wrapper: {
    position: 'relative',
    zIndex: 1220,
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
    width: '220px',
    maxHeight: 'calc(100vh - 48px)',
    overflowY: 'auto',
    background: 'var(--panel-glass)',
    border: '1px solid var(--apple-separator)',
    borderRadius: '12px',
    padding: '12px',
    zIndex: 1310,
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
};
