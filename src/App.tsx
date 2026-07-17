import { lazy, Suspense, useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { Combine, Import, ListChecks, NotebookText, Search, Trash2, X } from 'lucide-react';
import type { ClipboardEntry, FilterTab, Stats } from './types';
import {
  deleteEntry,
  togglePin,
  getStats,
  clearUnpinned,
  copyToClipboard,
  updateEntry,
  onClipboardChanged,
  unarchiveEntry,
  permanentDelete,
  purgeOldArchives,
  emptyArchive,
  mergeEntries,
  deleteEntries,
  reclassifyClipboardEntries,
} from './api/clipboard';
import { getShortcut, checkUpdate, openUrl, pasteToActiveWindow } from './api/settings';
import { unarchiveMemo, permanentDeleteMemo, purgeOldMemoArchives, emptyMemoArchive } from './api/memos';
import { formatShortcutLabel, getTabLabel } from './utils';
import { I18nProvider, useI18n } from './i18n';
import CategoryTabs from './components/CategoryTabs';
import ClipboardList from './components/ClipboardList';
import ArchivedMemoItem from './components/ArchivedMemoItem';
import ConfirmDialog, { type ConfirmDialogState } from './components/ConfirmDialog';
import { emitAppEvent, onAppEvent } from './events/appEvents';
import { useAppSettings } from './hooks/useAppSettings';
import { useClipboardDropImport } from './hooks/useClipboardDropImport';
import { useClipboardEntries } from './hooks/useClipboardEntries';
import { useArchivedMemos } from './hooks/useArchivedMemos';
import AppSettingsProvider from './components/settings/AppSettingsProvider';
import { canMergeEntries, getMergeCategory, getSelectedEntriesInListOrder, isBatchDeleteShortcut, MAX_MERGE_SELECTION } from './clipboardMerge';
import { getFooterItemCount } from './footerItemCount';
import './App.css';

const SettingsButton = lazy(() => import('./components/SettingsButton'));
const RemoteStorageButton = lazy(() => import('./components/RemoteStorageButton'));
const ExperimentalFeaturesButton = lazy(() => import('./components/ExperimentalFeaturesButton'));
const MemoList = lazy(() => import('./components/MemoList'));

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
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
    multiSelectEnabled,
    hideEntryColorStripEnabled,
    categoryTabSelectedColorsEnabled,
    categoryTabSortingEnabled,
    modernUiEnabled,
    themeAccent,
    themeMode,
    autoUpdate,
  } = settings;
  const multiTagClassificationEnabled = experimentalFeaturesEnabled && clipboardMultiTagEnabled;
  const multiSelectModeEnabled = experimentalFeaturesEnabled && multiSelectEnabled;
  const [titleVariant, setTitleVariant] = useState<'default' | 'xiaonan' | 'yingnan'>('default');
  const [activeTab, setActiveTab] = useState<FilterTab>('all');
  const [searchQuery, setSearchQuery] = useState('');
  const [stats, setStats] = useState<Stats | null>(null);
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
  const [memoArchiveCountState, setMemoArchiveCountState] = useState<number>(0);
  const [openedViaShortcut, setOpenedViaShortcut] = useState(false);
  const [confirmDialog, setConfirmDialog] = useState<ConfirmDialogState | null>(null);
  const [dropNotice, setDropNotice] = useState<{ message: string; tone: 'success' | 'error' } | null>(null);
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedEntryIds, setSelectedEntryIds] = useState<number[]>([]);
  const [batchActionPending, setBatchActionPending] = useState(false);
  const [archiveClearPending, setArchiveClearPending] = useState(false);
  const [reclassifyingHistory, setReclassifyingHistory] = useState(false);
  const [storageRevision, setStorageRevision] = useState(0);
  const [isWindowDragging, setIsWindowDragging] = useState(false);
  const fetchStatsRequestRef = useRef(0);
  const autoUpdateCheckedRef = useRef(false);
  const resumeRefreshRef = useRef(0);
  const lastWakeCheckRef = useRef(Date.now());
  const resumeRefreshTimersRef = useRef<number[]>([]);
  const {
    entries,
    setEntries,
    hasMore: entriesHasMore,
    loading,
    setLoading,
    loadingMore: entriesLoadingMore,
    fetchEntries,
    loadMoreEntries,
    refreshEntries,
    clearEntries,
  } = useClipboardEntries({
    activeTab,
    searchQuery,
    includeAuxiliaryTags: multiTagClassificationEnabled,
    refreshKey: storageRevision,
  });
  const {
    memos: archivedMemos,
    setMemos: setArchivedMemos,
    loadingMore: archivedMemosLoadingMore,
    listRef: archivedMemoListRef,
    loadMoreRef: archivedMemoLoadMoreRef,
    fetchMemos: fetchArchivedMemos,
    clearMemos: clearArchivedMemos,
  } = useArchivedMemos({
    enabled: archiveEnabled,
    active: activeTab === 'archive' && archiveSubTab === 'memos',
    refreshKey: storageRevision,
  });
  // Hidden title variants triggered from the Settings version badge.
  const displayTitle = titleVariant === 'xiaonan'
    ? '小楠の剪贴板'
    : titleVariant === 'yingnan'
      ? '瑛楠的剪贴板'
      : t.appTitle;
  const selectedEntries = useMemo(
    () => getSelectedEntriesInListOrder(entries, selectedEntryIds),
    [entries, selectedEntryIds],
  );
  const selectionCategory = useMemo(
    () => getMergeCategory(entries, selectedEntryIds),
    [entries, selectedEntryIds],
  );
  const canMergeSelection = useMemo(
    () => selectedEntries.length === selectedEntryIds.length && canMergeEntries(selectedEntries),
    [selectedEntries, selectedEntryIds.length],
  );
  const selectedEntryIdSet = useMemo(() => new Set(selectedEntryIds), [selectedEntryIds]);
  const footerItemCount = getFooterItemCount({
    activeTab,
    archiveSubTab,
    stats,
    memoCount: memoCountState,
    memoLoadedCount: memoListCount,
    archiveCount: archiveCountState,
    memoArchiveCount: memoArchiveCountState,
    loadedEntryCount: entries.length,
    loadedArchivedMemoCount: archivedMemos.length,
  });

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

  // Fetch stats
  const fetchStats = useCallback(async () => {
    const requestId = ++fetchStatsRequestRef.current;
    try {
      const s = await getStats(multiTagClassificationEnabled);
      if (fetchStatsRequestRef.current !== requestId) return;
      setStats(s);
      setArchiveCountState(s.archive);
      setMemoCountState(s.memoCount);
      setMemoArchiveCountState(s.memoArchive);
    } catch (err) {
      console.error('Failed to fetch stats:', err);
    }
  }, [multiTagClassificationEnabled]);

  const refreshDataOnce = useCallback(() => {
    if (activeTab !== 'memo') {
      setLoading(true);
    }
    setStorageRevision((value) => value + 1);
  }, [activeTab, setLoading]);

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
      clearEntries();
      clearArchivedMemos();
      refreshDataOnce();
    });
    return () => {
      offResume();
      offClipboard();
      offStorage();
    };
  }, [clearArchivedMemos, clearEntries, clearScheduledDataRefreshes, refreshDataOnce, scheduleDataRefresh]);

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
    [archiveEnabled, fetchStats, setEntries]
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
    [setEntries]
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
    [fetchStats, setEntries]
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
    [fetchStats, requestConfirm, setEntries, t]
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
    [fetchStats, setArchivedMemos]
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
    [fetchStats, requestConfirm, setArchivedMemos, t]
  );

  const handleEmptyArchive = useCallback(async () => {
    if (archiveClearPending || footerItemCount === 0) return;
    const scope = archiveSubTab === 'memos' ? t.memoSubTab : t.archiveSubTab;
    const confirmed = await requestConfirm({
      title: t.emptyArchiveTitle,
      message: t.emptyArchiveConfirm(scope, footerItemCount),
      confirmLabel: t.emptyArchive,
      tone: 'danger',
    });
    if (!confirmed) return;

    setArchiveClearPending(true);
    try {
      const removed = archiveSubTab === 'memos'
        ? await emptyMemoArchive()
        : await emptyArchive();
      if (archiveSubTab === 'memos') {
        clearArchivedMemos();
        setMemoArchiveCountState(0);
        setStats((current) => current ? { ...current, memoArchive: 0 } : current);
      } else {
        clearEntries();
        setArchiveCountState(0);
        setStats((current) => current ? { ...current, archive: 0 } : current);
      }
      await fetchStats();
      setDropNotice({ message: t.emptyArchiveSuccess(scope, removed), tone: 'success' });
    } catch (error) {
      console.error('Failed to empty recycle bin:', error);
      setDropNotice({ message: t.emptyArchiveFailed, tone: 'error' });
    } finally {
      setArchiveClearPending(false);
    }
  }, [archiveClearPending, archiveSubTab, clearArchivedMemos, clearEntries, fetchStats, footerItemCount, requestConfirm, t]);

  const handleClear = useCallback(async () => {
    const category = activeTab !== 'all' && activeTab !== 'archive' && activeTab !== 'memo'
      ? activeTab
      : undefined;
    let clearCategory = category;
    const clearDestination = archiveEnabled
      ? t.clearMovesToArchive
      : t.clearDeletesPermanently;

    if (category) {
      const clearScope = await new Promise<'current' | 'all' | null>((resolve) => {
        setConfirmDialog({
          title: t.clearHistory,
          message: `${t.clearScopeConfirm(getTabLabel(category, t))}\n\n${clearDestination}`,
          confirmLabel: t.clearCurrentTab,
          tone: 'danger',
          secondaryLabel: t.clearAllHistory,
          secondaryTone: 'danger',
          secondaryAfterPrimary: true,
          onSecondary: () => resolve('all'),
          resolve: (confirmed) => resolve(confirmed ? 'current' : null),
        });
      });
      if (!clearScope) return;
      if (clearScope === 'all') clearCategory = undefined;
    } else {
      const confirmed = await requestConfirm({
        title: t.clearHistory,
        message: `${t.clearConfirm}\n\n${clearDestination}`,
        confirmLabel: t.clearHistory,
        tone: 'danger',
      });
      if (!confirmed) return;
    }

    try {
      await clearUnpinned(
        archiveEnabled || undefined,
        clearCategory,
        clearCategory ? multiTagClassificationEnabled : false,
      );
      fetchEntries();
      fetchStats();
    } catch (err) {
      console.error('Failed to clear:', err);
    }
  }, [activeTab, archiveEnabled, fetchEntries, fetchStats, multiTagClassificationEnabled, requestConfirm, t]);

  // Handle tab change
  const handleTabChange = useCallback((tab: FilterTab) => {
    if (tab === activeTab) return;
    if (tab !== 'memo') {
      setLoading(true);
    }
    setSelectionMode(false);
    setSelectedEntryIds([]);
    setActiveTab(tab);
    void fetchStats();
  }, [activeTab, fetchStats, setLoading]);

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

  const handleReclassifyHistory = useCallback(async () => {
    if (reclassifyingHistory) return;
    const confirmed = await requestConfirm({
      title: t.reclassifyHistoryConfirmTitle,
      message: t.reclassifyHistoryConfirm,
      confirmLabel: t.reclassifyHistoryConfirmAction,
      tone: 'normal',
    });
    if (!confirmed) return;

    setReclassifyingHistory(true);
    try {
      const changed = await reclassifyClipboardEntries();
      refreshEntries();
      await fetchStats();
      setDropNotice({ message: t.reclassifyHistorySuccess(changed), tone: 'success' });
    } catch (error) {
      console.error('Failed to reclassify clipboard history:', error);
      setDropNotice({ message: t.reclassifyHistoryFailed, tone: 'error' });
    } finally {
      setReclassifyingHistory(false);
    }
  }, [fetchStats, reclassifyingHistory, refreshEntries, requestConfirm, t]);

  // Debounced search
  const [searchInput, setSearchInput] = useState('');

  const resetSelection = useCallback(() => {
    setSelectionMode(false);
    setSelectedEntryIds([]);
  }, []);

  useEffect(() => {
    if (!multiSelectModeEnabled && selectionMode) {
      resetSelection();
    }
  }, [multiSelectModeEnabled, resetSelection, selectionMode]);

  const handleSelectionToggle = useCallback((entry: ClipboardEntry) => {
    if (!multiSelectModeEnabled) return;
    setSelectionMode(true);
    setSelectedEntryIds((current) => current.includes(entry.id)
      ? current.filter((id) => id !== entry.id)
      : [...current, entry.id]);
  }, [multiSelectModeEnabled]);

  const handleMergeEntries = useCallback(async () => {
    if (selectedEntries.length < 2) {
      setDropNotice({ message: t.mergeNeedTwo, tone: 'error' });
      return;
    }
    if (selectedEntries.length !== selectedEntryIds.length) {
      setDropNotice({ message: t.mergeFailed, tone: 'error' });
      resetSelection();
      return;
    }
    if (selectedEntries.length > MAX_MERGE_SELECTION) {
      setDropNotice({ message: t.mergeLimitReached(MAX_MERGE_SELECTION), tone: 'error' });
      return;
    }
    if (!canMergeEntries(selectedEntries)) {
      setDropNotice({ message: t.mergeSameTypeOnly, tone: 'error' });
      return;
    }
    if (selectionCategory === 'image' && !memoEnabled) {
      setDropNotice({ message: t.mergeMemoRequired, tone: 'error' });
      return;
    }

    const mergeMode = await new Promise<'merge' | 'merge-delete' | null>((resolve) => {
      setConfirmDialog({
        title: t.mergeChoiceTitle,
        message: t.mergeChoiceMessage(selectedEntries.length, archiveEnabled),
        confirmLabel: t.mergeOnly,
        secondaryLabel: t.mergeAndDeleteOriginals,
        secondaryTone: 'danger',
        onSecondary: () => resolve('merge-delete'),
        resolve: (confirmed) => resolve(confirmed ? 'merge' : null),
      });
    });
    if (!mergeMode) return;

    const deleteOriginals = mergeMode === 'merge-delete';
    setBatchActionPending(true);
    try {
      const result = await mergeEntries(
        selectedEntries.map((entry) => entry.id),
        t.mergedImagesTitle(selectedEntries.length),
        deleteOriginals,
        archiveEnabled,
      );
      resetSelection();
      if (!result.created) {
        if (deleteOriginals && result.deletedOriginals > 0) {
          refreshEntries();
          await fetchStats();
          setDropNotice({ message: t.mergeDuplicateAndRemoved(archiveEnabled), tone: 'success' });
        } else {
          setDropNotice({ message: t.mergeDuplicate, tone: 'error' });
        }
        return;
      }
      if (result.kind === 'memo') {
        setSearchInput('');
        setSearchQuery('');
        setActiveTab('memo');
        setStorageRevision((value) => value + 1);
        setDropNotice({
          message: deleteOriginals
            ? t.mergeMemoCreatedAndRemoved(archiveEnabled)
            : t.mergeMemoCreated,
          tone: 'success',
        });
      } else {
        refreshEntries();
        await fetchStats();
        setDropNotice({
          message: deleteOriginals ? t.mergeCreatedAndRemoved(archiveEnabled) : t.mergeCreated,
          tone: 'success',
        });
      }
    } catch (error) {
      console.error('Failed to merge clipboard entries:', error);
      setDropNotice({ message: t.mergeFailed, tone: 'error' });
    } finally {
      setBatchActionPending(false);
    }
  }, [archiveEnabled, fetchStats, memoEnabled, refreshEntries, resetSelection, selectedEntries, selectedEntryIds.length, selectionCategory, t]);

  const handleDeleteSelectedEntries = useCallback(async () => {
    const ids = [...selectedEntryIds];
    if (ids.length === 0) return;
    const confirmed = await requestConfirm({
      title: t.deleteSelectedTitle,
      message: t.deleteSelectedConfirm(ids.length, archiveEnabled),
      confirmLabel: t.deleteSelected,
      tone: 'danger',
    });
    if (!confirmed) return;

    setBatchActionPending(true);
    try {
      const deleted = await deleteEntries(ids, archiveEnabled);
      resetSelection();
      refreshEntries();
      await fetchStats();
      setDropNotice({ message: t.deleteSelectedDone(deleted, archiveEnabled), tone: 'success' });
    } catch (error) {
      console.error('Failed to delete selected clipboard entries:', error);
      setDropNotice({ message: t.deleteSelectedFailed, tone: 'error' });
    } finally {
      setBatchActionPending(false);
    }
  }, [archiveEnabled, fetchStats, refreshEntries, requestConfirm, resetSelection, selectedEntryIds, t]);

  useEffect(() => {
    if (!multiSelectModeEnabled || !selectionMode || selectedEntryIds.length === 0) return;
    const handleDeleteShortcut = (event: KeyboardEvent) => {
      if (!isBatchDeleteShortcut(
        event.key,
        multiSelectModeEnabled,
        selectionMode,
        selectedEntryIds.length,
      ) || confirmDialog || batchActionPending) return;
      const target = event.target;
      if (target instanceof HTMLElement) {
        if (target.isContentEditable || target.matches('input, textarea, select')) return;
        if (target.closest('.glass-menu-panel, .confirm-dialog, .settings-gear-btn')) return;
      }
      event.preventDefault();
      void handleDeleteSelectedEntries();
    };
    window.addEventListener('keydown', handleDeleteShortcut);
    return () => window.removeEventListener('keydown', handleDeleteShortcut);
  }, [batchActionPending, confirmDialog, handleDeleteSelectedEntries, multiSelectModeEnabled, selectedEntryIds.length, selectionMode]);
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
  }, [searchInput, setLoading]);

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
            <Suspense fallback={<span className="title-actions-loading" aria-hidden="true" />}>
              {experimentalFeaturesEnabled && (
                <ExperimentalFeaturesButton
                  reclassifyingHistory={reclassifyingHistory}
                  onReclassifyHistory={handleReclassifyHistory}
                />
              )}
              <RemoteStorageButton onStorageModeChange={handleStorageModeChange} />
              <SettingsButton
                onShortcutChange={setCurrentShortcut}
                onVersionTitleTrigger={handleVersionTitleTrigger}
              />
            </Suspense>
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
          disabled={selectionMode}
        />
        {searchInput && !selectionMode && (
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
        <Suspense fallback={<div className="view-loading">{t.loading}</div>}>
          <MemoList
            searchQuery={searchQuery}
            archiveEnabled={archiveEnabled}
            refreshKey={storageRevision}
            onCountChange={handleMemoCountChange}
            onTotalCountChange={handleMemoTotalCountChange}
            onArchiveCountChange={setMemoArchiveCountState}
          />
        </Suspense>
      ) : activeTab === 'archive' ? (
        <div className="archive-view">
          {/* Archive sub-tabs */}
          <div className="archive-subtabs">
            <button
              type="button"
              className="archive-subtab"
              data-active={archiveSubTab === 'clipboard'}
              onClick={() => setArchiveSubTab('clipboard')}
            >
              {t.archiveSubTab} ({archiveCountState ?? 0})
            </button>
            <button
              type="button"
              className="archive-subtab"
              data-active={archiveSubTab === 'memos'}
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
                multiTagEnabled={multiTagClassificationEnabled}
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
              <div ref={archivedMemoListRef} className="entry-list" style={{ flex: 1, overflowY: 'auto' }}>
                {archivedMemos.map(memo => (
                  <ArchivedMemoItem
                    key={memo.id}
                    memo={memo}
                    onRestore={() => handleMemoRestore(memo.id)}
                    onPermanentDelete={() => handleMemoPermanentDelete(memo.id)}
                  />
                ))}
                <div ref={archivedMemoLoadMoreRef} className="entry-list-tail" style={{ minHeight: '18px', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
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
          multiTagEnabled={multiTagClassificationEnabled}
          showCategoryIndicator={effectiveEntryColorIndicatorEnabled}
          onRestore={handleRestore}
          onPermanentDelete={handlePermanentDelete}
          hasMore={entriesHasMore}
          loadingMore={entriesLoadingMore}
          onLoadMore={loadMoreEntries}
          selectionMode={selectionMode}
          multiSelectEnabled={multiSelectModeEnabled}
          selectedIds={selectedEntryIdSet}
          onSelectionToggle={handleSelectionToggle}
        />
      )}

      {/* Footer bar */}
      <div className="footer-bar">
        {selectionMode ? (
          <>
            <span className="footer-text batch-selection-count">{t.selectedCount(selectedEntryIds.length)}</span>
            <div className="batch-selection-actions">
              <button
                type="button"
                className="batch-cancel-btn batch-icon-btn"
                onClick={resetSelection}
                disabled={batchActionPending}
                title={t.cancel}
                aria-label={t.cancel}
              >
                <X size={13} />
              </button>
              <button
                type="button"
                className="batch-delete-btn batch-icon-btn"
                onClick={() => void handleDeleteSelectedEntries()}
                disabled={selectedEntryIds.length === 0 || batchActionPending}
                title={t.deleteSelected}
                aria-label={t.deleteSelected}
              >
                <Trash2 size={13} />
              </button>
              <button
                type="button"
                className="batch-merge-btn"
                onClick={() => void handleMergeEntries()}
                disabled={!canMergeSelection || batchActionPending}
                title={selectedEntryIds.length < 2
                  ? t.mergeNeedTwo
                  : selectedEntryIds.length > MAX_MERGE_SELECTION
                    ? t.mergeLimitReached(MAX_MERGE_SELECTION)
                    : !canMergeSelection
                      ? t.mergeSameTypeOnly
                      : undefined}
              >
                <Combine size={13} />
                <span>{selectionCategory === 'image' ? t.mergeToMemo : t.mergeEntries}</span>
              </button>
            </div>
          </>
        ) : (
          <>
            <span className="footer-text">
          {t.itemsCount(footerItemCount)}
          {activeTab === 'memo'
            ? stats?.memoSize != null && ` · ${t.memoStorage(formatBytes(stats.memoSize))}`
            : activeTab === 'archive' && archiveSubTab === 'memos'
            ? stats?.memoSize != null && ` · ${t.memoStorage(formatBytes(stats.memoSize))}`
            : stats?.clipboardSize != null && ` · ${t.clipboardStorage(formatBytes(stats.clipboardSize))}`}
            </span>
            {activeTab === 'archive' ? (
              <div className="footer-actions">
                <button
                  type="button"
                  className="clear-btn archive-empty-btn"
                  onClick={() => void handleEmptyArchive()}
                  disabled={footerItemCount === 0 || archiveClearPending}
                  title={t.emptyArchiveTitle}
                >
                  <Trash2 size={12} aria-hidden="true" />
                  <span>{archiveClearPending ? t.emptyArchivePending : t.emptyArchive}</span>
                </button>
              </div>
            ) : activeTab !== 'memo' && (
              <div className="footer-actions">
                {multiSelectModeEnabled && (
                  <button
                    type="button"
                    className="select-entries-btn"
                    onClick={() => setSelectionMode(true)}
                    title={t.selectEntries}
                  >
                    <ListChecks size={13} />
                    <span>{t.selectEntries}</span>
                  </button>
                )}
                <button
                  className="clear-btn"
                  onClick={handleClear}
                  title={t.clearHistory}
                >
                  {t.clearHistory}
                </button>
              </div>
            )}
          </>
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
