import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getSettings, setSetting } from '../../api/settings';
import { AppSettingsContext, type AppSettingsContextValue } from '../../settings/appSettingsContext';
import {
  APP_SETTING_KEYS,
  parseAppSettings,
  serializeAppSetting,
  type AppSettingName,
  type AppSettings,
} from '../../settings/schema';

const defaultSettings = parseAppSettings({});

export default function AppSettingsProvider({ children }: { children: React.ReactNode }) {
  const [settings, setSettingsState] = useState(defaultSettings);
  const [ready, setReady] = useState(false);
  const settingsRef = useRef(settings);

  const replaceSettings = useCallback((next: AppSettings) => {
    settingsRef.current = next;
    setSettingsState(next);
  }, []);

  const reloadSettings = useCallback(async () => {
    const values = await getSettings(APP_SETTING_KEYS);
    replaceSettings(parseAppSettings(values));
    setReady(true);
  }, [replaceSettings]);

  useEffect(() => {
    reloadSettings().catch((error) => {
      console.error('Failed to load app settings:', error);
      setReady(true);
    });
  }, [reloadSettings]);

  const setAppSetting = useCallback(async <K extends AppSettingName>(
    name: K,
    value: AppSettings[K],
  ) => {
    const previous = settingsRef.current;
    replaceSettings({ ...previous, [name]: value });
    const serialized = serializeAppSetting(name, value);
    try {
      await setSetting(serialized.key, serialized.value);
    } catch (error) {
      if (settingsRef.current[name] === value) {
        replaceSettings(previous);
      }
      throw error;
    }
  }, [replaceSettings]);

  const contextValue = useMemo<AppSettingsContextValue>(() => ({
    settings,
    ready,
    reloadSettings,
    setAppSetting,
  }), [ready, reloadSettings, setAppSetting, settings]);

  return (
    <AppSettingsContext.Provider value={contextValue}>
      {children}
    </AppSettingsContext.Provider>
  );
}
