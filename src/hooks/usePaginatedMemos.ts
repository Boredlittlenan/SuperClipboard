import { useCallback, useEffect, useRef, useState, type Dispatch, type SetStateAction } from 'react';
import { getMemos } from '../api/memos';
import type { Memo, MemoFilter } from '../types';

const MEMO_PAGE_SIZE = 50;

interface PaginatedMemos {
  memos: Memo[];
  setMemos: Dispatch<SetStateAction<Memo[]>>;
  hasMore: boolean;
  loadingMore: boolean;
  refresh: () => Promise<void>;
  loadMore: () => Promise<void>;
}

function buildFilter(searchQuery: string, offset = 0): MemoFilter {
  const filter: MemoFilter = { limit: MEMO_PAGE_SIZE, offset };
  const search = searchQuery.trim();
  if (search) filter.search = search;
  return filter;
}

export function usePaginatedMemos(searchQuery: string, refreshKey: number): PaginatedMemos {
  const [memos, setMemos] = useState<Memo[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const requestRef = useRef(0);
  const loadingMoreRef = useRef(false);

  const refresh = useCallback(async () => {
    const requestId = ++requestRef.current;
    try {
      const data = await getMemos(buildFilter(searchQuery));
      if (requestRef.current !== requestId) return;
      setMemos(data);
      setHasMore(data.length === MEMO_PAGE_SIZE);
    } catch (error) {
      console.error('Failed to fetch memos:', error);
    }
  }, [searchQuery]);

  useEffect(() => {
    void refresh();
  }, [refresh, refreshKey]);

  const loadMore = useCallback(async () => {
    if (!hasMore || loadingMoreRef.current) return;
    loadingMoreRef.current = true;
    setLoadingMore(true);
    const requestId = requestRef.current;
    const offset = memos.length;
    try {
      const data = await getMemos(buildFilter(searchQuery, offset));
      if (requestRef.current !== requestId) return;
      setMemos((current) => {
        if (current.length !== offset) return current;
        const knownIds = new Set(current.map((memo) => memo.id));
        return [...current, ...data.filter((memo) => !knownIds.has(memo.id))];
      });
      setHasMore(data.length === MEMO_PAGE_SIZE);
    } catch (error) {
      console.error('Failed to load more memos:', error);
    } finally {
      loadingMoreRef.current = false;
      setLoadingMore(false);
    }
  }, [hasMore, memos.length, searchQuery]);

  return { memos, setMemos, hasMore, loadingMore, refresh, loadMore };
}
