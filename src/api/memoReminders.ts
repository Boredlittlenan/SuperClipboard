import { invoke } from '@tauri-apps/api/core';
import type { Memo } from '../types';

export interface MemoReminder {
  memoId: number;
  token: string;
  title: string;
  dueAt: number;
  triggeredAt: number | null;
}

export interface MemoReminderList {
  source: string;
  items: MemoReminder[];
}

export function listMemoReminders(): Promise<MemoReminderList> {
  return invoke('list_memo_reminders');
}

export function scheduleMemoReminder(source: string, memo: Pick<Memo, 'id' | 'created_at'>, dueAt: number): Promise<void> {
  return invoke('schedule_memo_reminder', { source, memoId: memo.id, createdAt: memo.created_at, dueAt });
}

export function changeMemoReminder(token: string, snooze = false): Promise<void> {
  return invoke('change_memo_reminder', { token, snooze });
}

export function openMemoReminder(token: string): Promise<Memo | null> {
  return invoke('open_memo_reminder', { token });
}

export function takeMemoReminderActivation(): Promise<string | null> {
  return invoke('take_memo_reminder_activation');
}
