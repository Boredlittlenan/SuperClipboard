import { useCallback, useEffect, useRef, useState } from 'react';
import type { FilterTab, Stats } from '../types';
import { getTabLabel } from '../utils';
import { useI18n } from '../i18n';
import { getSetting, setSetting } from '../api/settings';

interface Props {
  activeTab: FilterTab;
  onTabChange: (tab: FilterTab) => void;
  stats: Stats | null;
  memoEnabled?: boolean;
  memoCount?: number | null;
  archiveEnabled?: boolean;
  archiveCount?: number | null;
  categorySortingEnabled?: boolean;
}

const TAB_ORDER_SETTING_KEY = 'category_tab_order';
const DEFAULT_CLIPBOARD_TABS: FilterTab[] = ['all', 'text', 'link', 'image', 'code', 'email', 'file_path'];
const DRAG_START_THRESHOLD = 6;
type InsertSide = 'before' | 'after';

function normalizeTabOrder(value: unknown): FilterTab[] {
  const validTabs = new Set<FilterTab>(DEFAULT_CLIPBOARD_TABS);
  const ordered = Array.isArray(value)
    ? value.filter((tab): tab is FilterTab => validTabs.has(tab as FilterTab))
    : [];
  return [
    ...ordered.filter((tab, index) => ordered.indexOf(tab) === index),
    ...DEFAULT_CLIPBOARD_TABS.filter((tab) => !ordered.includes(tab)),
  ];
}

export default function CategoryTabs({
  activeTab,
  onTabChange,
  stats,
  memoEnabled,
  memoCount,
  archiveEnabled,
  archiveCount,
  categorySortingEnabled = true,
}: Props) {
  const { t } = useI18n();
  const [baseTabs, setBaseTabs] = useState<FilterTab[]>(DEFAULT_CLIPBOARD_TABS);
  const [draggingTab, setDraggingTab] = useState<FilterTab | null>(null);
  const [insertIndicator, setInsertIndicator] = useState<{ tab: FilterTab; side: InsertSide } | null>(null);
  const orderRef = useRef(baseTabs);
  const dragRef = useRef<{
    tab: FilterTab;
    startX: number;
    started: boolean;
    pointerId: number;
  } | null>(null);
  const suppressClickRef = useRef(false);

  useEffect(() => {
    orderRef.current = baseTabs;
  }, [baseTabs]);

  useEffect(() => {
    getSetting(TAB_ORDER_SETTING_KEY)
      .then((value) => {
        if (!value) return;
        setBaseTabs(normalizeTabOrder(JSON.parse(value)));
      })
      .catch((err) => console.error('Failed to load category tab order:', err));
  }, []);

  const orderedBaseTabs = normalizeTabOrder(baseTabs);
  const tabs: FilterTab[] = [
    ...(memoEnabled ? ['memo' as FilterTab] : []),
    ...orderedBaseTabs,
    ...(archiveEnabled ? ['archive' as FilterTab] : []),
  ];

  const getCount = useCallback(
    (tab: FilterTab): number | null => {
      if (tab === 'memo') return memoCount ?? null;
      if (tab === 'archive') return archiveCount ?? null;
      if (!stats) return null;
      if (tab === 'all') return stats.total;
      return (stats as unknown as Record<string, number>)[tab] ?? null;
    },
    [stats, memoCount, archiveCount]
  );

  const scrollRef = useRef<HTMLDivElement>(null);

  const handleWheel = useCallback((e: React.WheelEvent<HTMLDivElement>) => {
    if (scrollRef.current) {
      scrollRef.current.scrollLeft += e.deltaY || e.deltaX;
    }
  }, []);

  const persistTabOrder = useCallback((order: FilterTab[]) => {
    setSetting(TAB_ORDER_SETTING_KEY, JSON.stringify(order)).catch((err) => {
      console.error('Failed to save category tab order:', err);
    });
  }, []);

  const moveTab = useCallback((from: FilterTab, to: FilterTab, side: InsertSide) => {
    if (from === to) return;
    if (!DEFAULT_CLIPBOARD_TABS.includes(from) || !DEFAULT_CLIPBOARD_TABS.includes(to)) return;

    setBaseTabs((current) => {
      const next = normalizeTabOrder(current);
      const fromIndex = next.indexOf(from);
      if (fromIndex === -1) return current;
      const [moved] = next.splice(fromIndex, 1);
      const targetIndex = next.indexOf(to);
      if (targetIndex === -1) return current;
      next.splice(side === 'after' ? targetIndex + 1 : targetIndex, 0, moved);
      orderRef.current = next;
      return next;
    });
  }, []);

  const finishDrag = useCallback(() => {
    const state = dragRef.current;
    if (!state) return;

    if (state.started) {
      suppressClickRef.current = true;
      persistTabOrder(orderRef.current);
      window.setTimeout(() => {
        suppressClickRef.current = false;
      }, 0);
    }

    dragRef.current = null;
    setDraggingTab(null);
    setInsertIndicator(null);
  }, [persistTabOrder]);

  const handleTabPointerDown = useCallback((tab: FilterTab, event: React.PointerEvent<HTMLButtonElement>) => {
    if (!categorySortingEnabled || event.button !== 0 || !DEFAULT_CLIPBOARD_TABS.includes(tab)) return;
    dragRef.current = {
      tab,
      startX: event.clientX,
      started: false,
      pointerId: event.pointerId,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  }, [categorySortingEnabled]);

  const handleTabPointerMove = useCallback((event: React.PointerEvent<HTMLButtonElement>) => {
    const state = dragRef.current;
    if (!state || state.pointerId !== event.pointerId) return;

    if (!state.started && Math.abs(event.clientX - state.startX) < DRAG_START_THRESHOLD) {
      return;
    }

    if (!state.started) {
      state.started = true;
      setDraggingTab(state.tab);
    }

    const target = document
      .elementFromPoint(event.clientX, event.clientY)
      ?.closest<HTMLElement>('[data-clipboard-tab]');
    const targetTab = target?.dataset.clipboardTab as FilterTab | undefined;
    if (!target || !targetTab || targetTab === state.tab || !DEFAULT_CLIPBOARD_TABS.includes(targetTab)) {
      setInsertIndicator(null);
      return;
    }

    const rect = target.getBoundingClientRect();
    const side: InsertSide = event.clientX > rect.left + rect.width / 2 ? 'after' : 'before';
    setInsertIndicator({ tab: targetTab, side });
    moveTab(state.tab, targetTab, side);
  }, [moveTab]);

  const handleTabClick = useCallback((tab: FilterTab) => {
    if (suppressClickRef.current) return;
    onTabChange(tab);
  }, [onTabChange]);

  return (
    <div style={styles.container}>
      <div ref={scrollRef} style={styles.scrollArea} onWheel={handleWheel}>
        {tabs.map((tab) => {
          const isActive = tab === activeTab;
          const count = getCount(tab);
          const draggable = categorySortingEnabled && DEFAULT_CLIPBOARD_TABS.includes(tab);
          const insertBefore = insertIndicator?.tab === tab && insertIndicator.side === 'before';
          const insertAfter = insertIndicator?.tab === tab && insertIndicator.side === 'after';
          return (
            <button
              key={tab}
              data-clipboard-tab={tab}
              onClick={() => handleTabClick(tab)}
              onPointerDown={(e) => handleTabPointerDown(tab, e)}
              onPointerMove={handleTabPointerMove}
              onPointerUp={finishDrag}
              onPointerCancel={finishDrag}
              title={draggable ? t.dragToReorder : undefined}
              style={{
                ...styles.tab,
                ...(draggable ? styles.tabDraggable : {}),
                ...(isActive
                  ? (tab === 'memo' ? styles.tabActiveMemo : styles.tabActive)
                  : {}),
                ...(draggingTab === tab ? styles.tabDragging : {}),
                ...(insertBefore ? styles.tabInsertBefore : {}),
                ...(insertAfter ? styles.tabInsertAfter : {}),
              }}
              onMouseEnter={(e) => {
                if (!isActive && draggingTab === null) {
                  e.currentTarget.style.background = 'var(--hover-bg)';
                }
              }}
              onMouseLeave={(e) => {
                if (!isActive && draggingTab === null) {
                  e.currentTarget.style.background = 'transparent';
                }
              }}
            >
              <span style={styles.tabLabel}>{tab === 'memo' ? t.memoTab : getTabLabel(tab, t)}</span>
              {count !== null && count > 0 && (
                <span
                  style={{
                    ...styles.badge,
                    ...(isActive ? styles.badgeActive : {}),
                  }}
                >
                  {count > 999 ? '999+' : count}
                </span>
              )}
            </button>
          );
        })}
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    borderBottom: '1px solid var(--border)',
    background: 'var(--surface)',
    overflow: 'hidden',
  },
  scrollArea: {
    display: 'flex',
    gap: '2px',
    padding: '4px 8px',
    overflowX: 'auto',
    scrollbarWidth: 'none',
  },
  tab: {
    display: 'flex',
    alignItems: 'center',
    gap: '4px',
    padding: '6px 12px',
    border: 'none',
    borderRadius: '6px',
    background: 'transparent',
    color: 'var(--text-secondary)',
    fontSize: '12px',
    fontWeight: 500,
    cursor: 'pointer',
    whiteSpace: 'nowrap',
    transition: 'all 0.15s ease',
  },
  tabDraggable: {
    cursor: 'grab',
    touchAction: 'none',
  },
  tabDragging: {
    opacity: 0.78,
    cursor: 'grabbing',
    transform: 'translateY(-1px) scale(1.03)',
    boxShadow: '0 4px 12px rgba(0, 0, 0, 0.14)',
  },
  tabInsertBefore: {
    boxShadow: 'inset 2px 0 0 var(--accent)',
  },
  tabInsertAfter: {
    boxShadow: 'inset -2px 0 0 var(--accent)',
  },
  tabActive: {
    background: 'var(--accent)',
    color: '#ffffff',
  },
  tabActiveMemo: {
    background: 'var(--memo-contrast)',
    color: '#ffffff',
  },
  tabLabel: {
    lineHeight: 1,
  },
  badge: {
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    minWidth: '18px',
    height: '18px',
    padding: '0 5px',
    borderRadius: '9px',
    background: 'var(--border)',
    color: 'var(--text-secondary)',
    fontSize: '10px',
    fontWeight: 600,
    lineHeight: 1,
  },
  badgeActive: {
    background: 'rgba(255,255,255,0.25)',
    color: '#ffffff',
  },
};
