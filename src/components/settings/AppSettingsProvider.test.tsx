import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getSettings, setSetting } from '../../api/settings';
import { useAppSettings } from '../../hooks/useAppSettings';
import AppSettingsProvider from './AppSettingsProvider';

vi.mock('../../api/settings', () => ({
  getSettings: vi.fn(),
  setSetting: vi.fn(),
}));

const getSettingsMock = vi.mocked(getSettings);
const setSettingMock = vi.mocked(setSetting);

const wrapper = ({ children }: { children: ReactNode }) => (
  <AppSettingsProvider>{children}</AppSettingsProvider>
);

describe('AppSettingsProvider', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getSettingsMock.mockResolvedValue({});
    setSettingMock.mockResolvedValue();
  });

  it('loads and parses the settings schema', async () => {
    getSettingsMock.mockResolvedValue({
      memo_enabled: 'true',
      theme_mode: 'dark',
    });
    const { result } = renderHook(useAppSettings, { wrapper });

    await waitFor(() => expect(result.current.ready).toBe(true));
    expect(result.current.settings.memoEnabled).toBe(true);
    expect(result.current.settings.themeMode).toBe('dark');
  });

  it('serializes writes so rapid toggles cannot overtake each other', async () => {
    let releaseFirstWrite: () => void = () => {};
    const firstWrite = new Promise<void>((resolve) => {
      releaseFirstWrite = resolve;
    });
    setSettingMock.mockImplementationOnce(() => firstWrite);
    const { result } = renderHook(useAppSettings, { wrapper });
    await waitFor(() => expect(result.current.ready).toBe(true));

    let firstUpdate!: Promise<void>;
    let secondUpdate!: Promise<void>;
    act(() => {
      firstUpdate = result.current.setAppSetting('autoUpdate', false);
      secondUpdate = result.current.setAppSetting('themeMode', 'dark');
    });

    await waitFor(() => expect(setSettingMock).toHaveBeenCalledTimes(1));
    expect(setSettingMock).toHaveBeenNthCalledWith(1, 'auto_update', 'false');

    releaseFirstWrite();
    await act(async () => Promise.all([firstUpdate, secondUpdate]));
    expect(setSettingMock).toHaveBeenNthCalledWith(2, 'theme_mode', 'dark');
  });

  it('rolls back only the setting whose latest write failed', async () => {
    setSettingMock.mockRejectedValueOnce(new Error('write failed'));
    const { result } = renderHook(useAppSettings, { wrapper });
    await waitFor(() => expect(result.current.ready).toBe(true));

    await act(async () => {
      await result.current.setAppSetting('alwaysOnTop', true).catch(() => undefined);
    });

    expect(result.current.settings.alwaysOnTop).toBe(false);
  });
});
