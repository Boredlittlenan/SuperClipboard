import type { Memo } from './types';

/** Keep a newly created draft directly below the New Memo button while it is edited. */
export function orderMemosForDisplay(memos: Memo[], newMemoId: number | null): Memo[] {
  const draft = newMemoId === null ? undefined : memos.find((memo) => memo.id === newMemoId);
  const remaining = draft ? memos.filter((memo) => memo.id !== draft.id) : memos;
  const pinned = remaining.filter((memo) => memo.pinned);
  const unpinned = remaining.filter((memo) => !memo.pinned);
  return draft ? [draft, ...pinned, ...unpinned] : [...pinned, ...unpinned];
}
