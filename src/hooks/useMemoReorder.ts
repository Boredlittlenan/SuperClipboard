import { useCallback, useEffect, useRef, useState } from 'react';
import type { Dispatch, PointerEvent as ReactPointerEvent, SetStateAction } from 'react';
import { reorderMemos } from '../api/memos';
import { hasMemoImage } from '../components/MemoBody';
import type { Memo } from '../types';

interface MemoReorderState {
  draggedId: number | null;
  dragOverId: number | null;
  dragGhostPos: { x: number; y: number } | null;
  dragGhostContent: string;
  handlePointerDown: (event: ReactPointerEvent, id: number) => void;
}

export function useMemoReorder(
  memos: Memo[],
  setMemos: Dispatch<SetStateAction<Memo[]>>,
  enabled: boolean,
  refresh: () => void,
): MemoReorderState {
  const [draggedId, setDraggedId] = useState<number | null>(null);
  const [dragOverId, setDragOverId] = useState<number | null>(null);
  const [dragGhostPos, setDragGhostPos] = useState<{ x: number; y: number } | null>(null);
  const [dragGhostContent, setDragGhostContent] = useState('');
  const draggedIdRef = useRef<number | null>(null);
  const dragOverIdRef = useRef<number | null>(null);

  const updateDragOverId = useCallback((id: number | null) => {
    dragOverIdRef.current = id;
    setDragOverId(id);
  }, []);

  const handlePointerDown = useCallback((event: ReactPointerEvent, id: number) => {
    if (!enabled) return;
    const memo = memos.find(item => item.id === id);
    if (!memo) return;

    event.preventDefault();
    event.stopPropagation();
    draggedIdRef.current = id;
    setDraggedId(id);
    setDragGhostPos({ x: event.clientX, y: event.clientY });
    setDragGhostContent(
      memo.title || (hasMemoImage(memo.body) ? '[image]' : memo.body.slice(0, 40)) || '(untitled)',
    );
  }, [enabled, memos]);

  useEffect(() => {
    if (draggedId === null) return;

    const handlePointerMove = (event: PointerEvent) => {
      event.preventDefault();
      setDragGhostPos({ x: event.clientX, y: event.clientY });

      const ghost = document.querySelector<HTMLElement>('.memo-drag-ghost');
      if (ghost) ghost.style.display = 'none';
      const target = document.elementFromPoint(event.clientX, event.clientY);
      if (ghost) ghost.style.display = '';

      const entry = target?.closest<HTMLElement>('.memo-entry[data-memo-id]');
      const targetId = Number(entry?.dataset.memoId ?? 0);
      const canDrop = targetId !== 0
        && targetId !== draggedIdRef.current
        && !memos.find(item => item.id === targetId)?.pinned;
      updateDragOverId(canDrop ? targetId : null);
    };

    const handlePointerUp = () => {
      const sourceId = draggedIdRef.current;
      const targetId = dragOverIdRef.current;
      if (sourceId !== null && targetId !== null && sourceId !== targetId) {
        const unpinned = memos.filter(memo => !memo.pinned);
        const sourceIndex = unpinned.findIndex(memo => memo.id === sourceId);
        const targetIndex = unpinned.findIndex(memo => memo.id === targetId);
        if (sourceIndex !== -1 && targetIndex !== -1) {
          const reordered = [...unpinned];
          const [moved] = reordered.splice(sourceIndex, 1);
          reordered.splice(targetIndex, 0, moved);

          const maxOrder = Math.max(...unpinned.map(memo => memo.sort_order), 0);
          const orders = reordered.map((memo, index) => ({
            id: memo.id,
            sort_order: maxOrder - index,
          }));
          const orderById = new Map(orders.map(order => [order.id, order.sort_order]));
          setMemos(current => current
            .map(memo => orderById.has(memo.id)
              ? { ...memo, sort_order: orderById.get(memo.id)! }
              : memo)
            .sort((left, right) => left.pinned === right.pinned
              ? right.sort_order - left.sort_order
              : left.pinned ? -1 : 1));

          reorderMemos(orders).catch(error => {
            console.error('Reorder failed, refreshing:', error);
            refresh();
          });
        }
      }

      draggedIdRef.current = null;
      setDraggedId(null);
      updateDragOverId(null);
      setDragGhostPos(null);
      setDragGhostContent('');
    };

    document.addEventListener('pointermove', handlePointerMove);
    document.addEventListener('pointerup', handlePointerUp);
    return () => {
      document.removeEventListener('pointermove', handlePointerMove);
      document.removeEventListener('pointerup', handlePointerUp);
    };
  }, [draggedId, memos, refresh, setMemos, updateDragOverId]);

  return {
    draggedId,
    dragOverId,
    dragGhostPos,
    dragGhostContent,
    handlePointerDown,
  };
}
