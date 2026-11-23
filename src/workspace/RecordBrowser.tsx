import { ask } from '@tauri-apps/plugin-dialog';
import { Archive, ChevronLeft, ChevronRight, Clipboard, Code, FileText, Folder, Image, Link, ListChecks, Mail, Search, Star, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { copyToClipboard, deleteEntries, emptyArchive, mergeEntries } from '../api/clipboard';
import { emptyMemoArchive } from '../api/memos';
import { pasteToActiveWindow } from '../api/settings';
import { canMergeEntries } from '../clipboardMerge';
import type { Category } from '../types';
import { listItems } from './bridge';
import { EmptyState, IconButton, LoadState } from './controls';
import { Editor } from './Editor';
import { useDebounced, useDesktopEvent, useLabels, useLoad, useOperation } from './hooks';
import { ItemDetail } from './ItemDetail';
import { categories, categoryName, formatDate, itemTitle, type Item, type RecordKind, type RecordView } from './model';

const PAGE_SIZE = 40;
const categoryIcons = { text: FileText, link: Link, image: Image, code: Code, email: Mail, file_path: Folder };

interface Props {
  view: RecordView;
  workspace: number;
  quick: boolean;
  focusVersion: number;
  initialMemo: number | 'new' | null;
  openMemo: (id: number) => void;
  openStorage: () => void;
  onError: (text: string) => void;
  onNotice: (text: string) => void;
}

export function RecordBrowser(props: Props) {
  const { view, workspace, quick, focusVersion, initialMemo, openMemo, openStorage, onError, onNotice } = props;
  const { t, locale } = useLabels();
  const [kindChoice, setKindChoice] = useState<RecordKind>('clipboard');
  const kind = view === 'memos' ? 'memo' : view === 'history' ? 'clipboard' : kindChoice;
  const [category, setCategory] = useState<Category>();
  const [query, setQuery] = useState('');
  const search = useDebounced(query);
  const [page, setPage] = useState(0);
  const [selected, setSelected] = useState<number | 'new' | null>(view === 'memos' ? initialMemo : null);
  const [selecting, setSelecting] = useState(false);
  const [checked, setChecked] = useState<number[]>([]);
  const searchRef = useRef<HTMLInputElement>(null);
  const { run, busy } = useOperation(onError);
  const archived = view === 'trash';
  const favorites = view === 'favorites';
  const load = useCallback(() => listItems(kind, {
    archived, favorites, search, category, limit: PAGE_SIZE + 1, offset: page * PAGE_SIZE,
  }), [kind, archived, favorites, search, category, page]);
  const resource = useLoad(JSON.stringify([workspace, kind, view, search, category, page]), load);

  useDesktopEvent('clipboard-changed', resource.refresh);
  useDesktopEvent('remote-storage-changed', resource.refresh);
  useEffect(() => { if (focusVersion) searchRef.current?.focus(); }, [focusVersion]);

  const items = resource.data?.slice(0, PAGE_SIZE) ?? [];
  const selectedEntries = items.flatMap((item) => item.kind === 'clipboard' && checked.includes(item.data.id) ? [item.data] : []);
  const updateScope = () => { setPage(0); setSelected(null); setChecked([]); };
  const toggleChecked = (id: number) => setChecked((ids) => ids.includes(id) ? ids.filter((value) => value !== id) : [...ids, id]);
  const copy = (item: Item) => run(async () => {
    if (item.kind !== 'clipboard' || archived) return;
    if (!await (quick ? pasteToActiveWindow(item.data.id) : copyToClipboard(item.data.id))) {
      throw new Error(t('条目已移除', 'Item unavailable'));
    }
    if (!quick) onNotice(t('已复制到剪贴板', 'Copied to clipboard'));
  });
  const handleClear = () => run(async () => {
    if (!await ask(t('永久删除当前分组的所有回收站条目？', 'Permanently delete every item in this Trash group?'), { kind: 'warning' })) return;
    const count = await (kind === 'memo' ? emptyMemoArchive() : emptyArchive());
    updateScope();
    resource.refresh();
    onNotice(t(`已永久删除 ${count} 条`, `Permanently deleted ${count} items`));
  });
  const moveSelection = () => run(async () => {
    const count = await deleteEntries(checked, true);
    setChecked([]);
    resource.refresh();
    onNotice(t(`已移至回收站 ${count} 条`, `Moved ${count} items to Trash`));
  });
  const mergeSelection = () => run(async () => {
    const result = await mergeEntries(selectedEntries.map((item) => item.id), t('合并的图片', 'Merged images'));
    setChecked([]);
    resource.refresh();
    onNotice(result.created ? t('已合并条目', 'Items merged') : t('已有相同条目', 'An identical item already exists'));
  });

  const scopeTabs = (view === 'favorites' || archived) && <div className="filters" aria-label={t('记录类型', 'Record type')}>
    <button className="chip" aria-pressed={kind === 'clipboard'} onClick={() => { setKindChoice('clipboard'); updateScope(); }}>{t('剪贴板', 'Clipboard')}</button>
    <button className="chip" aria-pressed={kind === 'memo'} onClick={() => { setKindChoice('memo'); updateScope(); }}>{t('备忘录', 'Memos')}</button>
  </div>;

  return <div className={`record-browser ${selected !== null ? 'has-detail' : ''}`}>
    <section className="records-pane" aria-label={t('记录列表', 'Records')}>
      <div className="records-tools">
        <div className="search-field">
          <Search size={18}/>
          <input ref={searchRef} value={query} aria-label={t('搜索', 'Search')} placeholder={kind === 'memo'
            ? t('搜索标题、内容、标签…', 'Search titles, content, or tags…') : t('搜索剪贴板内容…', 'Search clipboard…')}
            onChange={(event) => { setQuery(event.target.value); updateScope(); }}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && items[0]) {
                if (search !== query.trim()) return;
                if (quick && !archived) void copy(items[0]);
                else setSelected(items[0].data.id);
              }
              if (event.key === 'ArrowDown') {
                event.preventDefault();
                document.querySelector<HTMLButtonElement>('.item-row')?.focus();
              }
            }}/>
          <kbd>Ctrl F</kbd>
        </div>
        {view === 'history' && <IconButton label={t('多选', 'Select multiple')} aria-pressed={selecting}
          onClick={() => { setSelecting(!selecting); setChecked([]); }}><ListChecks size={19}/></IconButton>}
        {archived && <IconButton label={t('清空当前回收站', 'Empty this Trash group')} disabled={busy || !items.length}
          onClick={() => void handleClear()}><Trash2 size={18}/></IconButton>}
      </div>
      {scopeTabs}
      {view === 'history' && <div className="filters" aria-label={t('分类', 'Categories')}>
        <button className="chip" aria-pressed={!category} onClick={() => { setCategory(undefined); updateScope(); }}>{t('全部', 'All')}</button>
        {categories.map((value) => <button className="chip" key={value} aria-pressed={category === value}
          onClick={() => { setCategory(value); updateScope(); }}>{categoryName(value, t)}</button>)}
      </div>}
      {selecting && <div className="batch-toolbar">
        <span>{t(`已选 ${checked.length} 项`, `${checked.length} selected`)}</span>
        <button className="button" disabled={busy || !canMergeEntries(selectedEntries)} onClick={() => void mergeSelection()}>{t('合并', 'Merge')}</button>
        <button className="button" disabled={busy || !checked.length} onClick={() => void moveSelection()}>{t('移至回收站', 'Move to Trash')}</button>
      </div>}
      <div className="items-scroll">
        {!resource.data ? <LoadState error={resource.error} retry={resource.refresh}/>
          : items.length === 0 ? <EmptyState icon={query ? <Search size={24}/> : <Archive size={24}/>}
            title={query ? t('没有搜索结果', 'No matches') : t('暂无记录', 'No items yet')}
            message={view === 'history' && !query && !category ? t('复制或拖入文本、链接和图片即可开始。', 'Copy or drop text, links, or images to get started.') : undefined}
            action={view === 'history' && !query && !category
              ? <button className="button" onClick={openStorage}>{t('导入旧版数据', 'Import older data')}</button> : undefined}/>
          : <ul className="items">
            {items.map((item, index) => {
              const id = item.data.id;
              const Icon = item.kind === 'memo' ? FileText : categoryIcons[item.data.category] ?? Clipboard;
              const badge = item.kind === 'memo' ? item.data.tags || t('备忘录', 'Memo') : categoryName(item.data.category, t);
              return <li key={id}>
                {selecting && <input className="row-check" type="checkbox" aria-label={itemTitle(item, t)} checked={checked.includes(id)} onChange={() => toggleChecked(id)}/>}
                <button className="item-row" aria-pressed={selected === id} onClick={(event) => {
                  if (view === 'history' && (selecting || event.ctrlKey)) { setSelecting(true); toggleChecked(id); }
                  else setSelected(id);
                }} onDoubleClick={() => { if (!selecting) void copy(item); }}
                onKeyDown={(event) => {
                  if (event.key === 'Enter' && !selecting && quick && item.kind === 'clipboard' && !archived) {
                    event.preventDefault(); void copy(item);
                  }
                  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
                    event.preventDefault();
                    document.querySelectorAll<HTMLButtonElement>('.item-row')[index + (event.key === 'ArrowDown' ? 1 : -1)]?.focus();
                  }
                }}>
                  <span className={`type-icon ${item.kind === 'memo' ? 'memo' : item.data.category}`}><Icon size={18}/></span>
                  <span className="item-summary">
                    <span className="item-title">{itemTitle(item, t)}</span>
                    <span className="item-meta">{badge}</span>
                  </span>
                  {item.data.pinned && <Star size={14} className="item-star" fill="currentColor" aria-label={t('已收藏', 'Favorite')}/>}
                  <span className="item-date">{formatDate(item.data.archived_at ?? item.data.created_at, locale)}</span>
                </button>
              </li>;
            })}
          </ul>}
      </div>
      <footer className="pagination">
        <span>{t(`第 ${page + 1} 页`, `Page ${page + 1}`)}</span>
        <IconButton label={t('上一页', 'Previous page')} disabled={page === 0}
          onClick={() => { setPage(page - 1); setChecked([]); }}><ChevronLeft size={16}/></IconButton>
        <IconButton label={t('下一页', 'Next page')} disabled={(resource.data?.length ?? 0) <= PAGE_SIZE}
          onClick={() => { setPage(page + 1); setChecked([]); }}><ChevronRight size={16}/></IconButton>
      </footer>
    </section>
    <section className="detail-pane" aria-label={t('详情', 'Details')}>
      {selected === 'new' ? <Editor onCancel={() => setSelected(null)} onSaved={(id) => { updateScope(); setSelected(id); resource.refresh(); }} onError={onError}/>
        : selected !== null ? <ItemDetail key={`${kind}:${selected}`} id={selected} kind={kind} quick={quick} workspace={workspace}
          onClose={() => setSelected(null)} onChanged={resource.refresh} onError={onError} onNotice={onNotice} openMemo={openMemo}/>
          : <EmptyState icon={<Clipboard size={25}/>} title={t('选择一条记录', 'Select an item')}
            message={t('在这里查看内容并进行操作。', 'Read and act on the selected item here.')}/>}
    </section>
  </div>;
}
