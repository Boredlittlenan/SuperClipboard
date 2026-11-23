import { getCurrentWindow } from '@tauri-apps/api/window';
import { Bell, ClipboardList, Database, HardDrive, Minus, Pause, PencilLine, Play, Settings2, Square, Star, StickyNote, Trash2, X } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { getSettings, getShortcut } from '../api/settings';
import { useAppSettings } from '../hooks/useAppSettings';
import { useClipboardDropImport } from '../hooks/useClipboardDropImport';
import { IconButton } from './controls';
import { useDesktopEvent, useLabels, useLoad, useOperation } from './hooks';
import { describeError, type View } from './model';
import { RecordBrowser } from './RecordBrowser';
import { RemindersView } from './Reminders';
import { SettingsView, type Theme } from './settings/SettingsView';
import { StorageView } from './settings/StorageView';
import './workspace.css';

const loadPreferences = () => Promise.all([
  getSettings(['theme_mode', 'capture_paused', 'storage_mode']),
  getShortcut(),
]);

export function Workspace() {
  const { t } = useLabels();
  const { setAppSetting } = useAppSettings();
  const [view, setView] = useState<View>('history');
  const [workspace, setWorkspace] = useState(0);
  const [memo, setMemo] = useState<number | 'new' | null>(null);
  const [browseRevision, setBrowseRevision] = useState(0);
  const [quick, setQuick] = useState(false);
  const [switching, setSwitching] = useState(false);
  const [focusVersion, setFocusVersion] = useState(0);
  const [notice, setNotice] = useState<{ text: string; error: boolean }>();
  const preferences = useLoad('preferences', loadPreferences);
  const refreshPreferences = preferences.refresh;
  const [settings, shortcut = 'Alt+X'] = preferences.data ?? [];
  const theme = (settings?.theme_mode ?? 'system') as Theme;
  const paused = settings?.capture_paused === 'true';
  const remote = settings?.storage_mode === 'remote';
  const onError = useCallback((text: string) => setNotice({ text, error: true }), []);
  const onNotice = useCallback((text: string) => setNotice({ text, error: false }), []);
  const { run, busy } = useOperation(onError);

  const navigate = useCallback((next: View) => {
    if (switching) return;
    setView(next);
    setMemo(null);
    setQuick(false);
  }, [switching]);
  const openMemo = useCallback((id: number | 'new') => {
    setMemo(id);
    setView('memos');
    setBrowseRevision((value) => value + 1);
  }, []);
  const storageChanged = useCallback(() => {
    setWorkspace((value) => value + 1);
    refreshPreferences();
  }, [refreshPreferences]);
  const windowShown = useCallback((source: string) => {
    setQuick(source === 'shortcut');
    if (source === 'shortcut' && !switching) {
      setView('history');
      setFocusVersion((value) => value + 1);
    }
  }, [switching]);
  useDesktopEvent('workspace-changed', storageChanged);
  useDesktopEvent('window-shown', windowShown);
  const openSettings = useCallback(() => navigate('settings'), [navigate]);
  useDesktopEvent('open-settings', openSettings);
  const captureError = useCallback(() => onError(t('剪贴板写入失败。请检查工作区存储连接。', 'Clipboard capture failed. Check the workspace storage connection.')), [onError, t]);
  useDesktopEvent('clipboard-storage-error', captureError);
  const archivedDuplicate = useCallback(() => onNotice(t('相同内容已在回收站，可从回收站恢复。', 'This content is already in Trash. You can restore it there.')), [onNotice, t]);
  useDesktopEvent('clipboard-archived-duplicate', archivedDuplicate);
  useEffect(() => {
    if (!notice || notice.error) return;
    const timer = window.setTimeout(() => setNotice(undefined), 4800);
    return () => window.clearTimeout(timer);
  }, [notice]);
  useEffect(() => {
    const keyboard = (event: KeyboardEvent) => {
      if (event.ctrlKey && event.key.toLowerCase() === 'f') {
        event.preventDefault();
        if (view === 'settings' || view === 'storage' || view === 'reminders') navigate('history');
        setFocusVersion((value) => value + 1);
      }
      if (event.key === 'Escape' && !event.defaultPrevented && quick) {
        void getCurrentWindow().hide().catch((error) => onError(describeError(error)));
      }
    };
    window.addEventListener('keydown', keyboard);
    return () => window.removeEventListener('keydown', keyboard);
  }, [navigate, onError, quick, view]);

  const { isDragActive, isImporting, ...dropHandlers } = useClipboardDropImport({
    onComplete: (inserted) => onNotice(inserted ? t('已收录拖入的内容', 'Dropped content captured') : t('已复制到系统剪贴板', 'Copied to the system clipboard')),
    onError: () => onError(t('无法收录拖入的内容，请检查文件类型和大小。', 'Could not capture the drop. Check the file type and size.')),
  });
  const dropEvents = view === 'history' ? {
    onDragEnter: dropHandlers.handleDragEnter,
    onDragOver: dropHandlers.handleDragOver,
    onDragLeave: dropHandlers.handleDragLeave,
    onDrop: dropHandlers.handleDrop,
  } : {};
  const navigation = {
    history: [ClipboardList, t('剪贴板历史', 'Clipboard history')],
    memos: [StickyNote, t('备忘录', 'Memos')],
    favorites: [Star, t('收藏', 'Favorites')],
    reminders: [Bell, t('提醒', 'Reminders')],
    trash: [Trash2, t('回收站', 'Trash')],
    storage: [Database, t('存储与导入', 'Storage & import')],
    settings: [Settings2, t('设置', 'Settings')],
  } as const;
  const link = (name: View) => {
    const [Icon, label] = navigation[name];
    return <button key={name} className="navigation-item" aria-current={view === name ? 'page' : undefined}
      title={label} aria-label={label} disabled={switching} onClick={() => navigate(name)}><Icon size={18}/><span>{label}</span></button>;
  };

  return <div className="workspace" data-theme={theme} {...dropEvents}>
    <aside className="sidebar">
      <div className="brand" data-tauri-drag-region><img src="/app-icon.png" alt=""/><strong>SuperClipboard</strong></div>
      <nav aria-label={t('工作区导航', 'Workspace navigation')}>
        <div className="navigation-main">{(['history', 'memos', 'favorites', 'reminders', 'trash'] as const).map(link)}</div>
        <div className="navigation-settings">{(['storage', 'settings'] as const).map(link)}</div>
      </nav>
      <div className="sidebar-footer"><span className="status-dot" data-paused={paused}/><span>{paused ? t('已暂停捕获', 'Capture paused') : t('剪贴板监控中', 'Capture on')}</span></div>
    </aside>
    <main className="main-pane">
      <header className="titlebar" data-tauri-drag-region>
        <div className="location" data-tauri-drag-region><HardDrive size={15}/>{remote ? 'PostgreSQL' : t('本地工作区', 'Local workspace')}</div>
        <div className="window-actions">
          <IconButton label={t('最小化', 'Minimize')} onClick={() => void run(() => getCurrentWindow().minimize())}><Minus size={17}/></IconButton>
          <IconButton label={t('最大化或还原', 'Maximize or restore')} onClick={() => void run(() => getCurrentWindow().toggleMaximize())}><Square size={13}/></IconButton>
          <IconButton label={t('隐藏到托盘', 'Hide to tray')} onClick={() => void run(() => getCurrentWindow().hide())}><X size={17}/></IconButton>
        </div>
      </header>
      <div className="view-heading">
        <h1>{navigation[view][1]}</h1>
        {view === 'memos' && <button className="button primary" onClick={() => openMemo('new')}><PencilLine size={17}/>{t('新建备忘录', 'New memo')}</button>}
        {view === 'history' && <button className="button" disabled={busy} onClick={() => void run(async () => {
          await setAppSetting('capturePaused', !paused); preferences.refresh();
        })}>{paused ? <Play size={16}/> : <Pause size={16}/>}<span>{paused ? t('继续捕获', 'Resume capture') : t('暂停捕获', 'Pause capture')}</span></button>}
      </div>
      {view === 'settings' ? <SettingsView theme={theme} paused={paused} onTheme={preferences.refresh} onPause={preferences.refresh}
        onShortcut={preferences.refresh} onError={onError} onNotice={onNotice}/>
        : view === 'storage' ? <StorageView workspace={workspace} onError={onError} onNotice={onNotice} onSwitching={setSwitching}/>
        : view === 'reminders' ? <RemindersView workspace={workspace} openMemo={openMemo} onError={onError}/>
        : <RecordBrowser key={`${workspace}:${view}:${browseRevision}`} view={view} workspace={workspace} initialMemo={memo}
          focusVersion={focusVersion} quick={quick} openMemo={openMemo} openStorage={() => navigate('storage')} onError={onError} onNotice={onNotice}/>}
      <footer className="workspace-footer"><span>{isImporting ? t('正在收录…', 'Capturing…') : 'SuperClipboard 4.0'}</span>
        <span>{t('全局快捷键', 'Global shortcut')} <kbd>{shortcut}</kbd></span></footer>
    </main>
    {isDragActive && view === 'history' && <div className="drop-indicator">{t('松开以收录到剪贴板', 'Drop to capture')}</div>}
    {notice && <div className="toast" role={notice.error ? 'alert' : 'status'} data-error={notice.error}>
      <span>{notice.text}</span><IconButton label={t('关闭消息', 'Dismiss message')} onClick={() => setNotice(undefined)}><X size={15}/></IconButton>
    </div>}
  </div>;
}
