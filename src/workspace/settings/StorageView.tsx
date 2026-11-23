import { open, save } from '@tauri-apps/plugin-dialog';
import { Cloud, Database, Download, FileUp, HardDrive } from 'lucide-react';
import { useState } from 'react';
import { getStorageStatus } from '../../api/settings';
import { commitImport, discoverSources, exportWorkspace, previewImport, switchWorkspace, type ImportPreview, type ImportResult } from '../bridge';
import { LoadState } from '../controls';
import { useLabels, useLoad, useOperation } from '../hooks';

export function StorageView({ workspace, onError, onNotice, onSwitching, onImported, modeOverride }: {
  workspace: number; onError: (text: string) => void; onNotice: (text: string) => void; onSwitching: (switching: boolean) => void; onImported?: () => void; modeOverride?: "local" | "remote";
}) {
  const { t } = useLabels();
  const { run, busy } = useOperation(onError);
  const status = useLoad(String(workspace), getStorageStatus);
  const sources = useLoad('sources', discoverSources);
  const [url, setUrl] = useState('');
  const [preview, setPreview] = useState<ImportPreview>();
  const [previewPath, setPreviewPath] = useState<string>();
  const [importResult, setImportResult] = useState<ImportResult>();
  const mode = modeOverride ?? status.data?.mode;

  const inspect = async (path: string | null) => {
    setPreview(undefined);
    setImportResult(undefined);
    setPreviewPath(path ?? t('当前连接的旧版 PostgreSQL 库', 'Legacy schema on the connected PostgreSQL server'));
    setPreview(await previewImport(path));
  };

  const changeStorage = (target: 'local' | 'remote') => run(async () => {
    onSwitching(true);
    try {
      await switchWorkspace(target === 'local' ? { mode: 'local' } : { mode: 'remote', url });
      setPreview(undefined);
      setImportResult(undefined);
      setUrl('');
      status.refresh();
      onNotice(target === 'local' ? t('已切换到本地工作区', 'Local workspace selected') : t('已连接 PostgreSQL 工作区', 'PostgreSQL workspace connected'));
    } finally {
      onSwitching(false);
    }
  });
  return <div className="settings-scroll">
    {!modeOverride && <section className="settings-card">
      <h2><Database size={19}/>{t('存储位置', 'Workspace storage')}</h2>
      {!status.data ? <LoadState error={status.error} retry={status.refresh}/> : <>
        <div className="storage-options">
          <button className="storage-mode" aria-pressed={mode === 'local'} disabled={busy || mode === 'local'}
            onClick={() => void changeStorage('local')}>
            <HardDrive size={20}/><span><strong>{t('本地 SQLite', 'Local SQLite')}</strong>
              <small>{t('保存在这台设备', 'Stored on this device')}</small></span>
          </button>
          <div className="storage-mode" data-active={mode === 'remote'}>
            <Cloud size={20}/><span><strong>PostgreSQL</strong>
              <small>{mode === 'remote' && status.data.health === 'connected'
                ? t('已连接', 'Connected') : t('使用自己的数据库', 'Use your own database')}</small></span>
          </div>
        </div>
        {status.data.health === 'failed' && <p className="inline-error">{status.data.message}</p>}
        <form className="storage-connect" onSubmit={(event) => { event.preventDefault(); void changeStorage('remote'); }}>
          <label>{t('PostgreSQL 连接地址', 'PostgreSQL connection URL')}
            <input type="password" required autoComplete="off" placeholder="postgresql://user@host/database"
              value={url} onChange={(event) => setUrl(event.target.value)}/>
          </label>
          <button className="button primary" disabled={busy || !url.trim()}>{t('连接并切换', 'Connect and switch')}</button>
        </form>
        <p className="muted">{t('切换存储会打开对应的工作区。要在工作区之间转移记录，请先导出，再切换并导入。', 'Each storage location has its own workspace. Export, switch storage, and import to transfer records.')}</p>
      </>}
    </section>}
    <section className="settings-card">
      <h2><FileUp size={19}/>{t('导入与恢复', 'Import and restore')}</h2>
      <p className="muted">{t('预览 SQLite、JSON 或 .scbackup 中的数据，再合并到当前工作区。已有记录会保留。', 'Preview SQLite, JSON, or .scbackup data and merge it into the current workspace. Existing records are kept.')}</p>
      <div className="button-row">
        <button className="button" disabled={busy || !mode} onClick={() => void run(async () => {
          const path = await open({
            multiple: false,
            filters: [{ name: 'SuperClipboard', extensions: ['db', 'sqlite', 'sqlite3', 'json', 'scbackup'] }],
          });
          if (path) await inspect(path);
        })}><FileUp size={16}/>{t('选择备份或数据库', 'Choose a backup or database')}</button>
        {mode === 'remote' && <button className="button" disabled={busy} onClick={() => void run(() => inspect(null))}>
          {t('预览旧版 PostgreSQL', 'Preview legacy PostgreSQL')}</button>}
      </div>
      {sources.error && <p className="inline-error">{sources.error}</p>}
      {(sources.data ?? []).map((path) => <button key={path} disabled={busy || !mode} className="source-button" onClick={() => void run(() => inspect(path))}>
        <Database size={17}/><span><strong>{t('发现旧版数据库', 'Older database found')}</strong><small>{path}</small></span>
      </button>)}
      {importResult && <p className="import-result" role="status">{t(
        `导入完成。剪贴板 ${importResult.clipboard} 条，备忘录 ${importResult.memos} 条，跳过重复 ${importResult.duplicates} 条。`,
        `Import complete. ${importResult.clipboard} clipboard items, ${importResult.memos} memos, ${importResult.duplicates} duplicates skipped.`,
      )}</p>}
      {preview && <div className="import-preview">
        <div className="section-label">{t('导入预览', 'Import preview')} · {preview.format} · {preview.appVersion}</div>
        {previewPath && <p className="muted">{previewPath}</p>}
        <div className="stats-grid">
          {[
            [preview.counts.clipboard, t('剪贴板', 'Clipboard')],
            [preview.counts.memos, t('备忘录', 'Memos')],
            [preview.counts.images, t('图片记录', 'Images')],
            [preview.counts.favorites, t('已收藏', 'Favorites')],
            [preview.counts.archived, t('回收站', 'In Trash')],
          ].map(([count, label]) => <div key={label}><strong>{count}</strong><span>{label}</span></div>)}
        </div>
        {preview.sample.length > 0 && <ul className="sample-list">{preview.sample.map((item, index) =>
          <li key={index}>{item.label || t('无标题', 'Untitled')}</li>)}</ul>}
        <div className="button-row">
          <button className="button primary" disabled={busy} onClick={() => void run(async () => {
            const result = await commitImport(preview.token);
            setPreview(undefined);
            setImportResult(result);
            onImported?.();
          })}>{t('合并到当前工作区', 'Merge into workspace')}</button>
          <button className="button" disabled={busy} onClick={() => setPreview(undefined)}>{t('关闭预览', 'Close preview')}</button>
        </div>
      </div>}
    </section>
    <section className="settings-card">
      <h2><Download size={19}/>{t('导出备份', 'Export backup')}</h2>
      <p className="muted">{t('将当前工作区的历史、备忘录和图片保存为带校验的 .scbackup。', 'Save this workspace’s history, memos, and images to a checksummed .scbackup.')}</p>
      <button className="button" disabled={busy || !mode} onClick={() => void run(async () => {
        const path = await save({
          defaultPath: `SuperClipboard-${new Date().toISOString().replace(/[:.]/g, '-')}.scbackup`,
          filters: [{ name: 'SuperClipboard', extensions: ['scbackup'] }],
        });
        if (!path) return;
        await exportWorkspace(path);
        onNotice(t('备份已保存', 'Backup saved'));
      })}><Download size={16}/>{t('保存备份', 'Save backup')}</button>
    </section>
  </div>;
}
