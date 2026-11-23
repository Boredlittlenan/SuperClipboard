import { invoke } from '@tauri-apps/api/core';
import type { Category, ClipboardEntry, Memo } from '../types';
import type { Item, RecordKind } from './model';

export type Repeat = 'once' | 'daily' | 'weekly';
export interface Reminder {
  memoId: number;
  title: string;
  at: string;
  repeat: Repeat;
  status: 'active' | 'fired' | 'completed';
  nextDue: number;
  lastSent: number | null;
  error: string | null;
}

export interface ImportPreview {
  token: string;
  format: string;
  appVersion: string;
  counts: { clipboard: number; memos: number; images: number; favorites: number; archived: number };
  sample: Array<{ kind: RecordKind; label: string }>;
}

export interface ImportResult {
  clipboard: number;
  memos: number;
  duplicates: number;
}

export interface ListQuery {
  search: string;
  limit: number;
  offset: number;
  favorites: boolean;
  archived: boolean;
  category?: Category;
}

export async function listItems(kind: RecordKind, query: ListQuery): Promise<Item[]> {
  const { archived, ...options } = query;
  if (kind === 'memo') {
    const rows = await invoke<Memo[]>(archived ? 'get_archived_memos' : 'get_memos', {
      filter: { ...options, summary: true },
    });
    return rows.map((data) => ({ kind, data }));
  }
  const rows = await invoke<ClipboardEntry[]>(archived ? 'get_archived_entries' : 'get_entries', {
    filter: { ...options, summary: true, includeAuxiliaryTags: true },
  });
  return rows.map((data) => ({ kind, data }));
}

export async function getItem(kind: RecordKind, id: number): Promise<Item | null> {
  if (kind === 'memo') {
    const data = await invoke<Memo | null>('get_memo', { id });
    return data && { kind, data };
  }
  const data = await invoke<ClipboardEntry | null>('get_entry', { id });
  return data && { kind, data };
}

export const listReminders = () => invoke<Reminder[]>('list_reminders');
export const saveReminder = (memoId: number, at: string, repeat: Repeat) =>
  invoke<void>('save_reminder', { memoId, at, repeat });
export const finishReminder = (memoId: number, completed: boolean) =>
  invoke<void>('finish_reminder', { memoId, completed });
export const discoverSources = () => invoke<string[]>('discover_legacy_sources');
export const previewImport = (path: string | null) => invoke<ImportPreview>('preview_import', { path });
export const commitImport = (token: string) => invoke<ImportResult>('commit_import', { token });
export const exportWorkspace = (path: string) => invoke<void>('export_workspace', { path });
export const switchWorkspace = (target: { mode: 'local' } | { mode: 'remote'; url: string },
  classic?: { profiles: string; activeId: string }) =>
  invoke<void>('switch_workspace', { target, classicProfiles: classic?.profiles, activeProfile: classic?.activeId });
export const getClassicRemoteSettings = () => invoke<{
  isRemote: boolean; url: string; profiles: string; activeProfile: string;
}>('get_classic_remote_settings');
export const replaceClassicRemoteProfiles = (profiles: string) =>
  invoke<void>('replace_classic_remote_profiles', { profiles });
