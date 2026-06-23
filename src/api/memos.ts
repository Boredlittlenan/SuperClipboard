import { invoke } from '@tauri-apps/api/core';
import type { Memo, MemoFilter } from '../types';

export async function getMemos(filter?: MemoFilter): Promise<Memo[]> {
  return invoke('get_memos', { filter });
}

export async function createMemo(title: string, body: string, tags: string): Promise<Memo> {
  return invoke('create_memo', { title, body, tags });
}

export async function updateMemo(id: number, title: string, body: string, tags: string): Promise<boolean> {
  return invoke('update_memo', { id, title, body, tags });
}

export async function deleteMemo(id: number): Promise<boolean> {
  return invoke('delete_memo', { id });
}

export async function toggleMemoPin(id: number): Promise<boolean> {
  return invoke('toggle_memo_pin', { id });
}

export async function memoCount(): Promise<number> {
  return invoke('memo_count');
}
