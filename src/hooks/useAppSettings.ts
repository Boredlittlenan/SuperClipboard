import { useContext } from 'react';
import { AppSettingsContext, type AppSettingsContextValue } from '../settings/appSettingsContext';

export function useAppSettings(): AppSettingsContextValue {
  const context = useContext(AppSettingsContext);
  if (!context) {
    throw new Error('useAppSettings must be used inside AppSettingsProvider');
  }
  return context;
}
