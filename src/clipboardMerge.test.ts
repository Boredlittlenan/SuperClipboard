import { describe, expect, it } from 'vitest';
import type { ClipboardEntry } from './types';
import {
  canMergeEntries,
  getMergeCategory,
  getSelectedEntriesInListOrder,
  isBatchDeleteShortcut,
  MAX_MERGE_SELECTION,
  shouldToggleEntrySelection,
} from './clipboardMerge';

function entry(id: number, category: ClipboardEntry['category']): ClipboardEntry {
  return {
    id,
    category,
    category_tags: [category],
    content_type: category === 'image' ? 'image/png' : 'text/plain',
    content: String(id),
    preview: String(id),
    hash: String(id),
    pinned: false,
    created_at: '2026-07-16T00:00:00Z',
    original_content: null,
    updated_at: null,
    archived_at: null,
    version: 1,
  };
}

describe('clipboard merge selection', () => {
  it('keeps the current list order rather than click order', () => {
    const entries = [entry(3, 'text'), entry(2, 'text'), entry(1, 'text')];
    expect(getSelectedEntriesInListOrder(entries, [1, 3]).map((item) => item.id)).toEqual([3, 1]);
  });

  it('allows mixed categories to be selected but not merged', () => {
    const entries = [entry(1, 'link'), entry(2, 'text')];
    expect(getMergeCategory(entries, [1, 2])).toBe('link');
    expect(canMergeEntries(entries)).toBe(false);
  });

  it('only merges between two and the configured limit of one category', () => {
    expect(canMergeEntries([entry(1, 'text')])).toBe(false);
    expect(canMergeEntries([entry(1, 'text'), entry(2, 'text')])).toBe(true);
    expect(canMergeEntries(
      Array.from({ length: MAX_MERGE_SELECTION + 1 }, (_, index) => entry(index, 'text')),
    )).toBe(false);
  });

  it('starts selection from Ctrl+click only when the feature is enabled', () => {
    expect(shouldToggleEntrySelection(false, false, true)).toBe(false);
    expect(shouldToggleEntrySelection(false, true, false)).toBe(false);
    expect(shouldToggleEntrySelection(false, true, true)).toBe(true);
    expect(shouldToggleEntrySelection(true, true, false)).toBe(true);
  });

  it('handles Delete only for an active non-empty multi-selection', () => {
    expect(isBatchDeleteShortcut('Delete', true, true, 2)).toBe(true);
    expect(isBatchDeleteShortcut('Backspace', true, true, 2)).toBe(false);
    expect(isBatchDeleteShortcut('Delete', false, true, 2)).toBe(false);
    expect(isBatchDeleteShortcut('Delete', true, false, 2)).toBe(false);
    expect(isBatchDeleteShortcut('Delete', true, true, 0)).toBe(false);
  });
});
