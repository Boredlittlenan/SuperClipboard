import { createContext } from 'react';
import type { AppSettingName, AppSettings } from './schema';

export interface AppSettingsContextValue {
  settings: AppSettings;
  ready: boolean;
  reloadSettings: () => Promise<void>;
  setAppSetting: <K extends AppSettingName>(name: K, value: AppSettings[K]) => Promise<void>;
}

export const AppSettingsContext = createContext<AppSettingsContextValue | null>(null);
