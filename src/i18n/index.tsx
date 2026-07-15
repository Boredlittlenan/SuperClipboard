import { useState, useEffect, useCallback, useRef } from 'react';
import type { Locale } from './translations';
import { translationsMap } from './translations';
import { getSetting, setSetting } from '../api/settings';
import { I18nContext } from './context';

export { useI18n } from './context';

function detectSystemLocale(): Locale {
  return navigator.language.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en';
}

const DEFAULT_LOCALE: Locale = detectSystemLocale();
const SETTING_KEY = 'language';

export function I18nProvider({ children }: { children: React.ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(DEFAULT_LOCALE);
  const [ready, setReady] = useState(false);
  const localeRef = useRef(locale);
  const revisionRef = useRef(0);
  const writeQueueRef = useRef<Promise<void>>(Promise.resolve());

  // Load saved locale on mount
  useEffect(() => {
    let cancelled = false;
    getSetting(SETTING_KEY).then((saved) => {
      if (cancelled) return;
      const nextLocale = saved === 'zh-CN' || saved === 'en'
        ? saved
        : detectSystemLocale();
      localeRef.current = nextLocale;
      setLocaleState(nextLocale);
      setReady(true);
    }).catch((error) => {
      console.error('Failed to load language:', error);
      if (!cancelled) setReady(true);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const setLocale = useCallback(async (newLocale: Locale) => {
    if (localeRef.current === newLocale) return;
    const previousLocale = localeRef.current;
    const revision = ++revisionRef.current;
    localeRef.current = newLocale;
    setLocaleState(newLocale);
    const write = writeQueueRef.current.then(() => setSetting(SETTING_KEY, newLocale));
    writeQueueRef.current = write.catch(() => undefined);
    try {
      await write;
    } catch (error) {
      if (revisionRef.current === revision) {
        localeRef.current = previousLocale;
        setLocaleState(previousLocale);
      }
      throw error;
    }
  }, []);

  const t = translationsMap[locale] ?? translationsMap[DEFAULT_LOCALE];

  // While loading saved locale, render with default
  if (!ready) return null;

  return (
    <I18nContext.Provider value={{ locale, t, setLocale }}>
      {children}
    </I18nContext.Provider>
  );
}
