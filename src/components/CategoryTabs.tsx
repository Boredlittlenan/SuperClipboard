import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { FilterTab, Stats } from '../types';
import { getCategoryColor, getTabLabel } from '../utils';
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
  categoryTabSelectedColors?: boolean;
  modernUi?: boolean;
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
  categoryTabSelectedColors = false,
  modernUi = true,
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
  const tabSignature = tabs.join('|');

  const getActiveTabColor = (tab: FilterTab): string => {
    if (tab === 'memo') return 'var(--memo-contrast)';
    if (!categoryTabSelectedColors || tab === 'all' || tab === 'archive') return 'var(--accent)';
    return getCategoryColor(tab);
  };

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
  const tabRefs = useRef(new Map<FilterTab, HTMLButtonElement>());
  const [activeIndicator, setActiveIndicator] = useState({ left: 0, width: 0, visible: false });

  const updateActiveIndicator = useCallback(() => {
    const tabElement = tabRefs.current.get(activeTab);
    if (!tabElement) {
      setActiveIndicator((current) => (current.visible ? { ...current, visible: false } : current));
      return;
    }

    const next = {
      left: tabElement.offsetLeft,
      width: tabElement.offsetWidth,
      visible: true,
    };
    setActiveIndicator((current) => (
      current.left === next.left && current.width === next.width && current.visible === next.visible
        ? current
        : next
    ));
  }, [activeTab]);

  useLayoutEffect(() => {
    updateActiveIndicator();
    window.addEventListener('resize', updateActiveIndicator);
    return () => window.removeEventListener('resize', updateActiveIndicator);
  }, [archiveCount, memoCount, stats, tabSignature, updateActiveIndicator]);

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
    if (tab === activeTab) return;
    onTabChange(tab);
  }, [activeTab, onTabChange]);

  return (
    <div style={{ ...styles.container, ...(modernUi ? styles.containerModern : {}) }}>
      <div ref={scrollRef} style={{ ...styles.scrollArea, ...(modernUi ? styles.scrollAreaModern : {}) }} onWheel={handleWheel}>
        {modernUi && activeIndicator.visible && (
          <span
            className="tab-active-indicator"
            aria-hidden="true"
            style={{
              ...styles.activeIndicator,
              width: `${activeIndicator.width}px`,
              transform: `translateX(${activeIndicator.left}px)`,
              background: getActiveTabColor(activeTab),
            }}
          />
        )}
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
              data-active={isActive ? 'true' : undefined}
              ref={(node) => {
                if (node) {
                  tabRefs.current.set(tab, node);
                } else {
                  tabRefs.current.delete(tab);
                }
              }}
              onClick={() => handleTabClick(tab)}
              onPointerDown={(e) => handleTabPointerDown(tab, e)}
              onPointerMove={handleTabPointerMove}
              onPointerUp={finishDrag}
              onPointerCancel={finishDrag}
              title={draggable ? t.dragToReorder : undefined}
              style={{
                ...styles.tab,
                ...(modernUi ? styles.tabModern : {}),
                ...(draggable ? styles.tabDraggable : {}),
                ...(isActive
                  ? (tab === 'memo'
                    ? (modernUi ? styles.tabActiveMemoModern : styles.tabActiveMemo)
                    : (modernUi ? styles.tabActiveModern : styles.tabActive))
                  : {}),
                ...(isActive && !modernUi && categoryTabSelectedColors
                  ? { background: getActiveTabColor(tab) }
                  : {}),
                ...(draggingTab === tab ? { ...styles.tabDragging, ...(modernUi ? styles.tabDraggingModern : {}) } : {}),
                ...(insertBefore ? (modernUi ? styles.tabInsertBeforeModern : styles.tabInsertBefore) : {}),
                ...(insertAfter ? (modernUi ? styles.tabInsertAfterModern : styles.tabInsertAfter) : {}),
              }}
            >
              <span style={styles.tabLabel}>{tab === 'memo' ? t.memoTab : getTabLabel(tab, t)}</span>
              {count !== null && count > 0 && (
                  <span
                    style={{
                    ...styles.badge,
                    ...(modernUi ? styles.badgeModern : {}),
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
  containerModern: {
    borderBottom: 'none',
    position: 'relative',
    zIndex: 20,
    marginBottom: '3px',
    background: 'var(--tab-glass)',
    boxShadow: '0 5px 12px rgba(15, 23, 42, 0.035), inset 0 1px 0 var(--hairline-highlight)',
    backdropFilter: 'blur(22px) saturate(1.36)',
    WebkitBackdropFilter: 'blur(22px) saturate(1.36)',
  },
  scrollArea: {
    display: 'flex',
    gap: '2px',
    padding: '4px 8px',
    overflowX: 'auto',
    scrollbarWidth: 'none',
  },
  scrollAreaModern: {
    position: 'relative',
    gap: '4px',
    padding: '5px 8px 6px',
  },
  tab: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    gap: '4px',
    minHeight: '30px',
    padding: '6px 12px',
    border: 'none',
    borderRadius: '6px',
    background: 'transparent',
    color: 'var(--text-secondary)',
    fontSize: '12px',
    fontWeight: 500,
    lineHeight: 1,
    cursor: 'pointer',
    whiteSpace: 'nowrap',
    transition: 'all 0.15s ease',
  },
  tabModern: {
    position: 'relative',
    zIndex: 1,
    padding: '6px 11px',
    borderRadius: '999px',
    fontWeight: 600,
    transition: 'background 0.15s ease, color 0.15s ease, box-shadow 0.15s ease, transform 0.15s ease, opacity 0.15s ease',
  },
  tabDraggable: {
    cursor: 'grab',
    touchAction: 'none',
  },
  tabDragging: {
    opacity: 0.5,
    cursor: 'grabbing',
  },
  tabDraggingModern: {
    opacity: 0.78,
    transform: 'translateY(-2px) scale(1.03)',
    boxShadow: '0 8px 18px rgba(0, 0, 0, 0.18)',
  },
  tabInsertBefore: {
    boxShadow: 'inset 2px 0 0 var(--accent)',
  },
  tabInsertAfter: {
    boxShadow: 'inset -2px 0 0 var(--accent)',
  },
  tabInsertBeforeModern: {
    boxShadow: 'inset 3px 0 0 var(--accent), 0 0 0 1px var(--accent-ring)',
  },
  tabInsertAfterModern: {
    boxShadow: 'inset -3px 0 0 var(--accent), 0 0 0 1px var(--accent-ring)',
  },
  tabActive: {
    background: 'var(--accent)',
    color: '#ffffff',
  },
  tabActiveModern: {
    background: 'transparent',
    color: '#ffffff',
    boxShadow: 'none',
  },
  tabActiveMemo: {
    background: 'var(--memo-contrast)',
    color: '#ffffff',
  },
  tabActiveMemoModern: {
    background: 'transparent',
    color: '#ffffff',
    boxShadow: 'none',
  },
  tabLabel: {
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    minHeight: '18px',
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
  badgeModern: {
    background: 'var(--surface-raised)',
    boxShadow: 'inset 0 0 0 1px var(--border)',
  },
  badgeActive: {
    background: 'rgba(255,255,255,0.25)',
    color: '#ffffff',
  },
  activeIndicator: {
    position: 'absolute',
    zIndex: 0,
    top: '5px',
    bottom: '6px',
    left: 0,
    borderRadius: '999px',
    pointerEvents: 'none',
    boxShadow: 'inset 0 1px 0 rgba(255, 255, 255, 0.24), 0 1px 2px rgba(15, 23, 42, 0.1)',
    transition: 'transform 0.26s cubic-bezier(0.2, 0.8, 0.2, 1), width 0.26s cubic-bezier(0.2, 0.8, 0.2, 1), background 0.18s ease',
  },
};
