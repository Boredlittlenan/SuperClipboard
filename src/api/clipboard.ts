import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Category, ClipboardEntry, QueryFilter, Stats } from '../types';
import { createEntryContentLoader } from './entryContentLoader';

export interface UpdateResult {
  updated: boolean;
  conflict: boolean;
}

export interface MergeEntriesResult {
  kind: 'clipboard' | 'memo';
  created: boolean;
  deletedOriginals: number;
}

export interface ClassificationStatus {
  currentVersion: number;
  appliedVersion: number | null;
}

const loadEntryContent = createEntryContentLoader(
  (id) => invoke<string | null>('get_entry_content', { id }),
  1,
);

/** Fetch clipboard entries with optional filter */
export async function getEntries(filter?: QueryFilter): Promise<ClipboardEntry[]> {
  return invoke('get_entries', { filter });
}

/** Fetch deferred clipboard content, currently used for remote image previews. */
export async function getEntryContent(id: number): Promise<string | null> {
  return loadEntryContent(id);
}

/** Save an image clipboard entry as a PNG file selected by the user. */
export async function exportClipboardImage(id: number, path: string): Promise<void> {
  return invoke('export_clipboard_image', { id, path });
}

/** Import dropped plain text into the system clipboard and the active storage backend. */
export async function importDroppedText(text: string): Promise<boolean> {
  return invoke('import_dropped_text', { text });
}

/** Import an image data URL dropped onto the app into the system clipboard and history. */
export async function importDroppedImage(dataUrl: string): Promise<boolean> {
  return invoke('import_dropped_image', { dataUrl });
}

/** Merge selected entries in the provided display order. */
export async function mergeEntries(
  ids: number[],
  memoTitle: string,
  deleteOriginals = false,
  archiveOriginals = false,
): Promise<MergeEntriesResult> {
  return invoke('merge_entries', { ids, memoTitle, deleteOriginals, archiveOriginals });
}

/** Delete or archive multiple clipboard entries in one storage operation. */
export async function deleteEntries(ids: number[], archive?: boolean): Promise<number> {
  return invoke('delete_entries', { ids, archive });
}

/** Delete a clipboard entry by ID (optionally archive instead of hard delete) */
export async function deleteEntry(id: number, archive?: boolean): Promise<boolean> {
  return invoke('delete_entry', { id, archive });
}

/** Toggle pin status of an entry */
export async function togglePin(id: number): Promise<boolean> {
  return invoke('toggle_pin', { id });
}

/** Get category statistics */
export async function getStats(includeAuxiliaryTags = false): Promise<Stats> {
  return invoke('get_stats', { includeAuxiliaryTags });
}

/** Recompute category metadata for existing entries in the active storage backend. */
export async function reclassifyClipboardEntries(): Promise<number> {
  return invoke('reclassify_clipboard_entries');
}

/** Return the classification ruleset applied to the active storage backend. */
export async function getClassificationStatus(): Promise<ClassificationStatus> {
  return invoke('get_classification_status');
}

/** Clear non-pinned entries, optionally limited to the selected category tab. */
export async function clearUnpinned(
  archive?: boolean,
  category?: Category,
  includeAuxiliaryTags = false,
): Promise<number> {
  return invoke('clear_unpinned', { archive, category, includeAuxiliaryTags });
}

/** Copy an entry back to system clipboard, optionally using its first captured content. */
export async function copyToClipboard(id: number, useOriginal = false): Promise<boolean> {
  return invoke('copy_to_clipboard', { id, useOriginal });
}

/** Update a clipboard entry's content */
export async function updateEntry(
  id: number,
  content: string,
  expectedVersion?: number,
): Promise<UpdateResult> {
  return invoke('update_entry', { id, content, expectedVersion });
}

/** Archive a clipboard entry */
export async function archiveEntry(id: number): Promise<boolean> {
  return invoke('archive_entry', { id });
}

/** Unarchive (restore) a clipboard entry */
export async function unarchiveEntry(id: number): Promise<boolean> {
  return invoke('unarchive_entry', { id });
}

/** Get archived clipboard entries */
export async function getArchivedEntries(filter?: QueryFilter): Promise<ClipboardEntry[]> {
  return invoke('get_archived_entries', { filter });
}

/** Get archived entries count */
export async function archiveCount(): Promise<number> {
  return invoke('archive_count');
}

/** Permanently delete an archived entry */
export async function permanentDelete(id: number): Promise<boolean> {
  return invoke('permanent_delete', { id });
}

/** Purge archives older than specified days */
export async function purgeOldArchives(days: number): Promise<number> {
  return invoke('purge_old_archives', { days });
}

/** Permanently delete every clipboard entry currently in the recycle bin. */
export async function emptyArchive(): Promise<number> {
  return invoke('empty_archive');
}

/** Listen for new clipboard events */
export function onClipboardChanged(
  callback: (entry: ClipboardEntry) => void
): Promise<() => void> {
  return listen<ClipboardEntry>('clipboard-changed', (event) => {
    callback(event.payload);
  });
}
