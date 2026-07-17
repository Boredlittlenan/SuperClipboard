import type { Category, ClipboardEntry } from './types';

export const MAX_MERGE_SELECTION = 20;

export function getSelectedEntriesInListOrder(
  entries: ClipboardEntry[],
  selectedIds: readonly number[],
): ClipboardEntry[] {
  const selected = new Set(selectedIds);
  return entries.filter((entry) => selected.has(entry.id));
}

export function getMergeCategory(
  entries: ClipboardEntry[],
  selectedIds: readonly number[],
): Category | null {
  return getSelectedEntriesInListOrder(entries, selectedIds)[0]?.category ?? null;
}

export function canMergeEntries(entries: readonly ClipboardEntry[]): boolean {
  if (entries.length < 2 || entries.length > MAX_MERGE_SELECTION) return false;
  const category = entries[0].category;
  return entries.every((entry) => entry.category === category);
}

export function shouldToggleEntrySelection(
  selectionMode: boolean,
  multiSelectEnabled: boolean,
  ctrlKey: boolean,
): boolean {
  return selectionMode || (multiSelectEnabled && ctrlKey);
}

export function isBatchDeleteShortcut(
  key: string,
  multiSelectEnabled: boolean,
  selectionMode: boolean,
  selectedCount: number,
): boolean {
  return key === 'Delete' && multiSelectEnabled && selectionMode && selectedCount > 0;
}
