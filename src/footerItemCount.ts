import type { FilterTab, Stats } from './types';

interface FooterItemCountInput {
  activeTab: FilterTab;
  archiveSubTab: 'clipboard' | 'memos';
  stats: Stats | null;
  memoCount: number | null;
  memoLoadedCount: number;
  archiveCount: number | null;
  memoArchiveCount: number | null;
  loadedEntryCount: number;
  loadedArchivedMemoCount: number;
}

export function getFooterItemCount({
  activeTab,
  archiveSubTab,
  stats,
  memoCount,
  memoLoadedCount,
  archiveCount,
  memoArchiveCount,
  loadedEntryCount,
  loadedArchivedMemoCount,
}: FooterItemCountInput): number {
  if (activeTab === 'memo') {
    return memoCount ?? stats?.memoCount ?? memoLoadedCount;
  }

  if (activeTab === 'archive') {
    return archiveSubTab === 'memos'
      ? memoArchiveCount ?? stats?.memoArchive ?? loadedArchivedMemoCount
      : archiveCount ?? stats?.archive ?? loadedEntryCount;
  }

  if (activeTab === 'all') {
    return stats?.total ?? loadedEntryCount;
  }

  return stats?.[activeTab] ?? loadedEntryCount;
}
