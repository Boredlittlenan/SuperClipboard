import { useState, useEffect, useCallback, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { Import, NotebookText, Search, Trash2 } from 'lucide-react';
import type { ClipboardEntry, FilterTab, QueryFilter, Stats, Memo } from './types';
import {
  getEntries,
  deleteEntry,
  togglePin,
  getStats,
  clearUnpinned,
  copyToClipboard,
  updateEntry,
  onClipboardChanged,
  getArchivedEntries,
  unarchiveEntry,
  permanentDelete,
  purgeOldArchives,
} from './api/clipboard';
import { getShortcut, checkUpdate, openUrl, pasteToActiveWindow } from './api/settings';
import { getArchivedMemos, unarchiveMemo, permanentDeleteMemo, purgeOldMemoArchives } from './api/memos';
import { formatShortcutLabel } from './utils';
import { I18nProvider, useI18n } from './i18n';
import CategoryTabs from './components/CategoryTabs';
import ClipboardList from './components/ClipboardList';
import SettingsButton from './components/SettingsButton';
import RemoteStorageButton from './components/RemoteStorageButton';
import ExperimentalFeaturesButton from './components/ExperimentalFeaturesButton';
import MemoList from './components/MemoList';
import ArchivedMemoItem from './components/ArchivedMemoItem';
import ConfirmDialog, { type ConfirmDialogState } from './components/ConfirmDialog';
import { emitAppEvent, onAppEvent } from './events/appEvents';
import { useAppSettings } from './hooks/useAppSettings';
import { useInfiniteScroll } from './hooks/useInfiniteScroll';
import { useClipboardDropImport } from './hooks/useClipboardDropImport';
import AppSettingsProvider from './components/settings/AppSettingsProvider';
import './App.css';

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

const CLIPBOARD_PAGE_SIZE = 50;

function buildClipboardFilter(activeTab: FilterTab, searchQuery: string, offset = 0): QueryFilter {
  const filter: QueryFilter = { limit: CLIPBOARD_PAGE_SIZE, offset };
  if (activeTab !== 'all' && activeTab !== 'archive' && activeTab !== 'memo') {
    filter.category = activeTab;
  }
  const search = searchQuery.trim();
  if (search) filter.search = search;
  return filter;
}

function AppContent() {
  const { t } = useI18n();
  const { settings } = useAppSettings();
  const {
    memoEnabled,
    memoColor,
    rawPreview,
    archiveEnabled,
    experimentalFeaturesEnabled,
    clipboardMultiTagEnabled,
    hideEntryColorStripEnabled,
    categoryTabSelectedColorsEnabled,
    categoryTabSortingEnabled,
    modernUiEnabled,
    themeAccent,
    themeMode,
    autoUpdate,
  } = settings;
  const [titleVariant, setTitleVariant] = useState<'default' | 'xiaonan' | 'yingnan'>('default');
  const [entries, setEntries] = useState<ClipboardEntry[]>([]);
  const [entriesHasMore, setEntriesHasMore] = useState(false);
  const [entriesLoadingMore, setEntriesLoadingMore] = useState(false);
  const [activeTab, setActiveTab] = useState<FilterTab>('all');
  const [searchQuery, setSearchQuery] = useState('');
  const [stats, setStats] = useState<Stats | null>(null);
  const [loading, setLoading] = useState(true);
  const [copied, setCopied] = useState<number | null>(null);
  const [currentShortcut, setCurrentShortcut] = useState('Alt+X');
  const [memoCountState, setMemoCountState] = useState<number | null>(null);
  const [memoListCount, setMemoListCount] = useState<number>(0);
  const [archiveCountState, setArchiveCountState] = useState<number | null>(null);
  const [systemTheme, setSystemTheme] = useState<'light' | 'dark'>(() =>
    window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
  );
  const searchRef = useRef<HTMLInputElement>(null);
  const searchQueryRef = useRef('');
  const [archiveSubTab, setArchiveSubTab] = useState<'clipboard' | 'memos'>('clipboard');
  const [archivedMemos, setArchivedMemos] = useState<Memo[]>([]);
  const [archivedMemosHasMore, setArchivedMemosHasMore] = useState(false);
  const [archivedMemosLoadingMore, setArchivedMemosLoadingMore] = useState(false);
  const [memoArchiveCountState, setMemoArchiveCountState] = useState<number>(0);
  const [openedViaShortcut, setOpenedViaShortcut] = useState(false);
  const [confirmDialog, setConfirmDialog] = useState<ConfirmDialogState | null>(null);
  const [dropNotice, setDropNotice] = useState<{ message: string; tone: 'success' | 'error' } | null>(null);
  const [storageRevision, setStorageRevision] = useState(0);
  const [entriesRefreshNonce, setEntriesRefreshNonce] = useState(0);
  const [isWindowDragging, setIsWindowDragging] = useState(false);
  const fetchEntriesRequestRef = useRef(0);
  const fetchEntriesInFlightRef = useRef(false);
  const fetchEntriesPendingRef = useRef(false);
  const entriesLoadingMoreRef = useRef(false);
  const archivedMemosLoadingMoreRef = useRef(false);
  const archivedMemosRequestRef = useRef(0);
  const archivedMemoListRef = useRef<HTMLDivElement>(null);
  const archivedMemoLoadMoreRef = useRef<HTMLDivElement>(null);
  const autoUpdateCheckedRef = useRef(false);
  const resumeRefreshRef = useRef(0);
  const lastWakeCheckRef = useRef(Date.now());
  const resumeRefreshTimersRef = useRef<number[]>([]);
  // Hidden title variants triggered from the Settings version badge.
  const displayTitle = titleVariant === 'xiaonan'
    ? '小楠の剪贴板'
    : titleVariant === 'yingnan'
      ? '瑛楠的剪贴板'
      : t.appTitle;

  const handleTitleDragStart = useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    setIsWindowDragging(true);

    const stopDragging = () => setIsWindowDragging(false);
    window.addEventListener('pointerup', stopDragging, { once: true });
    window.addEventListener('blur', stopDragging, { once: true });
    window.setTimeout(stopDragging, 1800);
  }, []);

  useEffect(() => {
    document.title = displayTitle;
  }, [displayTitle]);

  const handleVersionTitleTrigger = useCallback((clickCount: number) => {
    // Keep this undocumented; it is a small maintenance-only title easter egg.
    if (clickCount === 2) {
      setTitleVariant('default');
    } else if (clickCount === 3) {
      setTitleVariant('xiaonan');
    } else if (clickCount === 5) {
      setTitleVariant('yingnan');
    }
  }, []);

  // Fetch current shortcut on mount
  useEffect(() => {
    getShortcut().then(setCurrentShortcut).catch(console.error);
  }, []);

  useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const updateSystemTheme = (value: boolean) => {
      setSystemTheme(value ? 'dark' : 'light');
    };

    updateSystemTheme(media.matches);

    const listener = (event: MediaQueryListEvent) => updateSystemTheme(event.matches);
    if (media.addEventListener) {
      media.addEventListener('change', listener);
      return () => media.removeEventListener('change', listener);
    }

    media.addListener(listener);
    return () => media.removeListener(listener);
  }, []);

  const resolvedTheme = themeMode === 'system' ? systemTheme : themeMode;
  const effectiveModernUiEnabled = experimentalFeaturesEnabled && modernUiEnabled;
  const effectiveEntryColorIndicatorEnabled = !experimentalFeaturesEnabled || !hideEntryColorStripEnabled;

  useEffect(() => {
    document.documentElement.dataset.theme = resolvedTheme;
    document.documentElement.dataset.accent = themeAccent;
  }, [resolvedTheme, themeAccent]);

  // Apply custom memo color via data attribute + CSS variables
  useEffect(() => {
    const root = document.documentElement;
    if (memoColor) {
      const r = parseInt(memoColor.slice(1, 3), 16);
      const g = parseInt(memoColor.slice(3, 5), 16);
      const b = parseInt(memoColor.slice(5, 7), 16);
      root.setAttribute('data-memo-color', memoColor);
      root.style.setProperty('--custom-memo-color', memoColor);
      root.style.setProperty('--custom-memo-color-bg', `rgba(${r}, ${g}, ${b}, 0.1)`);
    } else {
      root.removeAttribute('data-memo-color');
      root.style.removeProperty('--custom-memo-color');
      root.style.removeProperty('--custom-memo-color-bg');
    }
  }, [memoColor]);

  // Auto-check for updates on startup if enabled
  useEffect(() => {
    if (!autoUpdate || autoUpdateCheckedRef.current) return;
    autoUpdateCheckedRef.current = true;

    let cancelled = false;

    Promise.resolve()
      .then(async () => {
        const info = await checkUpdate();
        if (cancelled || !info.hasUpdate) return;

        const releaseNotes = info.releaseNotes
          ? `\n\n${t.releaseNotes}\n${info.releaseNotes}`
          : '';
        setConfirmDialog({
          title: t.hasUpdate(info.latestVersion),
          message: `${t.updateCurrent(info.currentVersion)} · ${t.updateLatest(info.latestVersion)}${releaseNotes}`,
          confirmLabel: t.downloadUpdate,
          resolve: (confirmed) => {
            if (confirmed && info.downloadUrl) {
              openUrl(info.downloadUrl).catch(console.error);
            }
          },
        });
      })
      .catch((err) => {
        console.error('Failed to auto-check update:', err);
      });

    return () => {
      cancelled = true;
    };
  }, [autoUpdate, t]);

  // Fetch entries based on current filter
  const fetchEntries = useCallback(async () => {
    const requestId = ++fetchEntriesRequestRef.current;
    if (fetchEntriesInFlightRef.current) {
      fetchEntriesPendingRef.current = true;
      return;
    }
    fetchEntriesInFlightRef.current = true;
    if (activeTab === 'memo') {
      setLoading(false);
      setEntriesHasMore(false);
      fetchEntriesInFlightRef.current = false;
      return;
    }
    const filter = buildClipboardFilter(activeTab, searchQuery);
    try {
      if (activeTab === 'archive') {
        const data = await getArchivedEntries(filter);
        if (fetchEntriesRequestRef.current === requestId) {
          setEntries(data);
          setEntriesHasMore(data.length === CLIPBOARD_PAGE_SIZE);
        }
      } else {
        const data = await getEntries(filter);
        if (fetchEntriesRequestRef.current === requestId) {
          setEntries(data);
          setEntriesHasMore(data.length === CLIPBOARD_PAGE_SIZE);
        }
      }
    } catch (err) {
      console.error('Failed to fetch entries:', err);
    } finally {
      if (fetchEntriesRequestRef.current === requestId) {
        setLoading(false);
      }
      fetchEntriesInFlightRef.current = false;
      if (fetchEntriesPendingRef.current) {
        fetchEntriesPendingRef.current = false;
        setEntriesRefreshNonce((value) => value + 1);
      }
    }
  }, [activeTab, searchQuery]);

  const loadMoreEntries = useCallback(async () => {
    if (
      activeTab === 'memo'
      || !entriesHasMore
      || entriesLoadingMoreRef.current
      || fetchEntriesInFlightRef.current
    ) return;

    entriesLoadingMoreRef.current = true;
    setEntriesLoadingMore(true);
    const requestId = fetchEntriesRequestRef.current;
    const offset = entries.length;
    const filter = buildClipboardFilter(activeTab, searchQuery, offset);
    try {
      const data = activeTab === 'archive'
        ? await getArchivedEntries(filter)
        : await getEntries(filter);
      if (fetchEntriesRequestRef.current !== requestId) return;
      setEntries((current) => {
        if (current.length !== offset) return current;
        const knownIds = new Set(current.map((entry) => entry.id));
        return [...current, ...data.filter((entry) => !knownIds.has(entry.id))];
      });
      setEntriesHasMore(data.length === CLIPBOARD_PAGE_SIZE);
    } catch (error) {
      console.error('Failed to load more entries:', error);
    } finally {
      entriesLoadingMoreRef.current = false;
      setEntriesLoadingMore(false);
    }
  }, [activeTab, entries.length, entriesHasMore, searchQuery]);

  // Fetch stats
  const fetchStats = useCallback(async () => {
    try {
      const s = await getStats();
      setStats(s);
      setArchiveCountState(s.archive);
      setMemoCountState(s.memoCount);
      setMemoArchiveCountState(s.memoArchive);
    } catch (err) {
      console.error('Failed to fetch stats:', err);
    }
  }, []);

  const refreshDataOnce = useCallback(() => {
    if (activeTab !== 'memo') {
      setLoading(true);
    }
    setStorageRevision((value) => value + 1);
  }, [activeTab]);

  const clearScheduledDataRefreshes = useCallback(() => {
    resumeRefreshTimersRef.current.forEach((timer) => window.clearTimeout(timer));
    resumeRefreshTimersRef.current = [];
  }, []);

  const scheduleDataRefresh = useCallback((force = false) => {
    const now = Date.now();
    if (!force && now - resumeRefreshRef.current < 1000) {
      return;
    }
    resumeRefreshRef.current = now;

    refreshDataOnce();
    clearScheduledDataRefreshes();
    resumeRefreshTimersRef.current = [
      window.setTimeout(refreshDataOnce, 1500),
      window.setTimeout(refreshDataOnce, 5000),
    ];
  }, [clearScheduledDataRefreshes, refreshDataOnce]);

  useEffect(() => {
    const offResume = onAppEvent('app:resume', () => scheduleDataRefresh(true));
    const offClipboard = onAppEvent('clipboard:changed', refreshDataOnce);
    const offStorage = onAppEvent('storage:changed', () => {
      clearScheduledDataRefreshes();
      setEntries([]);
      setArchivedMemos([]);
      refreshDataOnce();
    });
    return () => {
      offResume();
      offClipboard();
      offStorage();
    };
  }, [clearScheduledDataRefreshes, refreshDataOnce, scheduleDataRefresh]);

  // Listen for window-shown events to track how the window was opened and refresh remote data.
  useEffect(() => {
    const unlisten = listen<string>('window-shown', (event) => {
      const source = event.payload;
      setOpenedViaShortcut(source === 'shortcut');
      emitAppEvent('app:resume');
      // Follow mode positioning is handled in Rust before window.show()
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  // Refresh again after sleep/resume or network recovery.
  // Do not refresh on every focus: clicking the draggable title bar focuses the
  // WebView before window movement, and a synchronous list refresh makes drag
  // feel sticky.
  useEffect(() => {
    const checkForWake = () => {
      const now = Date.now();
      const elapsed = now - lastWakeCheckRef.current;
      lastWakeCheckRef.current = now;
      if (elapsed > 60_000) {
        emitAppEvent('app:resume');
      }
    };
    const refreshWhenVisible = () => {
      if (document.visibilityState === 'visible') {
        checkForWake();
      }
    };

    const wakeTimer = window.setInterval(checkForWake, 30_000);
    window.addEventListener('online', checkForWake);
    window.addEventListener('pageshow', checkForWake);
    document.addEventListener('visibilitychange', refreshWhenVisible);

    return () => {
      window.clearInterval(wakeTimer);
      window.removeEventListener('online', checkForWake);
      window.removeEventListener('pageshow', checkForWake);
      document.removeEventListener('visibilitychange', refreshWhenVisible);
      resumeRefreshTimersRef.current.forEach((timer) => window.clearTimeout(timer));
      resumeRefreshTimersRef.current = [];
    };
  }, []);

  // Fetch archived memos
  const fetchArchivedMemos = useCallback(async () => {
    const requestId = ++archivedMemosRequestRef.current;
    if (!archiveEnabled) {
      setArchivedMemos([]);
      setArchivedMemosHasMore(false);
      return;
    }
    try {
      const data = await getArchivedMemos({ limit: CLIPBOARD_PAGE_SIZE });
      if (archivedMemosRequestRef.current !== requestId) return;
      setArchivedMemos(data);
      setArchivedMemosHasMore(data.length === CLIPBOARD_PAGE_SIZE);
    } catch (err) {
      console.error('Failed to fetch archived memos:', err);
    }
  }, [archiveEnabled]);

  const loadMoreArchivedMemos = useCallback(async () => {
    if (!archivedMemosHasMore || archivedMemosLoadingMoreRef.current) return;
    archivedMemosLoadingMoreRef.current = true;
    setArchivedMemosLoadingMore(true);
    const requestId = archivedMemosRequestRef.current;
    const offset = archivedMemos.length;
    try {
      const data = await getArchivedMemos({ limit: CLIPBOARD_PAGE_SIZE, offset });
      if (archivedMemosRequestRef.current !== requestId) return;
      setArchivedMemos((current) => {
        if (current.length !== offset) return current;
        const knownIds = new Set(current.map((memo) => memo.id));
        return [...current, ...data.filter((memo) => !knownIds.has(memo.id))];
      });
      setArchivedMemosHasMore(data.length === CLIPBOARD_PAGE_SIZE);
    } catch (error) {
      console.error('Failed to load more archived memos:', error);
    } finally {
      archivedMemosLoadingMoreRef.current = false;
      setArchivedMemosLoadingMore(false);
    }
  }, [archivedMemos.length, archivedMemosHasMore]);

  useInfiniteScroll(
    archivedMemoListRef,
    archivedMemoLoadMoreRef,
    activeTab === 'archive' && archiveSubTab === 'memos' && archivedMemosHasMore && !archivedMemosLoadingMore,
    () => { void loadMoreArchivedMemos(); },
  );

  // Fetch visible list when the active filter changes.
  useEffect(() => {
    void fetchEntries();
  }, [fetchEntries, storageRevision, entriesRefreshNonce]);

  // Refresh counts when storage changes; tab clicks also trigger a direct stats refresh.
  useEffect(() => {
    void fetchStats();
  }, [fetchStats, storageRevision]);

  useEffect(() => {
    if (!archiveEnabled) return;
    purgeOldArchives(30).catch(() => {});
    purgeOldMemoArchives(30).catch(() => {});
  }, [archiveEnabled]);

  // Listen for real-time clipboard events
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    onClipboardChanged(() => {
      emitAppEvent('clipboard:changed');
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  // Coalesce PostgreSQL LISTEN/NOTIFY events before refreshing the active view.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let refreshTimer: number | undefined;
    listen<string>('remote-storage-changed', () => {
      if (refreshTimer !== undefined) {
        window.clearTimeout(refreshTimer);
      }
      refreshTimer = window.setTimeout(() => {
        refreshTimer = undefined;
        emitAppEvent('storage:changed');
      }, 150);
    }).then((handler) => {
      unlisten = handler;
    }).catch(console.error);

    return () => {
      unlisten?.();
      if (refreshTimer !== undefined) {
        window.clearTimeout(refreshTimer);
      }
    };
  }, []);

  // Keyboard shortcut: focus search with Ctrl+F
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
        e.preventDefault();
        searchRef.current?.focus();
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);

  useEffect(() => {
    if (!confirmDialog) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        confirmDialog.resolve(false);
        setConfirmDialog(null);
      }
      if (e.key === 'Enter') {
        confirmDialog.resolve(true);
        setConfirmDialog(null);
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [confirmDialog]);

  const requestConfirm = useCallback((dialog: Omit<ConfirmDialogState, 'resolve'>) => {
    return new Promise<boolean>((resolve) => {
      setConfirmDialog({ ...dialog, resolve });
    });
  }, []);

  // Actions
  const handleCopy = useCallback(async (id: number, useOriginal = false) => {
    try {
      if (openedViaShortcut && !useOriginal) {
        // Paste directly to the active window (hides window + simulates Ctrl+V)
        await pasteToActiveWindow(id);
        setOpenedViaShortcut(false);
      } else {
        await copyToClipboard(id, useOriginal);
      }
      setCopied(id);
      setTimeout(() => setCopied(null), 1500);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  }, [openedViaShortcut]);

  const handleDelete = useCallback(
    async (id: number) => {
      try {
        await deleteEntry(id, archiveEnabled || undefined);
        setEntries((prev) => prev.filter((e) => e.id !== id));
        fetchStats();
      } catch (err) {
        console.error('Failed to delete:', err);
      }
    },
    [fetchStats, archiveEnabled]
  );

  const handleTogglePin = useCallback(
    async (id: number) => {
      try {
        const newPinned = await togglePin(id);
        setEntries((prev) =>
          prev.map((e) => (e.id === id ? { ...e, pinned: newPinned } : e))
        );
      } catch (err) {
        console.error('Failed to toggle pin:', err);
      }
    },
    []
  );

  const handleEdit = useCallback(
    async (id: number, content: string, expectedVersion: number) => {
      const result = await updateEntry(id, content, expectedVersion);
      await Promise.all([fetchEntries(), fetchStats()]);
      return result;
    },
    [fetchEntries, fetchStats]
  );

  const handleRestore = useCallback(
    async (id: number) => {
      try {
        await unarchiveEntry(id);
        setEntries((prev) => prev.filter((e) => e.id !== id));
        fetchStats();
      } catch (err) {
        console.error('Failed to restore:', err);
      }
    },
    [fetchStats]
  );

  const handlePermanentDelete = useCallback(
    async (id: number) => {
      const confirmed = await requestConfirm({
        title: t.permanentDelete,
        message: t.permanentDeleteConfirm,
        confirmLabel: t.permanentDelete,
        tone: 'danger',
      });
      if (!confirmed) return;
      try {
        await permanentDelete(id);
        setEntries((prev) => prev.filter((e) => e.id !== id));
        fetchStats();
      } catch (err) {
        console.error('Failed to permanently delete:', err);
      }
    },
    [fetchStats, requestConfirm, t]
  );

  const handleMemoRestore = useCallback(
    async (id: number) => {
      try {
        await unarchiveMemo(id);
        setArchivedMemos((prev) => prev.filter((m) => m.id !== id));
        fetchStats();
      } catch (err) {
        console.error('Failed to restore memo:', err);
      }
    },
    [fetchStats]
  );

  const handleMemoPermanentDelete = useCallback(
    async (id: number) => {
      const confirmed = await requestConfirm({
        title: t.permanentDelete,
        message: t.permanentDeleteConfirm,
        confirmLabel: t.permanentDelete,
        tone: 'danger',
      });
      if (!confirmed) return;
      try {
        await permanentDeleteMemo(id);
        setArchivedMemos((prev) => prev.filter((m) => m.id !== id));
        fetchStats();
      } catch (err) {
        console.error('Failed to permanently delete memo:', err);
      }
    },
    [fetchStats, requestConfirm, t]
  );

  const handleClear = useCallback(async () => {
    const confirmed = await requestConfirm({
      title: t.clearHistory,
      message: t.clearConfirm,
      confirmLabel: t.clearHistory,
      tone: 'danger',
    });
    if (!confirmed) return;
    try {
      await clearUnpinned(archiveEnabled || undefined);
      fetchEntries();
      fetchStats();
    } catch (err) {
      console.error('Failed to clear:', err);
    }
  }, [fetchEntries, fetchStats, archiveEnabled, requestConfirm, t]);

  // Fetch archived memos when archive tab is active
  useEffect(() => {
    if (activeTab === 'archive' && archiveEnabled) {
      fetchArchivedMemos();
    }
  }, [activeTab, archiveEnabled, fetchArchivedMemos, storageRevision]);

  // Handle tab change
  const handleTabChange = useCallback((tab: FilterTab) => {
    if (tab === activeTab) return;
    if (tab !== 'memo') {
      setLoading(true);
    }
    setActiveTab(tab);
    void fetchStats();
  }, [activeTab, fetchStats]);

  const handleMemoCountChange = useCallback((count: number) => {
    setMemoListCount(count);
  }, []);

  const handleMemoTotalCountChange = useCallback((count: number) => {
    setMemoCountState(count);
    setStats((prev) => (prev ? { ...prev, memoCount: count } : prev));
  }, []);

  const handleStorageModeChange = useCallback(() => {
    emitAppEvent('storage:changed');
  }, []);

  // Debounced search
  const [searchInput, setSearchInput] = useState('');
  useEffect(() => {
    searchQueryRef.current = searchQuery;
  }, [searchQuery]);

  useEffect(() => {
    const timer = setTimeout(() => {
      if (searchQueryRef.current !== searchInput) {
        setLoading(true);
        setSearchQuery(searchInput);
      }
    }, 250);
    return () => clearTimeout(timer);
  }, [searchInput]);

  const handleDropImportComplete = useCallback((inserted: boolean) => {
    if (inserted) {
      setDropNotice({ message: t.dropImportDone, tone: 'success' });
    }
  }, [t]);

  const handleDropImportError = useCallback(() => {
    setDropNotice({ message: t.dropImportFailed, tone: 'error' });
  }, [t]);

  useEffect(() => {
    if (!dropNotice) return;
    const timer = window.setTimeout(() => setDropNotice(null), 2200);
    return () => window.clearTimeout(timer);
  }, [dropNotice]);

  const dropImport = useClipboardDropImport({
    onComplete: handleDropImportComplete,
    onError: handleDropImportError,
  });

  return (
    <div
      className={`app-root${isWindowDragging ? ' is-window-dragging' : ''}`}
      data-theme={resolvedTheme}
      data-accent={themeAccent}
      data-ui-style={effectiveModernUiEnabled ? 'modern' : 'classic'}
      data-memo-color={memoColor || undefined}
      onDragEnter={dropImport.handleDragEnter}
      onDragOver={dropImport.handleDragOver}
      onDragLeave={dropImport.handleDragLeave}
      onDrop={dropImport.handleDrop}
    >
      {/* Title bar (draggable, frameless window) */}
      <div data-tauri-drag-region className="title-bar" onPointerDown={handleTitleDragStart}>
        <div data-tauri-drag-region className="title-content">
          <span className="title-text">{displayTitle}</span>
          <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
            <span className="shortcut-hint">{formatShortcutLabel(currentShortcut)}</span>
            {experimentalFeaturesEnabled && (
              <ExperimentalFeaturesButton />
            )}
            <RemoteStorageButton onStorageModeChange={handleStorageModeChange} />
            <SettingsButton
              onShortcutChange={setCurrentShortcut}
              onVersionTitleTrigger={handleVersionTitleTrigger}
            />
          </div>
        </div>
      </div>

      {/* Search bar */}
      <div className={`search-bar${activeTab === 'memo' ? ' search-bar-memo' : ''}`}>
        <span className="search-icon" aria-hidden="true">
          <Search size={15} strokeWidth={2.2} />
        </span>
        <input
          ref={searchRef}
          type="text"
          placeholder={activeTab === 'memo' ? t.memoSearchPlaceholder : t.searchPlaceholder}
          value={searchInput}
          onChange={(e) => setSearchInput(e.target.value)}
          className="search-input"
        />
        {searchInput && (
          <button
            className="clear-search-btn"
            onClick={() => setSearchInput('')}
            title={t.clearSearch}
          >
            &#x2715;
          </button>
        )}
      </div>

      {/* Category tabs */}
      <CategoryTabs
        activeTab={activeTab}
        onTabChange={handleTabChange}
        stats={stats}
        memoEnabled={memoEnabled}
        memoCount={memoCountState}
        archiveEnabled={archiveEnabled}
        archiveCount={archiveCountState}
        categorySortingEnabled={categoryTabSortingEnabled}
        categoryTabSelectedColors={experimentalFeaturesEnabled && categoryTabSelectedColorsEnabled}
        modernUi={effectiveModernUiEnabled}
      />

      {/* Main content: memo list or clipboard list */}
      {activeTab === 'memo' ? (
        <MemoList
          searchQuery={searchQuery}
          archiveEnabled={archiveEnabled}
          refreshKey={storageRevision}
          onCountChange={handleMemoCountChange}
          onTotalCountChange={handleMemoTotalCountChange}
          onArchiveCountChange={setMemoArchiveCountState}
        />
      ) : activeTab === 'archive' ? (
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
          {/* Archive sub-tabs */}
          <div style={{ display: 'flex', gap: '0', padding: '0 12px', borderBottom: '1px solid var(--border)' }}>
            <button
              style={{
                flex: 1, padding: '8px 0', border: 'none', borderBottom: archiveSubTab === 'clipboard' ? '2px solid var(--accent)' : '2px solid transparent',
                background: 'transparent', color: archiveSubTab === 'clipboard' ? 'var(--accent)' : 'var(--text-muted)',
                fontSize: '12px', fontWeight: 600, cursor: 'pointer', transition: 'all 0.15s',
                display: 'flex', alignItems: 'center', justifyContent: 'center', minHeight: '34px', lineHeight: 1,
              }}
              onClick={() => setArchiveSubTab('clipboard')}
            >
              {t.archiveSubTab} ({archiveCountState ?? 0})
            </button>
            <button
              style={{
                flex: 1, padding: '8px 0', border: 'none', borderBottom: archiveSubTab === 'memos' ? '2px solid var(--accent)' : '2px solid transparent',
                background: 'transparent', color: archiveSubTab === 'memos' ? 'var(--accent)' : 'var(--text-muted)',
                fontSize: '12px', fontWeight: 600, cursor: 'pointer', transition: 'all 0.15s',
                display: 'flex', alignItems: 'center', justifyContent: 'center', minHeight: '34px', lineHeight: 1,
              }}
              onClick={() => { setArchiveSubTab('memos'); fetchArchivedMemos(); }}
            >
              {t.memoSubTab} ({memoArchiveCountState})
            </button>
          </div>
          {/* Sub-tab content */}
          {archiveSubTab === 'clipboard' ? (
            entries.length === 0 && !loading ? (
              <div style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: '8px', padding: '40px 20px' }}>
                <Trash2 size={36} strokeWidth={1.8} style={{ opacity: 0.5 }} />
                <span style={{ fontSize: '13px', color: 'var(--text-secondary)', fontWeight: 500 }}>{t.archiveEmpty}</span>
                <span style={{ fontSize: '11px', color: 'var(--text-muted)' }}>{t.archiveEmptyHint}</span>
              </div>
            ) : (
              <ClipboardList
                entries={entries}
                onCopy={handleCopy}
                onDelete={handlePermanentDelete}
                onTogglePin={handleTogglePin}
                onEdit={handleEdit}
                rawPreview={rawPreview}
                loading={loading}
                isArchive={true}
                archiveEnabled={archiveEnabled}
                multiTagEnabled={experimentalFeaturesEnabled && clipboardMultiTagEnabled}
                showCategoryIndicator={effectiveEntryColorIndicatorEnabled}
                onRestore={handleRestore}
                onPermanentDelete={handlePermanentDelete}
                hasMore={entriesHasMore}
                loadingMore={entriesLoadingMore}
                onLoadMore={loadMoreEntries}
              />
            )
          ) : (
            archivedMemos.length === 0 ? (
              <div style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: '8px', padding: '40px 20px' }}>
                <NotebookText size={36} strokeWidth={1.8} style={{ opacity: 0.5 }} />
                <span style={{ fontSize: '13px', color: 'var(--text-secondary)', fontWeight: 500 }}>{t.archiveEmpty}</span>
                <span style={{ fontSize: '11px', color: 'var(--text-muted)' }}>{t.archiveEmptyHint}</span>
              </div>
            ) : (
              <div ref={archivedMemoListRef} style={{ flex: 1, overflowY: 'auto' }}>
                {archivedMemos.map(memo => (
                  <ArchivedMemoItem
                    key={memo.id}
                    memo={memo}
                    onRestore={() => handleMemoRestore(memo.id)}
                    onPermanentDelete={() => handleMemoPermanentDelete(memo.id)}
                  />
                ))}
                <div ref={archivedMemoLoadMoreRef} style={{ minHeight: '18px', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                  {archivedMemosLoadingMore && <div style={{ width: '14px', height: '14px', border: '2px solid var(--border)', borderTopColor: 'var(--accent)', borderRadius: '50%', animation: 'spin 0.8s linear infinite' }} />}
                </div>
              </div>
            )
          )}
        </div>
      ) : (
        <ClipboardList
          entries={entries}
          onCopy={handleCopy}
          onDelete={handleDelete}
          onTogglePin={handleTogglePin}
          onEdit={handleEdit}
          rawPreview={rawPreview}
          loading={loading}
          archiveEnabled={archiveEnabled}
          multiTagEnabled={experimentalFeaturesEnabled && clipboardMultiTagEnabled}
          showCategoryIndicator={effectiveEntryColorIndicatorEnabled}
          onRestore={handleRestore}
          onPermanentDelete={handlePermanentDelete}
          hasMore={entriesHasMore}
          loadingMore={entriesLoadingMore}
          onLoadMore={loadMoreEntries}
        />
      )}

      {/* Footer bar */}
      <div className="footer-bar">
        <span className="footer-text">
          {activeTab === 'memo'
            ? t.itemsCount(memoListCount)
            : activeTab === 'archive' && archiveSubTab === 'memos'
            ? t.itemsCount(archivedMemos.length)
            : t.itemsCount(entries.length)}
          {activeTab === 'memo'
            ? stats?.memoSize != null && ` · ${t.memoStorage(formatBytes(stats.memoSize))}`
            : activeTab === 'archive' && archiveSubTab === 'memos'
            ? stats?.memoSize != null && ` · ${t.memoStorage(formatBytes(stats.memoSize))}`
            : stats?.clipboardSize != null && ` · ${t.clipboardStorage(formatBytes(stats.clipboardSize))}`}
        </span>
        {activeTab !== 'memo' && activeTab !== 'archive' && (
          <button
            className="clear-btn"
            onClick={handleClear}
            title={t.clearHistory}
          >
            {t.clearHistory}
          </button>
        )}
      </div>

      {(dropImport.isDragActive || dropImport.isImporting) && (
        <div className="clipboard-drop-overlay" role="status" aria-live="polite">
          <div className="clipboard-drop-overlay-panel">
            <Import size={24} strokeWidth={1.9} aria-hidden="true" />
            <strong>{dropImport.isImporting ? t.dropImporting : t.dropImportPrompt}</strong>
            {!dropImport.isImporting && <span>{t.dropImportHint}</span>}
          </div>
        </div>
      )}

      {/* Copied toast */}
      {copied !== null && (
        <div className="toast">{t.copied}</div>
      )}
      {dropNotice && (
        <div className={`toast${dropNotice.tone === 'error' ? ' toast-error' : ''}`}>{dropNotice.message}</div>
      )}

      {confirmDialog && (
        <ConfirmDialog dialog={confirmDialog} onClose={() => setConfirmDialog(null)} />
      )}
    </div>
  );
}

function App() {
  return (
    <I18nProvider>
      <AppSettingsProvider>
        <AppContent />
      </AppSettingsProvider>
    </I18nProvider>
  );
}

export default App;
