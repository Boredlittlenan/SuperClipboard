import { ask, save } from '@tauri-apps/plugin-dialog';
import { ArchiveRestore, ClipboardPaste, Copy, Download, FileText, Pencil, Star, Trash2, X } from 'lucide-react';
import { useCallback, useState } from 'react';
import { copyToClipboard, deleteEntry, exportClipboardImage, permanentDelete, togglePin, unarchiveEntry } from '../api/clipboard';
import { createMemo, deleteMemo, permanentDeleteMemo, toggleMemoPin, unarchiveMemo } from '../api/memos';
import { pasteToActiveWindow } from '../api/settings';
import { renderMemoBody } from '../components/MemoBody';
import { getItem } from './bridge';
import { EmptyState, IconButton, LoadState } from './controls';
import { Editor } from './Editor';
import { useLabels, useLoad, useOperation, useReminders } from './hooks';
import { categoryName, formatDate, itemTitle, type Item, type RecordKind } from './model';
import { ReminderForm } from './Reminders';

interface Props {
  id: number;
  kind: RecordKind;
  workspace: number;
  quick: boolean;
  onError: (text: string) => void;
  onNotice: (text: string) => void;
  onChanged: () => void;
  onClose: () => void;
  openMemo: (id: number) => void;
}

export function ItemDetail(props: Props) {
  const { id, kind, workspace, onChanged, onClose } = props;
  const { t } = useLabels();
  const [editing, setEditing] = useState(false);
  const load = useCallback(() => getItem(kind, id), [id, kind]);
  const resource = useLoad(`${kind}:${id}:${workspace}`, load);
  const item = resource.data;
  const changed = () => { resource.refresh(); onChanged(); };
  if (item === undefined) return <LoadState error={resource.error} retry={resource.refresh}/>;
  if (!item) return <EmptyState icon={<FileText size={23}/>} title={t('条目已移除', 'Item unavailable')}
    action={<button className="button" onClick={onClose}>{t('返回列表', 'Back to list')}</button>}/>;
  return editing
    ? <Editor item={item} onError={props.onError} onCancel={() => setEditing(false)}
      onSaved={() => { setEditing(false); changed(); }}/>
    : <DisplayItem {...props} item={item} onEdit={() => setEditing(true)} onChanged={changed}/>;
}

function DisplayItem({ item, onEdit, quick, workspace, onError, onNotice, onChanged, onClose, openMemo }: Props & {
  item: Item; onEdit: () => void;
}) {
  const { t, locale } = useLabels();
  const { run, busy } = useOperation(onError);
  const data = item.data;
  const id = data.id;
  const archived = Boolean(data.archived_at);
  const image = item.kind === 'clipboard' && item.data.content_type.startsWith('image/');
  const requireResult = (result: boolean) => {
    if (!result) throw new Error(t('条目已更改，请刷新后重试。', 'This item changed. Refresh and try again.'));
  };
  const copy = (original = false) => run(async () => {
    requireResult(await copyToClipboard(id, original));
    onNotice(t('已复制到剪贴板', 'Copied to clipboard'));
  });
  const trash = () => run(async () => {
    if (archived && !await ask(t('永久删除这条记录？', 'Permanently delete this item?'), { kind: 'warning' })) return;
    const deleted = item.kind === 'memo'
      ? await (archived ? permanentDeleteMemo(id) : deleteMemo(id, true))
      : await (archived ? permanentDelete(id) : deleteEntry(id, true));
    requireResult(deleted);
    onChanged();
    onClose();
    onNotice(archived ? t('已永久删除', 'Permanently deleted') : t('已移至回收站', 'Moved to Trash'));
  });
  const restore = () => run(async () => {
    requireResult(await (item.kind === 'memo' ? unarchiveMemo(id) : unarchiveEntry(id)));
    onChanged();
    onClose();
    onNotice(t('已恢复条目', 'Item restored'));
  });
  const favorite = () => run(async () => {
    await (item.kind === 'memo' ? toggleMemoPin(id) : togglePin(id));
    onChanged();
  });
  const download = () => run(async () => {
    const path = await save({ defaultPath: `clipboard-${id}.png`, filters: [{ name: 'PNG', extensions: ['png'] }] });
    if (path) { await exportClipboardImage(id, path); onNotice(t('已保存图片', 'Image saved')); }
  });
  const convert = () => run(async () => {
    if (item.kind !== 'clipboard') return;
    const value = item.data;
    const body = image ? `![image](data:${value.content_type};base64,${value.content})` : value.content;
    const memo = await createMemo(image ? t('剪贴板图片', 'Clipboard image') : value.preview.slice(0, 80), body, '');
    onNotice(t('已创建备忘录', 'Memo created'));
    openMemo(memo.id);
  });
  return <article className="record-detail">
    <div className="detail-top">
      <span className="section-label">{item.kind === 'memo' ? t('备忘录', 'Memo') : categoryName(item.data.category, t)}</span>
      <IconButton label={t('关闭详情', 'Close details')} onClick={onClose}><X size={17}/></IconButton>
    </div>
    <h2>{item.kind === 'memo' ? itemTitle(item, t) : t('条目详情', 'Item details')}</h2>
    <p className="muted detail-date">{formatDate(data.archived_at ?? data.created_at, locale, true)}</p>
    <div className="button-row detail-toolbar">
      {archived ? <button className="button primary" onClick={() => void restore()} disabled={busy}><ArchiveRestore size={16}/>{t('恢复', 'Restore')}</button>
        : <>
          {item.kind === 'clipboard' && <button className="button primary" disabled={busy} onClick={() => void (quick
            ? run(async () => requireResult(await pasteToActiveWindow(id))) : copy())}>
            {quick ? <ClipboardPaste size={16}/> : <Copy size={16}/>}
            {quick ? t('粘贴', 'Paste') : t('复制', 'Copy')}
          </button>}
          {quick && item.kind === 'clipboard' && <IconButton label={t('复制', 'Copy')} disabled={busy} onClick={() => void copy()}><Copy size={16}/></IconButton>}
          {!image && <IconButton label={t('编辑', 'Edit')} disabled={busy} onClick={onEdit}><Pencil size={16}/></IconButton>}
          <IconButton label={data.pinned ? t('取消收藏', 'Remove favorite') : t('收藏', 'Favorite')}
            aria-pressed={data.pinned} disabled={busy} onClick={() => void favorite()}><Star size={17} fill={data.pinned ? 'currentColor' : 'none'}/></IconButton>
          {item.kind === 'clipboard' && <IconButton label={t('转为备忘录', 'Save as memo')} disabled={busy} onClick={() => void convert()}><FileText size={16}/></IconButton>}
        </>}
      {image && <IconButton label={t('保存图片', 'Save image')} disabled={busy} onClick={() => void download()}><Download size={16}/></IconButton>}
      <IconButton label={archived ? t('永久删除', 'Delete permanently') : t('移至回收站', 'Move to Trash')} className="icon-button danger"
        disabled={busy} onClick={() => void trash()}><Trash2 size={17}/></IconButton>
    </div>
    {item.kind === 'memo' ? <>
      {item.data.tags && <p className="detail-tags">{item.data.tags}</p>}
      <div className="record-content">{renderMemoBody(item.data.body, Infinity, 520)}</div>
      {!archived && <MemoReminder id={id} workspace={workspace} onError={onError}/>}
    </> : <>
      {image
        ? <div className="image-preview"><img src={`data:${item.data.content_type};base64,${item.data.content}`} alt={itemTitle(item, t)}/></div>
        : <pre className="record-content">{item.data.content}</pre>}
      {item.data.original_content && <details className="original-content">
        <summary>{t('查看原始内容', 'Show original')}</summary>
        <pre className="record-content">{item.data.original_content}</pre>
        <button className="button" onClick={() => void copy(true)} disabled={busy}><Copy size={15}/>{t('复制原文', 'Copy original')}</button>
      </details>}
    </>}
  </article>;
}

function MemoReminder({ id, workspace, onError }: { id: number; workspace: number; onError: (text: string) => void }) {
  const reminders = useReminders(workspace);
  return reminders.data
    ? <ReminderForm memoId={id} reminder={reminders.data.find((item) => item.memoId === id)} onChange={reminders.refresh} onError={onError}/>
    : <LoadState error={reminders.error} retry={reminders.refresh}/>;
}
