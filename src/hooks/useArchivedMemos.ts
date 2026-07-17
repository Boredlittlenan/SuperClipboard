import { useCallback, useEffect, useRef, useState } from 'react';
import type { Dispatch, RefObject, SetStateAction } from 'react';
import { getArchivedMemos } from '../api/memos';
import type { Memo } from '../types';
import { useInfiniteScroll } from './useInfiniteScroll';

const PAGE_SIZE = 50;

interface Options {
  enabled: boolean;
  active: boolean;
  refreshKey: number;
}

interface Result {
  memos: Memo[];
  setMemos: Dispatch<SetStateAction<Memo[]>>;
  hasMore: boolean;
  loadingMore: boolean;
  listRef: RefObject<HTMLDivElement | null>;
  loadMoreRef: RefObject<HTMLDivElement | null>;
  fetchMemos: () => Promise<void>;
  clearMemos: () => void;
}

export function useArchivedMemos({ enabled, active, refreshKey }: Options): Result {
  const [memos, setMemos] = useState<Memo[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const requestRef = useRef(0);
  const loadingMoreRef = useRef(false);
  const listRef = useRef<HTMLDivElement>(null);
  const loadMoreRef = useRef<HTMLDivElement>(null);

  const clearMemos = useCallback(() => {
    requestRef.current += 1;
    loadingMoreRef.current = false;
    setLoadingMore(false);
    setMemos([]);
    setHasMore(false);
  }, []);

  const fetchMemos = useCallback(async () => {
    const requestId = ++requestRef.current;
    if (!enabled) {
      setMemos([]);
      setHasMore(false);
      return;
    }
    try {
      const data = await getArchivedMemos({ limit: PAGE_SIZE });
      if (requestRef.current !== requestId) return;
      setMemos(data);
      setHasMore(data.length === PAGE_SIZE);
    } catch (error) {
      console.error('Failed to fetch archived memos:', error);
    }
  }, [enabled]);

  const loadMoreMemos = useCallback(async () => {
    if (!hasMore || loadingMoreRef.current) return;
    loadingMoreRef.current = true;
    setLoadingMore(true);
    const requestId = requestRef.current;
    const offset = memos.length;
    try {
      const data = await getArchivedMemos({ limit: PAGE_SIZE, offset });
      if (requestRef.current !== requestId) return;
      setMemos((current) => {
        if (current.length !== offset) return current;
        const knownIds = new Set(current.map((memo) => memo.id));
        return [...current, ...data.filter((memo) => !knownIds.has(memo.id))];
      });
      setHasMore(data.length === PAGE_SIZE);
    } catch (error) {
      console.error('Failed to load more archived memos:', error);
    } finally {
      loadingMoreRef.current = false;
      setLoadingMore(false);
    }
  }, [hasMore, memos.length]);

  useInfiniteScroll(
    listRef,
    loadMoreRef,
    active && hasMore && !loadingMore,
    () => { void loadMoreMemos(); },
  );

  useEffect(() => {
    if (!enabled) {
      clearMemos();
    } else if (active) {
      void fetchMemos();
    }
  }, [active, clearMemos, enabled, fetchMemos, refreshKey]);

  return {
    memos,
    setMemos,
    hasMore,
    loadingMore,
    listRef,
    loadMoreRef,
    fetchMemos,
    clearMemos,
  };
}
