import { describe, expect, it } from 'vitest';
import { getFooterItemCount } from './footerItemCount';
import type { Stats } from './types';

const stats: Stats = {
  total: 177,
  text: 95,
  link: 23,
  image: 52,
  code: 7,
  email: 5,
  file_path: 3,
  dbSize: 0,
  clipboardSize: 0,
  memoSize: 0,
  archive: 12,
  memoCount: 64,
  memoArchive: 9,
};

const baseInput = {
  activeTab: 'all' as const,
  archiveSubTab: 'clipboard' as const,
  stats,
  memoCount: 64,
  memoLoadedCount: 50,
  archiveCount: 12,
  memoArchiveCount: 9,
  loadedEntryCount: 50,
  loadedArchivedMemoCount: 50,
};

describe('getFooterItemCount', () => {
  it('uses the database total instead of the loaded clipboard page size', () => {
    expect(getFooterItemCount(baseInput)).toBe(177);
    expect(getFooterItemCount({ ...baseInput, activeTab: 'text' })).toBe(95);
  });

  it('uses total counts for memo and archive views', () => {
    expect(getFooterItemCount({ ...baseInput, activeTab: 'memo' })).toBe(64);
    expect(getFooterItemCount({ ...baseInput, activeTab: 'archive' })).toBe(12);
    expect(getFooterItemCount({
      ...baseInput,
      activeTab: 'archive',
      archiveSubTab: 'memos',
    })).toBe(9);
  });

  it('falls back to loaded counts before statistics are available', () => {
    expect(getFooterItemCount({
      ...baseInput,
      stats: null,
      memoCount: null,
      archiveCount: null,
      memoArchiveCount: null,
    })).toBe(50);
  });
});
