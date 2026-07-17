import { useCallback, useEffect, useRef, useState } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import { getArchivedEntries, getEntries } from '../api/clipboard';
import type { ClipboardEntry, FilterTab, QueryFilter } from '../types';

const PAGE_SIZE = 50;

function buildFilter(
  activeTab: FilterTab,
  searchQuery: string,
  includeAuxiliaryTags: boolean,
  offset = 0,
): QueryFilter {
  const filter: QueryFilter = { limit: PAGE_SIZE, offset, includeAuxiliaryTags };
  if (activeTab !== 'all' && activeTab !== 'archive' && activeTab !== 'memo') {
    filter.category = activeTab;
  }
  const search = searchQuery.trim();
  if (search) filter.search = search;
  return filter;
}

interface Options {
  activeTab: FilterTab;
  searchQuery: string;
  includeAuxiliaryTags: boolean;
  refreshKey: number;
}

interface Result {
  entries: ClipboardEntry[];
  setEntries: Dispatch<SetStateAction<ClipboardEntry[]>>;
  hasMore: boolean;
  loading: boolean;
  setLoading: Dispatch<SetStateAction<boolean>>;
  loadingMore: boolean;
  fetchEntries: () => Promise<void>;
  loadMoreEntries: () => Promise<void>;
  refreshEntries: () => void;
  clearEntries: () => void;
}

export function useClipboardEntries({
  activeTab,
  searchQuery,
  includeAuxiliaryTags,
  refreshKey,
}: Options): Result {
  const [entries, setEntries] = useState<ClipboardEntry[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [refreshNonce, setRefreshNonce] = useState(0);
  const requestRef = useRef(0);
  const inFlightRef = useRef(false);
  const pendingRef = useRef(false);
  const loadingMoreRef = useRef(false);

  const fetchEntries = useCallback(async () => {
    const requestId = ++requestRef.current;
    if (inFlightRef.current) {
      pendingRef.current = true;
      return;
    }
    inFlightRef.current = true;
    if (activeTab === 'memo') {
      setLoading(false);
      setHasMore(false);
      inFlightRef.current = false;
      return;
    }

    const filter = buildFilter(activeTab, searchQuery, includeAuxiliaryTags);
    try {
      const data = activeTab === 'archive'
        ? await getArchivedEntries(filter)
        : await getEntries(filter);
      if (requestRef.current === requestId) {
        setEntries(data);
        setHasMore(data.length === PAGE_SIZE);
      }
    } catch (error) {
      console.error('Failed to fetch entries:', error);
    } finally {
      if (requestRef.current === requestId) setLoading(false);
      inFlightRef.current = false;
      if (pendingRef.current) {
        pendingRef.current = false;
        setRefreshNonce((value) => value + 1);
      }
    }
  }, [activeTab, includeAuxiliaryTags, searchQuery]);

  const loadMoreEntries = useCallback(async () => {
    if (activeTab === 'memo' || !hasMore || loadingMoreRef.current || inFlightRef.current) return;

    loadingMoreRef.current = true;
    setLoadingMore(true);
    const requestId = requestRef.current;
    const offset = entries.length;
    const filter = buildFilter(activeTab, searchQuery, includeAuxiliaryTags, offset);
    try {
      const data = activeTab === 'archive'
        ? await getArchivedEntries(filter)
        : await getEntries(filter);
      if (requestRef.current !== requestId) return;
      setEntries((current) => {
        if (current.length !== offset) return current;
        const knownIds = new Set(current.map((entry) => entry.id));
        return [...current, ...data.filter((entry) => !knownIds.has(entry.id))];
      });
      setHasMore(data.length === PAGE_SIZE);
    } catch (error) {
      console.error('Failed to load more entries:', error);
    } finally {
      loadingMoreRef.current = false;
      setLoadingMore(false);
    }
  }, [activeTab, entries.length, hasMore, includeAuxiliaryTags, searchQuery]);

  const refreshEntries = useCallback(() => {
    setRefreshNonce((value) => value + 1);
  }, []);

  const clearEntries = useCallback(() => {
    requestRef.current += 1;
    loadingMoreRef.current = false;
    setLoadingMore(false);
    setEntries([]);
    setHasMore(false);
  }, []);

  useEffect(() => {
    void fetchEntries();
  }, [fetchEntries, refreshKey, refreshNonce]);

  return {
    entries,
    setEntries,
    hasMore,
    loading,
    setLoading,
    loadingMore,
    fetchEntries,
    loadMoreEntries,
    refreshEntries,
    clearEntries,
  };
}
