import { useState } from 'react';
import MemoRichEditor from '../components/MemoRichEditor';
import { createMemo, updateMemo } from '../api/memos';
import { updateEntry } from '../api/clipboard';
import { useLabels, useOperation } from './hooks';
import type { Item } from './model';

export function Editor({ item, onCancel, onSaved, onError }: {
  item?: Item; onCancel: () => void; onSaved: (id: number) => void; onError: (text: string) => void;
}) {
  const { t } = useLabels();
  const [title, setTitle] = useState(item?.kind === 'memo' ? item.data.title : '');
  const [body, setBody] = useState((item?.kind === 'memo' ? item.data.body : item?.data.content) ?? '');
  const [tags, setTags] = useState(item?.kind === 'memo' ? item.data.tags : '');
  const { run, busy } = useOperation(onError);
  const isMemo = item?.kind !== 'clipboard';
  const canSave = isMemo ? Boolean(title.trim() || body.trim()) : Boolean(body.trim());
  function submit() {
    if (!canSave) return;
    void run(async () => {
      if (!item) {
        const memo = await createMemo(title, body, tags);
        onSaved(memo.id);
        return;
      }
      const result = item.kind === 'memo'
        ? await updateMemo(item.data.id, title, body, tags, item.data.version)
        : await updateEntry(item.data.id, body, item.data.version);
      if (result.conflict || !result.updated) {
        throw new Error(t('条目已经更改或移除。你的输入仍在编辑区，可以先复制再重新打开。', 'This item changed or was removed. Your draft remains here and can be copied before reopening.'));
      }
      onSaved(item.data.id);
    });
  }
  return <form className="detail-editor" onSubmit={(event) => { event.preventDefault(); submit(); }}>
    <h2>{item ? t('编辑条目', 'Edit item') : t('新建备忘录', 'New memo')}</h2>
    {isMemo && <label>{t('标题', 'Title')}<input autoFocus value={title} maxLength={250}
      placeholder={t('给备忘录取个名字', 'Name this memo')} onChange={(event) => setTitle(event.target.value)}/></label>}
    <div className="field">
      <span>{t('内容', 'Content')}</span>
      {isMemo ? <MemoRichEditor body={body} onChange={setBody} placeholder={t('输入文字或粘贴图片', 'Write here or paste an image')}
        dragLabel={t('移动图片', 'Move image')} deleteLabel={t('删除图片', 'Remove image')} initialHeight={190}
        onEscape={onCancel} onSave={submit}/>
        : <textarea autoFocus value={body} onChange={(event) => setBody(event.target.value)} rows={9}/>}
    </div>
    {isMemo && <label>{t('标签', 'Tags')}<input value={tags}
      placeholder={t('以逗号分隔多个标签', 'Separate tags with commas')} onChange={(event) => setTags(event.target.value)}/></label>}
    <div className="button-row">
      <button className="button primary" disabled={busy || !canSave}>{t('保存', 'Save')}</button>
      <button className="button" disabled={busy} type="button" onClick={onCancel}>{t('取消', 'Cancel')}</button>
    </div>
  </form>;
}
