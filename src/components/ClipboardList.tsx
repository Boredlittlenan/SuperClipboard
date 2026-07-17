import { useRef } from 'react';
import type { ClipboardEntry } from '../types';
import ClipboardItem from './ClipboardItem';
import { useI18n } from '../i18n';
import type { UpdateResult } from '../api/clipboard';
import { useInfiniteScroll } from '../hooks/useInfiniteScroll';

interface Props {
  entries: ClipboardEntry[];
  onCopy: (id: number, useOriginal?: boolean) => void;
  onDelete: (id: number) => void;
  onTogglePin: (id: number) => void;
  onEdit: (id: number, content: string, expectedVersion: number) => Promise<UpdateResult>;
  rawPreview: boolean;
  loading: boolean;
  isArchive?: boolean;
  archiveEnabled?: boolean;
  multiTagEnabled?: boolean;
  showCategoryIndicator?: boolean;
  onRestore?: (id: number) => void;
  onPermanentDelete?: (id: number) => void;
  hasMore?: boolean;
  loadingMore?: boolean;
  onLoadMore?: () => void;
  selectionMode?: boolean;
  multiSelectEnabled?: boolean;
  selectedIds?: ReadonlySet<number>;
  onSelectionToggle?: (entry: ClipboardEntry) => void;
}

export default function ClipboardList({
  entries,
  onCopy,
  onDelete,
  onTogglePin,
  onEdit,
  rawPreview,
  loading,
  isArchive,
  archiveEnabled,
  multiTagEnabled,
  showCategoryIndicator,
  onRestore,
  onPermanentDelete,
  hasMore = false,
  loadingMore = false,
  onLoadMore,
  selectionMode = false,
  multiSelectEnabled = false,
  selectedIds,
  onSelectionToggle,
}: Props) {
  const { t } = useI18n();
  const listRef = useRef<HTMLDivElement>(null);
  const loadMoreRef = useRef<HTMLDivElement>(null);
  useInfiniteScroll(listRef, loadMoreRef, hasMore && !loadingMore, () => onLoadMore?.());

  if (loading && entries.length === 0) {
    return (
      <div style={styles.empty}>
        <div style={styles.spinner} />
        <span style={styles.emptyText}>{t.loading}</span>
      </div>
    );
  }

  if (entries.length === 0) {
    return (
      <div style={styles.empty}>
        <div style={styles.emptyIcon}>{'\u{1F4CB}'}</div>
        <span style={styles.emptyText}>{t.noEntries}</span>
        <span style={styles.emptyHint}>{t.noEntriesHint}</span>
      </div>
    );
  }

  return (
    <div ref={listRef} className="clipboard-list entry-list" style={styles.list}>
      {entries.map((entry) => (
        <ClipboardItem
          key={entry.id}
          entry={entry}
          onCopy={onCopy}
          onDelete={onDelete}
          onTogglePin={onTogglePin}
          onEdit={onEdit}
          rawPreview={rawPreview}
          isArchive={isArchive}
          archiveEnabled={archiveEnabled}
          multiTagEnabled={multiTagEnabled}
          showCategoryIndicator={showCategoryIndicator}
          onRestore={onRestore}
          onPermanentDelete={onPermanentDelete}
          selectionMode={selectionMode}
          multiSelectEnabled={multiSelectEnabled}
          selected={selectedIds?.has(entry.id) ?? false}
          onSelectionToggle={onSelectionToggle}
        />
      ))}
      <div ref={loadMoreRef} className="entry-list-tail" style={styles.loadMore} aria-hidden={!loadingMore}>
        {loadingMore && <div style={styles.loadMoreSpinner} />}
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  list: {
    flex: 1,
    overflowY: 'auto',
    overflowX: 'hidden',
  },
  empty: {
    flex: 1,
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    justifyContent: 'center',
    gap: '8px',
    padding: '40px 20px',
  },
  emptyIcon: {
    fontSize: '48px',
    opacity: 0.5,
  },
  emptyText: {
    fontSize: '14px',
    color: 'var(--text-secondary)',
    fontWeight: 500,
  },
  emptyHint: {
    fontSize: '12px',
    color: 'var(--text-muted)',
  },
  spinner: {
    width: '24px',
    height: '24px',
    border: '2px solid var(--border)',
    borderTopColor: 'var(--accent)',
    borderRadius: '50%',
    animation: 'spin 0.8s linear infinite',
  },
  loadMore: {
    minHeight: '18px',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
  },
  loadMoreSpinner: {
    width: '14px',
    height: '14px',
    border: '2px solid var(--border)',
    borderTopColor: 'var(--accent)',
    borderRadius: '50%',
    animation: 'spin 0.8s linear infinite',
  },
};
