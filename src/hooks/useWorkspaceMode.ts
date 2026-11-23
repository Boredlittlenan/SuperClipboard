import { useState } from 'react';
import { useAppSettings } from './useAppSettings';

export function useWorkspaceMode() {
  const { settings, setAppSetting } = useAppSettings();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const enabled = settings.workspaceModeBeta;

  async function toggle() {
    setBusy(true);
    setError(undefined);
    try {
      await setAppSetting('workspaceModeBeta', !enabled);
    } catch (failure) {
      setError(String(failure));
    } finally {
      setBusy(false);
    }
  }

  return { enabled, busy, error, toggle };
}
