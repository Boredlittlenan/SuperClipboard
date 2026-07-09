import { useCallback, useEffect, useRef, useState } from 'react';
import type React from 'react';
import { FlaskConical } from 'lucide-react';
import { getSetting, setSetting } from '../api/settings';
import { useI18n } from '../i18n';

const SETTING_KEYS = {
  clipboardMultiTag: 'clipboard_multi_tag_enabled',
  hideEntryColorStrip: 'hide_entry_color_strip_enabled',
  modernUi: 'modern_ui_enabled',
};

interface ExperimentalFeaturesButtonProps {
  onClipboardMultiTagChange?: (enabled: boolean) => void;
  onHideEntryColorStripChange?: (enabled: boolean) => void;
  onModernUiChange?: (enabled: boolean) => void;
}

export default function ExperimentalFeaturesButton({ onClipboardMultiTagChange, onHideEntryColorStripChange, onModernUiChange }: ExperimentalFeaturesButtonProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [clipboardMultiTagEnabled, setClipboardMultiTagEnabled] = useState(false);
  const [hideEntryColorStripEnabled, setHideEntryColorStripEnabled] = useState(false);
  const [modernUiEnabled, setModernUiEnabled] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    Promise.all([
      getSetting(SETTING_KEYS.clipboardMultiTag),
      getSetting(SETTING_KEYS.hideEntryColorStrip),
      getSetting(SETTING_KEYS.modernUi),
    ])
      .then(([multiTagValue, hideStripValue, modernUiValue]) => {
        const multiTagEnabled = multiTagValue === 'true';
        const hideStripEnabled = hideStripValue === 'true';
        const modernEnabled = modernUiValue === 'true';
        setClipboardMultiTagEnabled(multiTagEnabled);
        setHideEntryColorStripEnabled(hideStripEnabled);
        setModernUiEnabled(modernEnabled);
        onClipboardMultiTagChange?.(multiTagEnabled);
        onHideEntryColorStripChange?.(hideStripEnabled);
        onModernUiChange?.(modernEnabled);
      })
      .catch(console.error);
  }, [onClipboardMultiTagChange, onHideEntryColorStripChange, onModernUiChange]);

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
    const nextEnabled = !clipboardMultiTagEnabled;
    await setSetting(SETTING_KEYS.clipboardMultiTag, nextEnabled ? 'true' : 'false');
    setClipboardMultiTagEnabled(nextEnabled);
    onClipboardMultiTagChange?.(nextEnabled);
  }, [clipboardMultiTagEnabled, onClipboardMultiTagChange]);

  const handleModernUiToggle = useCallback(async () => {
    const nextEnabled = !modernUiEnabled;
    await setSetting(SETTING_KEYS.modernUi, nextEnabled ? 'true' : 'false');
    setModernUiEnabled(nextEnabled);
    onModernUiChange?.(nextEnabled);
  }, [modernUiEnabled, onModernUiChange]);

  const handleHideEntryColorStripToggle = useCallback(async () => {
    const nextEnabled = !hideEntryColorStripEnabled;
    await setSetting(SETTING_KEYS.hideEntryColorStrip, nextEnabled ? 'true' : 'false');
    setHideEntryColorStripEnabled(nextEnabled);
    onHideEntryColorStripChange?.(nextEnabled);
  }, [hideEntryColorStripEnabled, onHideEntryColorStripChange]);

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
          <div style={styles.compactRow} title={t.modernUiDesc}>
            <span style={styles.rowLabel}>{t.modernUi}</span>
            <button
              style={{ ...styles.toggle, ...(modernUiEnabled ? styles.toggleOn : {}) }}
              onClick={handleModernUiToggle}
            >
              <div style={{ ...styles.toggleKnob, ...(modernUiEnabled ? styles.toggleKnobOn : {}) }} />
            </button>
          </div>
          <div style={styles.compactRow} title={t.clipboardMultiTagDesc}>
            <span style={styles.rowLabel}>{t.clipboardMultiTag}</span>
            <button
              style={{ ...styles.toggle, ...(clipboardMultiTagEnabled ? styles.toggleOn : {}) }}
              onClick={handleClipboardMultiTagToggle}
            >
              <div style={{ ...styles.toggleKnob, ...(clipboardMultiTagEnabled ? styles.toggleKnobOn : {}) }} />
            </button>
          </div>
          <div style={styles.compactRow} title={t.hideEntryColorStripDesc}>
            <span style={styles.rowLabel}>{t.hideEntryColorStrip}</span>
            <button
              style={{ ...styles.toggle, ...(hideEntryColorStripEnabled ? styles.toggleOn : {}) }}
              onClick={handleHideEntryColorStripToggle}
            >
              <div style={{ ...styles.toggleKnob, ...(hideEntryColorStripEnabled ? styles.toggleKnobOn : {}) }} />
            </button>
          </div>
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
  compactRow: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    padding: '4px 0',
  },
  rowLabel: {
    fontSize: '12px',
    color: 'var(--text-primary)',
    flex: 1,
  },
  toggle: {
    position: 'relative',
    width: '34px',
    height: '18px',
    border: 'none',
    borderRadius: '9px',
    background: 'var(--border)',
    cursor: 'pointer',
    padding: 0,
    transition: 'background 0.2s',
    flexShrink: 0,
  },
  toggleOn: {
    background: 'var(--accent)',
  },
  toggleKnob: {
    position: 'absolute',
    top: '2px',
    left: '2px',
    width: '14px',
    height: '14px',
    borderRadius: '50%',
    background: '#ffffff',
    transition: 'transform 0.2s',
  },
  toggleKnobOn: {
    transform: 'translateX(16px)',
  },
};
