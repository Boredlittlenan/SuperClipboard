import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getSetting, setSetting } from '../api/settings';
import { I18nProvider, useI18n } from './index';

vi.mock('../api/settings', () => ({
  getSetting: vi.fn(),
  setSetting: vi.fn(),
}));

const getSettingMock = vi.mocked(getSetting);
const setSettingMock = vi.mocked(setSetting);

function LanguageHarness() {
  const { locale, t, setLocale } = useI18n();
  return (
    <div>
      <span data-testid="locale">{locale}</span>
      <span data-testid="title">{t.appTitle}</span>
      <button type="button" onClick={() => { void setLocale('en').catch(() => undefined); }}>
        Switch English
      </button>
      <button type="button" onClick={() => { void setLocale('zh-CN').catch(() => undefined); }}>
        Switch Chinese
      </button>
    </div>
  );
}

describe('I18nProvider', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getSettingMock.mockResolvedValue('zh-CN');
    setSettingMock.mockResolvedValue();
  });

  it('keeps locale, translations, and persistence in sync', async () => {
    render(<I18nProvider><LanguageHarness /></I18nProvider>);
    await screen.findByText('超级剪贴板');
    expect(screen.getByTestId('locale')).toHaveTextContent('zh-CN');

    fireEvent.click(screen.getByRole('button', { name: 'Switch English' }));
    await screen.findByText('SuperClipboard');
    expect(screen.getByTestId('locale')).toHaveTextContent('en');
    expect(setSettingMock).toHaveBeenCalledWith('language', 'en');
  });

  it('rolls the whole language state back when persistence fails', async () => {
    setSettingMock.mockRejectedValueOnce(new Error('write failed'));
    render(<I18nProvider><LanguageHarness /></I18nProvider>);
    await screen.findByText('超级剪贴板');

    fireEvent.click(screen.getByRole('button', { name: 'Switch English' }));
    await waitFor(() => {
      expect(screen.getByTestId('locale')).toHaveTextContent('zh-CN');
      expect(screen.getByTestId('title')).toHaveTextContent('超级剪贴板');
    });
  });

  it('serializes rapid language changes', async () => {
    let releaseFirstWrite: () => void = () => {};
    const firstWrite = new Promise<void>((resolve) => {
      releaseFirstWrite = resolve;
    });
    setSettingMock.mockImplementationOnce(() => firstWrite);
    render(<I18nProvider><LanguageHarness /></I18nProvider>);
    await screen.findByText('超级剪贴板');

    fireEvent.click(screen.getByRole('button', { name: 'Switch English' }));
    fireEvent.click(screen.getByRole('button', { name: 'Switch Chinese' }));
    await waitFor(() => expect(setSettingMock).toHaveBeenCalledTimes(1));

    releaseFirstWrite();
    await waitFor(() => expect(setSettingMock).toHaveBeenCalledTimes(2));
    expect(setSettingMock).toHaveBeenNthCalledWith(1, 'language', 'en');
    expect(setSettingMock).toHaveBeenNthCalledWith(2, 'language', 'zh-CN');
    expect(screen.getByTestId('locale')).toHaveTextContent('zh-CN');
    expect(screen.getByTestId('title')).toHaveTextContent('超级剪贴板');
  });
});
