import { useCallback, useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { listMemoReminders, type MemoReminderList } from '../api/memoReminders';

export function useMemoReminders(refreshKey: number) {
  const [data, setData] = useState<MemoReminderList | null>(null);
  const [failed, setFailed] = useState(false);
  const scan = useRef(0);

  const reload = useCallback(async () => {
    const request = ++scan.current;
    try {
      const next = await listMemoReminders();
      if (request === scan.current) {
        setData(next);
        setFailed(false);
      }
    } catch (error) {
      if (request === scan.current) {
        setFailed(true);
      }
      console.error('Failed to load memo reminders:', error);
    }
  }, []);

  useEffect(() => {
    let live = true;
    let unlisten: (() => void) | undefined;
    listen('memo-reminders-changed', () => { void reload(); }).then((dispose) => {
      if (live) {
        unlisten = dispose;
        void reload();
      } else {
        dispose();
      }
    }).catch(console.error);
    return () => {
      live = false;
      unlisten?.();
      scan.current += 1;
    };
  }, [reload]);

  useEffect(() => { void reload(); }, [reload, refreshKey]);

  return { data, failed, reload };
}
