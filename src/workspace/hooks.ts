import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useI18n } from '../i18n';
import { listReminders } from './bridge';
import { describeError } from './model';

export function useLabels() {
  const { locale, setLocale } = useI18n();
  const t = useCallback((zh: string, en: string) => locale === 'zh-CN' ? zh : en, [locale]);
  return { t, locale, setLocale };
}

export function useDesktopEvent<T>(event: string, onEvent: (payload: T) => void) {
  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    void listen<T>(event, ({ payload }) => { if (!disposed) onEvent(payload); }).then((unlisten) => {
      if (disposed) unlisten();
      else unsubscribe = unlisten;
    }).catch((error) => console.error(`Could not listen for ${event}`, error));
    return () => { disposed = true; unsubscribe?.(); };
  }, [event, onEvent]);
}

interface LoadResult<T> {
  key: string;
  data?: T;
  error?: string;
}

export function useLoad<T>(key: string, load: () => Promise<T>) {
  const [result, setResult] = useState<LoadResult<T>>();
  const [revision, setRevision] = useState(0);
  const refresh = useCallback(() => setRevision((value) => value + 1), []);
  useEffect(() => {
    let disposed = false;
    void load().then(
      (data) => { if (!disposed) setResult({ key, data }); },
      (error) => { if (!disposed) setResult({ key, error: describeError(error) }); },
    );
    return () => { disposed = true; };
  }, [key, load, revision]);
  const current = result?.key === key ? result : undefined;
  return { data: current?.data, error: current?.error, loading: !current, refresh };
}

export function useDebounced(value: string) {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(value.trim()), 160);
    return () => window.clearTimeout(timer);
  }, [value]);
  return debounced;
}

export function useReminders(workspace: number) {
  const reminders = useLoad(String(workspace), listReminders);
  useDesktopEvent('reminders-changed', reminders.refresh);
  return reminders;
}

export function useOperation(onError: (text: string) => void) {
  const [busy, setBusy] = useState(false);
  const active = useRef(false);
  const run = async <T,>(job: () => Promise<T>): Promise<T | undefined> => {
    if (active.current) return;
    active.current = true;
    setBusy(true);
    try { return await job(); }
    catch (error) { onError(describeError(error)); }
    finally { active.current = false; setBusy(false); }
  };
  return { busy, run };
}
