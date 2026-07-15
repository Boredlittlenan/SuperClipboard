import { createContext, useContext } from 'react';
import type { Locale, Translations } from './translations';

export interface I18nContextValue {
  locale: Locale;
  t: Translations;
  setLocale: (locale: Locale) => Promise<void>;
}

export const I18nContext = createContext<I18nContextValue | null>(null);

export function useI18n(): I18nContextValue {
  const context = useContext(I18nContext);
  if (!context) {
    throw new Error('useI18n must be used inside I18nProvider');
  }
  return context;
}
