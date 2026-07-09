import { RotateCcw, Trash2 } from 'lucide-react';
import type { Memo } from '../types';
import { useI18n } from '../i18n';
import { formatRelativeTime, getArchiveDaysRemaining, getArchiveTone } from '../utils';
import { renderMemoBody } from './MemoBody';

interface Props {
  memo: Memo;
  onRestore: () => void;
  onPermanentDelete: () => void;
}

export default function ArchivedMemoItem({ memo, onRestore, onPermanentDelete }: Props) {
  const { t } = useI18n();
  const archiveDaysRemaining = memo.archived_at ? getArchiveDaysRemaining(memo.archived_at) : null;
  const archiveTone = getArchiveTone(archiveDaysRemaining ?? 0);
  const archiveTimerStyle = {
    warning: { color: '#f59e0b', background: 'rgba(245,158,11,0.1)' },
    danger: { color: '#ef4444', background: 'rgba(239,68,68,0.1)' },
  }[archiveTone];

  return (
    <div className="memo-entry" style={styles.item}>
      <div style={styles.content}>
        <div style={styles.header}>
          <span className="memo-selectable" style={styles.title}>
            {memo.title || '(untitled)'}
          </span>
          <div style={styles.actions}>
            <button style={styles.actionBtn} onClick={onRestore} title={t.restore}>
              <RotateCcw size={13} />
            </button>
            <button
              style={{ ...styles.actionBtn, ...styles.deleteBtn }}
              onClick={onPermanentDelete}
              title={t.permanentDelete}
            >
              <Trash2 size={13} />
            </button>
          </div>
        </div>
        <div className="memo-selectable memo-preview" style={styles.preview}>
          {renderMemoBody(memo.body, 100, 96)}
        </div>
        <div style={styles.meta}>
          <span className="memo-time" style={styles.time}>{formatRelativeTime(memo.created_at, t)}</span>
          {archiveDaysRemaining !== null && (
            <span style={{ ...styles.archiveTimer, ...archiveTimerStyle }}>
              {t.daysRemaining(archiveDaysRemaining)}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  item: {
    padding: '10px 12px',
    borderBottom: '1px solid var(--border)',
  },
  content: {
    display: 'flex',
    flexDirection: 'column',
    gap: '4px',
  },
  header: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'flex-start',
    background: 'var(--memo-contrast-bg)',
    padding: '4px 6px',
    borderRadius: '4px',
    margin: '-2px -4px 2px -4px',
  },
  title: {
    fontSize: '13px',
    fontWeight: 600,
    color: 'var(--text-primary)',
    flex: 1,
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
  },
  actions: {
    display: 'flex',
    gap: '4px',
    flexShrink: 0,
  },
  actionBtn: {
    width: 22,
    height: 22,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    border: 'none',
    borderRadius: 4,
    background: 'var(--surface)',
    color: 'var(--text-secondary)',
    fontSize: 11,
    cursor: 'pointer',
  },
  deleteBtn: {
    color: '#ef4444',
  },
  preview: {
    fontSize: '13px',
    color: 'var(--text-secondary)',
    margin: 0,
    lineHeight: 1.4,
    maxHeight: 64,
    overflow: 'hidden',
  },
  meta: {
    display: 'flex',
    alignItems: 'center',
    gap: '6px',
    marginTop: '2px',
  },
  time: {
    fontSize: '11px',
    color: 'var(--text-muted)',
  },
  archiveTimer: {
    fontSize: '10px',
    padding: '1px 6px',
    borderRadius: '8px',
    fontWeight: 500,
  },
};
