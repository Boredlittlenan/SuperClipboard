import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { ClipboardEntry, QueryFilter, Stats } from '../types';

/** Fetch clipboard entries with optional filter */
export async function getEntries(filter?: QueryFilter): Promise<ClipboardEntry[]> {
  return invoke('get_entries', { filter });
}

/** Delete a clipboard entry by ID */
export async function deleteEntry(id: number): Promise<boolean> {
  return invoke('delete_entry', { id });
}

/** Toggle pin status of an entry */
export async function togglePin(id: number): Promise<boolean> {
  return invoke('toggle_pin', { id });
}

/** Get category statistics */
export async function getStats(): Promise<Stats> {
  return invoke('get_stats');
}

/** Clear all non-pinned entries */
export async function clearUnpinned(): Promise<number> {
  return invoke('clear_unpinned');
}

/** Copy an entry back to system clipboard */
export async function copyToClipboard(id: number): Promise<boolean> {
  return invoke('copy_to_clipboard', { id });
}

/** Listen for new clipboard events */
export function onClipboardChanged(
  callback: (entry: ClipboardEntry) => void
): Promise<() => void> {
  return listen<ClipboardEntry>('clipboard-changed', (event) => {
    callback(event.payload);
  });
}
