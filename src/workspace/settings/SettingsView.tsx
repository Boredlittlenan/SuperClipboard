import { invoke } from '@tauri-apps/api/core';
import { Bell, FlaskConical, Keyboard, Laptop, RefreshCw } from 'lucide-react';
import { useState } from 'react';
import { checkUpdate, getAutostartEnabled, getShortcut, openUrl, setAlwaysOnTop, setAutostartEnabled, setShortcut } from '../../api/settings';
import { useAppSettings } from '../../hooks/useAppSettings';
import { useI18n } from '../../i18n';
import { useWorkspaceMode } from '../../hooks/useWorkspaceMode';
import { LoadState, Select } from '../controls';
import { useLabels, useLoad, useOperation } from '../hooks';

export type Theme = 'system' | 'light' | 'dark';

const loadSettings = () => Promise.all([getAutostartEnabled(), getShortcut()]);

export function SettingsView({ theme, onTheme, paused, onPause, onShortcut, onNotice, onError }: {
  theme: Theme; onTheme: (theme: Theme) => void; paused: boolean; onPause: (paused: boolean) => void;
  onShortcut: (shortcut: string) => void; onNotice: (text: string) => void; onError: (text: string) => void;
}) {
  const { t, locale, setLocale } = useLabels();
  const { t: labels } = useI18n();
  const workspaceMode = useWorkspaceMode();
  const { setAppSetting } = useAppSettings();
  const settings = useLoad('settings', loadSettings);
  const [shortcutDraft, setShortcutDraft] = useState<string>();
  const { run, busy } = useOperation(onError);
  return <div className="settings-scroll">
    <section className="settings-card">
      <h2><FlaskConical size={19}/>{labels.experimentalFeatures}</h2>
      <label className="setting">{labels.workspaceModeBeta}
        <input type="checkbox" checked={workspaceMode.enabled} disabled={workspaceMode.busy}
          onChange={() => void workspaceMode.toggle()}/>
      </label>
      {workspaceMode.error && <p role="alert" className="inline-error">{workspaceMode.error}</p>}
    </section>
    <section className="settings-card">
      <h2><Laptop size={19}/>{t('外观和启动', 'Appearance and startup')}</h2>
      <div className="setting"><span>{t('界面主题', 'Theme')}</span>
        <Select label={t('界面主题', 'Theme')} value={theme} onChange={(value) => void run(async () => {
          await setAppSetting('themeMode', value as Theme); onTheme(value as Theme);
        })}>
          <option value="system">{t('跟随系统', 'System')}</option>
          <option value="light">{t('浅色', 'Light')}</option>
          <option value="dark">{t('深色', 'Dark')}</option>
        </Select>
      </div>
      <div className="setting"><span>{t('语言', 'Language')}</span>
        <Select label={t('语言', 'Language')} value={locale} onChange={(value) => void run(() => setLocale(value as 'zh-CN' | 'en'))}>
          <option value="zh-CN">简体中文</option><option value="en">English</option>
        </Select>
      </div>
      {settings.data ? <label className="setting">{t('登录后在托盘启动', 'Start in tray on login')}
        <input type="checkbox" checked={settings.data[0]} disabled={busy} onChange={(event) => {
          const checked = event.target.checked;
          void run(async () => { await setAutostartEnabled(checked); settings.refresh(); });
        }}/>
      </label> : <LoadState error={settings.error} retry={settings.refresh}/>}
      <label className="setting">{t('记录剪贴板', 'Capture clipboard')}
        <input type="checkbox" checked={!paused} disabled={busy} onChange={(event) => {
          const next = !event.target.checked;
          void run(async () => { await setAppSetting('capturePaused', next); onPause(next); });
        }}/>
      </label>
      <PinWindow onError={onError}/>
    </section>
    <section className="settings-card">
      <h2><Keyboard size={19}/>{t('全局快捷键', 'Global shortcut')}</h2>
      <form className="storage-connect" onSubmit={(event) => {
        event.preventDefault();
        if (!shortcutDraft) return;
        void run(async () => { onShortcut(await setShortcut(shortcutDraft)); setShortcutDraft(undefined); settings.refresh(); });
      }}>
        <label>{t('显示或隐藏窗口', 'Show or hide the window')}<input value={shortcutDraft ?? settings.data?.[1] ?? ''}
          onChange={(event) => setShortcutDraft(event.target.value)} placeholder="Alt+X"/></label>
        <button className="button primary" disabled={busy || !shortcutDraft?.trim()}>{t('保存', 'Save')}</button>
      </form>
    </section>
    <section className="settings-card">
      <h2><Bell size={19}/>{t('系统通知', 'System notifications')}</h2>
      <p className="muted">{t('提醒按本机时间送达，适用于当前 Windows 用户。', 'Reminders use this device’s local time and are delivered to the current Windows user.')}</p>
      <button className="button" disabled={busy} onClick={() => void run(async () => {
        await invoke('test_reminder_notification');
        onNotice(t('已请求发送测试通知', 'Test notification requested'));
      })}>{t('发送测试通知', 'Send test notification')}</button>
    </section>
    <section className="settings-card">
      <h2><RefreshCw size={19}/>{t('应用更新', 'App updates')}</h2>
      <button className="button" disabled={busy} onClick={() => void run(async () => {
        const result = await checkUpdate();
        if (result.hasUpdate) await openUrl(result.downloadUrl);
        else onNotice(t('当前已是最新版本', 'You’re up to date'));
      })}>{t('检查更新', 'Check for updates')}</button>
    </section>
  </div>;
}

const loadPinned = () => invoke<string | null>('get_setting', { key: 'always_on_top' });

function PinWindow({ onError }: { onError: (text: string) => void }) {
  const { t } = useLabels();
  const pin = useLoad('window-pin', loadPinned);
  const { reloadSettings } = useAppSettings();
  const { run, busy } = useOperation(onError);
  return <label className="setting">{t('窗口置顶', 'Always on top')}
    <input type="checkbox" checked={pin.data === 'true'} disabled={busy || pin.loading} onChange={(event) => {
      const value = event.target.checked;
      void run(async () => { await setAlwaysOnTop(value); await reloadSettings(); pin.refresh(); });
    }}/>
  </label>;
}
