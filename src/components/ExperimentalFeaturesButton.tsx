import { useCallback, useEffect, useRef, useState } from 'react';
import type React from 'react';
import { FlaskConical, RefreshCw } from 'lucide-react';
import { useI18n } from '../i18n';
import { useAppSettings } from '../hooks/useAppSettings';
import { useClickOutside } from '../hooks/useClickOutside';
import { ToggleSettingRow } from './settings/SettingRow';
import FloatingMenuPanel from './FloatingMenuPanel';
import { getClassificationStatus, type ClassificationStatus } from '../api/clipboard';

interface Props {
  reclassifyingHistory: boolean;
  onReclassifyHistory: () => void | Promise<void>;
}

export default function ExperimentalFeaturesButton({
  reclassifyingHistory,
  onReclassifyHistory,
}: Props) {
  const { t } = useI18n();
  const { settings, setAppSetting } = useAppSettings();
  const {
    clipboardMultiTagEnabled,
    multiSelectEnabled,
    hideEntryColorStripEnabled,
    categoryTabSelectedColorsEnabled,
    modernUiEnabled,
  } = settings;
  const [open, setOpen] = useState(false);
  const [classificationStatus, setClassificationStatus] = useState<ClassificationStatus | null>(null);
  const anchorRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  useClickOutside(anchorRef, open, () => setOpen(false), false, panelRef);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    getClassificationStatus()
      .then((status) => {
        if (!cancelled) setClassificationStatus(status);
      })
      .catch((error) => console.error('Failed to load classification status:', error));
    return () => {
      cancelled = true;
    };
  }, [open]);

  const handleClipboardMultiTagToggle = useCallback(async () => {
    await setAppSetting('clipboardMultiTagEnabled', !clipboardMultiTagEnabled);
  }, [clipboardMultiTagEnabled, setAppSetting]);

  const handleMultiSelectToggle = useCallback(async () => {
    await setAppSetting('multiSelectEnabled', !multiSelectEnabled);
  }, [multiSelectEnabled, setAppSetting]);

  const handleModernUiToggle = useCallback(async () => {
    await setAppSetting('modernUiEnabled', !modernUiEnabled);
  }, [modernUiEnabled, setAppSetting]);

  const handleHideEntryColorStripToggle = useCallback(async () => {
    await setAppSetting('hideEntryColorStripEnabled', !hideEntryColorStripEnabled);
  }, [hideEntryColorStripEnabled, setAppSetting]);

  const handleCategoryTabSelectedColorsToggle = useCallback(async () => {
    await setAppSetting('categoryTabSelectedColorsEnabled', !categoryTabSelectedColorsEnabled);
  }, [categoryTabSelectedColorsEnabled, setAppSetting]);

  const handleReclassifyHistory = useCallback(() => {
    setOpen(false);
    void onReclassifyHistory();
  }, [onReclassifyHistory]);

  return (
    <div style={styles.wrapper} ref={anchorRef}>
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
        <FloatingMenuPanel anchorRef={anchorRef} panelRef={panelRef} style={styles.panel}>
          <div style={styles.panelTitle}>{t.experimentalFeatures}</div>
          <div style={styles.sectionHeader}>
            <span>{t.appearanceSettings}</span>
            <span style={styles.sectionLine} />
          </div>
          <ToggleSettingRow label={t.modernUi} title={t.modernUiDesc} checked={modernUiEnabled} onChange={handleModernUiToggle} />
          <ToggleSettingRow label={t.hideEntryColorStrip} title={t.hideEntryColorStripDesc} checked={hideEntryColorStripEnabled} onChange={handleHideEntryColorStripToggle} />
          <ToggleSettingRow label={t.categoryTabSelectedColors} title={t.categoryTabSelectedColorsDesc} checked={categoryTabSelectedColorsEnabled} onChange={handleCategoryTabSelectedColorsToggle} />
          <div style={{ ...styles.sectionHeader, ...styles.sectionHeaderSpaced }}>
            <span>{t.featureSettings}</span>
            <span style={styles.sectionLine} />
          </div>
          <ToggleSettingRow label={t.multiSelectMode} title={t.multiSelectModeDesc} checked={multiSelectEnabled} onChange={handleMultiSelectToggle} />
          <ToggleSettingRow label={t.clipboardMultiTag} title={t.clipboardMultiTagDesc} checked={clipboardMultiTagEnabled} onChange={handleClipboardMultiTagToggle} />
          <button
            type="button"
            className="experimental-action-button"
            title={t.reclassifyHistoryDesc}
            disabled={reclassifyingHistory}
            onClick={handleReclassifyHistory}
          >
            <RefreshCw size={13} strokeWidth={2.1} aria-hidden="true" />
            <span>{reclassifyingHistory ? t.reclassifyHistoryPending : t.reclassifyHistory}</span>
          </button>
          {classificationStatus && (
            <div style={styles.classificationStatus}>
              {classificationStatus.appliedVersion === classificationStatus.currentVersion
                ? t.classificationRulesCurrent(classificationStatus.currentVersion)
                : t.classificationRulesOutdated(classificationStatus.currentVersion)}
            </div>
          )}
        </FloatingMenuPanel>
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
    border: '1px solid var(--apple-separator)',
    borderRadius: '12px',
    padding: '12px',
    zIndex: 1310,
  },
  panelTitle: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    fontSize: '13px',
    fontWeight: 600,
    lineHeight: '20px',
    color: 'var(--text-primary)',
    marginBottom: '7px',
    padding: '0 1px',
  },
  sectionHeader: {
    display: 'flex',
    alignItems: 'center',
    gap: '8px',
    color: 'var(--text-muted)',
    fontSize: '11px',
    fontWeight: 600,
    lineHeight: '16px',
    letterSpacing: '0',
    marginBottom: '3px',
    padding: '0 1px',
  },
  sectionLine: {
    flex: 1,
    height: '1px',
    background: 'var(--border)',
    transform: 'translateY(1px)',
  },
  sectionHeaderSpaced: {
    marginTop: '8px',
  },
  classificationStatus: {
    marginTop: '6px',
    padding: '0 2px',
    color: 'var(--text-muted)',
    fontSize: '10px',
    lineHeight: '15px',
  },
};
