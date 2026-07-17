import { useState, useEffect, useCallback, useRef } from 'react';
import type React from 'react';
import { Archive, GripVertical, NotebookText, Pencil, Pin, Plus, Trash2 } from 'lucide-react';
import type { Memo } from '../types';
import {
  createMemo,
  updateMemo,
  deleteMemo,
  toggleMemoPin,
  archiveMemo,
  memoCount,
  memoArchiveCount,
  inferMemoTagTypes,
  type MemoAutoTagType,
} from '../api/memos';
import { useI18n } from '../i18n';
import { useInfiniteScroll } from '../hooks/useInfiniteScroll';
import { useMemoReorder } from '../hooks/useMemoReorder';
import { usePaginatedMemos } from '../hooks/usePaginatedMemos';
import {
  manualMemoTags,
  visibleMemoTags,
  type MemoTagLabels,
} from '../memoTagPresentation';
import { orderMemosForDisplay } from '../memoListOrdering';
import { formatRelativeTime, getCategoryColor } from '../utils';
import MemoRichEditor from './MemoRichEditor';
import {
  hasMemoImage,
  isImageOnlyMemo,
  parseMemoBody,
  renderMemoBody,
} from './MemoBody';

const MEMO_COLLAPSE_TEXT_LIMIT = 300;
const MEMO_COLLAPSE_LINE_LIMIT = 5;

const AUTO_TAG_CATEGORY: Record<MemoAutoTagType, Parameters<typeof getCategoryColor>[0]> = {
  image: 'image',
  email: 'email',
  path: 'file_path',
  link: 'link',
  code: 'code',
};

function getMemoTagLabels(t: ReturnType<typeof useI18n>['t']): MemoTagLabels {
  return {
    image: t.tabImage,
    email: t.tabEmail,
    path: t.tabPath,
    link: t.tabLink,
    code: t.tabCode,
  };
}

function isMemoBodyCollapsible(body: string): boolean {
  if (!body) return false;
  const blocks = parseMemoBody(body);
  const textLength = blocks.reduce((sum, block) => sum + (block.type === 'text' ? block.text.length : 0), 0);
  const lineCount = blocks.reduce((sum, block) => sum + (block.type === 'text' ? block.text.split('\n').length : 0), 0);
  return textLength > MEMO_COLLAPSE_TEXT_LIMIT || lineCount > MEMO_COLLAPSE_LINE_LIMIT || hasMemoImage(body);
}

function getMemoEditorHeight(body: string): number {
  const blocks = parseMemoBody(body);
  const text = blocks.filter(block => block.type === 'text').map(block => block.text).join('');
  const lineCount = text.split('\n').length;
  const imageCount = blocks.filter(block => block.type === 'image').length;
  if (text.length > 1600 || lineCount > 28 || imageCount >= 3) return 400;
  if (text.length > 700 || lineCount > 12 || imageCount > 0) return 300;
  return 120;
}

interface Props {
  searchQuery: string;
  archiveEnabled?: boolean;
  refreshKey?: number;
  onCountChange?: (count: number) => void;
  onTotalCountChange?: (count: number) => void;
  onArchiveCountChange?: (count: number) => void;
}

export default function MemoList({ searchQuery, archiveEnabled, refreshKey = 0, onCountChange, onTotalCountChange, onArchiveCountChange }: Props) {
  const { t } = useI18n();
  const { memos, setMemos, hasMore, loadingMore, refresh: fetchMemos, loadMore } = usePaginatedMemos(searchQuery, refreshKey);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editDraft, setEditDraft] = useState<{ title: string; body: string; tags: string } | null>(null);
  const [savingMemo, setSavingMemo] = useState(false);
  const [expandedMemoIds, setExpandedMemoIds] = useState<Set<number>>(() => new Set());
  const [editConflictMessage, setEditConflictMessage] = useState('');
  const [newMemoId, setNewMemoId] = useState<number | null>(null);

  const editingItemRef = useRef<HTMLDivElement>(null);
  const editingIdRef = useRef<number | null>(null);
  const editingVersionRef = useRef(1);
  const newMemoIdRef = useRef<number | null>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const loadMoreRef = useRef<HTMLDivElement>(null);
  useInfiniteScroll(listRef, loadMoreRef, hasMore && !loadingMore, () => { void loadMore(); });

  const refreshTotalCount = useCallback(async () => {
    try {
      const count = await memoCount();
      onTotalCountChange?.(count);
    } catch (err) {
      console.error('Failed to refresh memo count:', err);
    }
  }, [onTotalCountChange]);

  // Keep editingIdRef in sync with editingId state
  useEffect(() => {
    editingIdRef.current = editingId;
  }, [editingId]);

  useEffect(() => {
    onCountChange?.(memos.length);
  }, [memos.length, onCountChange]);

  // ─── Auto-scroll editing item into view ───────────────────
  useEffect(() => {
    if (editingId !== null && editingItemRef.current) {
      // Delay to let the editor expand first
      setTimeout(() => {
        editingItemRef.current?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      }, 80);
    }
  }, [editingId]);

  // ─── Editing handlers ─────────────────────────────────────
  const startEditing = useCallback((memo: Memo) => {
    editingIdRef.current = memo.id;
    editingVersionRef.current = memo.version;
    setEditConflictMessage('');
    setEditingId(memo.id);
    setEditDraft({ title: memo.title, body: memo.body, tags: manualMemoTags(memo.tags) });
  }, []);

  const stopEditing = useCallback(() => {
    editingIdRef.current = null;
    editingVersionRef.current = 1;
    newMemoIdRef.current = null;
    setNewMemoId(null);
    setEditingId(null);
    setEditDraft(null);
    setSavingMemo(false);
  }, []);

  const handleDraftChange = (field: 'title' | 'body' | 'tags', value: string) => {
    setEditDraft(prev => prev ? { ...prev, [field]: value } : null);
  };

  const handleSaveEditing = useCallback(async () => {
    const id = editingIdRef.current;
    if (id === null || editDraft === null || savingMemo) return;

    setSavingMemo(true);
    try {
      const autoTagTypes = await inferMemoTagTypes(editDraft.title, editDraft.body);
      const finalTags = manualMemoTags(editDraft.tags);
      const hasContent = editDraft.title.trim() || editDraft.body.trim() || finalTags.trim();
      if (!hasContent && newMemoIdRef.current === id) {
        await deleteMemo(id);
        setMemos(prev => prev.filter(m => m.id !== id));
        void refreshTotalCount();
      } else {
        const result = await updateMemo(
          id,
          editDraft.title,
          editDraft.body,
          finalTags,
          editingVersionRef.current,
        );
        if (result.conflict) {
          await fetchMemos();
          stopEditing();
          setEditConflictMessage(t.editConflict);
          return;
        }
        if (!result.updated) throw new Error('Memo update failed');
        const updatedAt = new Date().toISOString();
        setMemos(prev => prev.map(m =>
          m.id === id
            ? {
                ...m,
                title: editDraft.title,
                body: editDraft.body,
                tags: finalTags,
                auto_tags: autoTagTypes,
                updated_at: updatedAt,
                version: m.version + 1,
              }
            : m
        ));
      }
      stopEditing();
    } catch (err) {
      console.error('Failed to save memo:', err);
      setSavingMemo(false);
    }
  }, [editDraft, fetchMemos, refreshTotalCount, savingMemo, setMemos, stopEditing, t]);

  const handleCancelEditing = useCallback(async () => {
    const id = editingIdRef.current;
    if (id === null) {
      stopEditing();
      return;
    }

    try {
      if (newMemoIdRef.current === id) {
        await deleteMemo(id);
        setMemos(prev => prev.filter(m => m.id !== id));
        void refreshTotalCount();
      }
    } catch (err) {
      console.error('Failed to cancel memo editing:', err);
    } finally {
      stopEditing();
    }
  }, [refreshTotalCount, setMemos, stopEditing]);

  const expandMemo = useCallback((id: number) => {
    setExpandedMemoIds(prev => {
      if (prev.has(id)) return prev;
      const next = new Set(prev);
      next.add(id);
      return next;
    });
  }, []);

  const collapseMemo = useCallback((id: number) => {
    setExpandedMemoIds(prev => {
      if (!prev.has(id)) return prev;
      const next = new Set(prev);
      next.delete(id);
      return next;
    });
  }, []);

  // ─── Create new memo (toggle editor) ───────────────────────
  const handleCreate = async () => {
    try {
      // Use ref for synchronous check — state is stale during rapid clicks
      if (editingIdRef.current !== null) {
        await handleSaveEditing();
        return;
      }

      // Not editing — create new memo and start editing
      const newMemo = await createMemo('', '', '');
      setMemos(prev => [newMemo, ...prev]);
      void refreshTotalCount();
      newMemoIdRef.current = newMemo.id;
      setNewMemoId(newMemo.id);
      editingIdRef.current = newMemo.id;
      startEditing(newMemo);
    } catch (err) {
      console.error('Failed to create memo:', err);
    }
  };

  // ─── Delete / Archive ──────────────────────────────────────
  const handleDelete = async (id: number) => {
    try {
      if (archiveEnabled) {
        await archiveMemo(id);
        // Refresh archive count
        const count = await memoArchiveCount();
        onArchiveCountChange?.(count);
      } else {
        await deleteMemo(id);
      }
      setMemos(prev => prev.filter(m => m.id !== id));
      void refreshTotalCount();
      if (editingId === id) {
        setEditingId(null);
        setEditDraft(null);
      }
      fetchMemos();
    } catch (err) {
      console.error('Failed to delete/archive memo:', err);
    }
  };

  // ─── Toggle pin ───────────────────────────────────────────
  const handleTogglePin = async (id: number) => {
    try {
      await toggleMemoPin(id);
      fetchMemos();
    } catch (err) {
      console.error('Failed to toggle pin:', err);
    }
  };

  // ─── Pointer-based Drag-and-drop ────────────────────────────
  const canDrag = editingId === null && searchQuery.trim() === '';
  const {
    draggedId,
    dragOverId,
    dragGhostPos,
    dragGhostContent,
    handlePointerDown,
  } = useMemoReorder(memos, setMemos, canDrag, fetchMemos);

  // ─── Render helpers ───────────────────────────────────────
  const displayedMemos = orderMemosForDisplay(memos, newMemoId);

  const renderMemoItem = (memo: Memo, draggable: boolean) => {
    const isEditing = editingId === memo.id;
    const isDragging = draggedId === memo.id;
    const isDragOver = dragOverId === memo.id;
    const isExpanded = expandedMemoIds.has(memo.id);
    const hasImages = hasMemoImage(memo.body);
    const canExpand = !isEditing && isMemoBodyCollapsible(memo.body);
    const memoTagLabels = getMemoTagLabels(t);
    const detectedAutoTagTypes = isImageOnlyMemo(memo.body)
      ? (memo.auto_tags ?? []).filter((tag) => tag !== 'code')
      : memo.auto_tags ?? [];
    const displayedTags = visibleMemoTags(memo.tags, detectedAutoTagTypes, memoTagLabels);

    return (
      <div
        key={memo.id}
        className="memo-entry"
        data-memo-id={memo.id}
        ref={isEditing ? editingItemRef : undefined}
        style={{
          ...styles.memoItem,
          ...(isEditing ? styles.memoItemActive : {}),
          ...(isDragging ? styles.memoItemDragging : {}),
          ...(isDragOver ? { borderTop: '2px solid var(--memo-contrast)' } : {}),
          borderLeft: memo.pinned ? '3px solid var(--memo-contrast)' : '3px solid transparent',
        }}
        onClick={() => {
          if (!isEditing && canExpand && !isExpanded) expandMemo(memo.id);
        }}
      >
        <div style={styles.memoContent}>
          {/* Header row */}
          <div style={{ ...styles.memoHeader, ...(isEditing ? styles.memoHeaderEditing : {}) }}>
            {isEditing && editDraft ? (
              <input
                style={styles.editTitle}
                value={editDraft.title}
                onChange={(e) => handleDraftChange('title', e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Escape') void handleCancelEditing();
                  if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) void handleSaveEditing();
                }}
                placeholder={t.memoTitlePlaceholder}
                autoFocus
                onClick={(e) => e.stopPropagation()}
              />
            ) : (
              <span className="memo-selectable" style={styles.memoTitle}>{memo.title || t.memoUntitled}</span>
            )}
            {!isEditing && (
              <div className="memo-actions" style={styles.memoActions}>
                {/* Drag handle — visible only while the item can leave edit mode cleanly */}
                {draggable && (
                  <span
                    style={{
                      ...styles.dragHandle,
                      ...(canDrag ? {} : styles.dragHandleDisabled),
                    }}
                    onPointerDown={(e) => handlePointerDown(e, memo.id)}
                  >
                    <GripVertical size={14} />
                  </span>
                )}
                <div style={styles.hoverActions}>
                  <button
                    style={styles.actionBtn}
                    onClick={(e) => { e.stopPropagation(); startEditing(memo); }}
                    title={t.edit}
                  >
                    <Pencil size={13} />
                  </button>
                  <button
                    style={{
                      ...styles.actionBtn,
                      ...(memo.pinned ? styles.actionBtnActive : {}),
                    }}
                    onClick={(e) => { e.stopPropagation(); handleTogglePin(memo.id); }}
                    title={memo.pinned ? t.unpin : t.pin}
                  >
                    <Pin size={13} />
                  </button>
                  <button
                    style={{ ...styles.actionBtn, ...styles.deleteBtn }}
                    onClick={(e) => { e.stopPropagation(); handleDelete(memo.id); }}
                    title={archiveEnabled ? t.archiveSetting : t.delete}
                  >
                    {archiveEnabled ? <Archive size={13} /> : <Trash2 size={13} />}
                  </button>
                </div>
              </div>
            )}
          </div>

          {/* Content area */}
          {isEditing && editDraft ? (
            <div style={styles.inlineEditor} onClick={(e) => e.stopPropagation()}>
              <MemoRichEditor
                body={editDraft.body}
                placeholder={t.memoBodyPlaceholder}
                dragLabel={t.dragToReorder}
                deleteLabel={t.delete}
                initialHeight={getMemoEditorHeight(editDraft.body)}
                onChange={(body) => handleDraftChange('body', body)}
                onEscape={() => { void handleCancelEditing(); }}
                onSave={() => { void handleSaveEditing(); }}
              />
              <input
                style={styles.editTags}
                value={editDraft.tags}
                onChange={(e) => handleDraftChange('tags', e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Escape') void handleCancelEditing();
                  if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) void handleSaveEditing();
                }}
                placeholder={t.memoTagsPlaceholder}
              />
              <div style={styles.editActions}>
                <span style={styles.editHint}>Ctrl+Enter {t.save} / Esc {t.cancel}</span>
                <button
                  style={styles.cancelBtn}
                  onClick={(e) => {
                    e.stopPropagation();
                    void handleCancelEditing();
                  }}
                  disabled={savingMemo}
                >
                  {t.cancel}
                </button>
                <button
                  style={{
                    ...styles.saveBtn,
                    ...(savingMemo ? styles.saveBtnDisabled : {}),
                  }}
                  onClick={(e) => {
                    e.stopPropagation();
                    void handleSaveEditing();
                  }}
                  disabled={savingMemo}
                >
                  {savingMemo ? '...' : t.save}
                </button>
              </div>
            </div>
          ) : (
            <>
              {!hasImages ? (
                <pre
                  className="memo-selectable"
                  style={{
                    ...styles.rawPreview,
                    ...(isExpanded ? styles.rawPreviewExpanded : {}),
                  }}
                  onClick={(e) => {
                    if (isExpanded) e.stopPropagation();
                  }}
                >
                  {memo.body || '\u00A0'}
                </pre>
              ) : (
                <div
                  className="memo-selectable memo-preview"
                  style={{
                    ...styles.memoPreview,
                    ...(!isExpanded ? styles.memoPreviewWithImage : {}),
                    ...(isExpanded ? styles.memoPreviewExpanded : {}),
                  }}
                  onClick={(e) => {
                    if (isExpanded) e.stopPropagation();
                  }}
                >
                  {renderMemoBody(memo.body, isExpanded ? 10000 : MEMO_COLLAPSE_TEXT_LIMIT, isExpanded ? 220 : 72)}
                </div>
              )}
              {/* Timestamps */}
              <div style={styles.timestampRow}>
                <div style={styles.timestampLeft}>
                  <span className="memo-time" style={styles.timestamp}>{formatRelativeTime(memo.created_at, t)}</span>
                  {memo.updated_at && memo.updated_at !== memo.created_at && (
                    <span style={styles.editedBadge}>
                      {t.editedAt(formatRelativeTime(memo.updated_at, t))}
                    </span>
                  )}
                  {memo.pinned && <Pin size={12} style={styles.pinBadge} />}
                </div>
                <div style={styles.timestampRight}>
                  {displayedTags.length > 0 && (
                    <div style={styles.tags}>
                      {displayedTags.map((tag, i) => {
                        const color = tag.type ? getCategoryColor(AUTO_TAG_CATEGORY[tag.type]) : null;
                        return (
                          <span key={i} style={{
                            ...styles.tag,
                            ...(color ? {
                              borderColor: `${color}55`,
                              color,
                            } : styles.manualTag),
                          }}>{tag.label}</span>
                        );
                      })}
                    </div>
                  )}
                  {canExpand && (
                    <button
                      style={styles.expandBtn}
                      onClick={(e) => {
                        e.stopPropagation();
                        if (isExpanded) {
                          collapseMemo(memo.id);
                        } else {
                          expandMemo(memo.id);
                        }
                      }}
                    >
                      {isExpanded ? t.memoShowLess : t.memoShowMore}
                    </button>
                  )}
                </div>
              </div>
            </>
          )}
        </div>
      </div>
    );
  };

  // ─── Empty state ──────────────────────────────────────────
  if (memos.length === 0) {
    return (
      <div style={styles.container}>
        <div style={{ padding: '8px 12px' }}>
          <button className="memo-new-button" style={styles.newBtn} onClick={handleCreate}><Plus size={14} strokeWidth={2.25} /> {t.memoNew}</button>
        </div>
        <div style={styles.empty}>
          <NotebookText style={styles.emptyIcon} />
          <span style={styles.emptyText}>{t.memoEmpty}</span>
          <span style={styles.emptyHint}>{t.memoEmptyHint}</span>
        </div>
      </div>
    );
  }

  // ─── Main render ──────────────────────────────────────────
  return (
    <div style={styles.container}>
        <div style={{ padding: '8px 12px', flexShrink: 0 }}>
          <button className="memo-new-button" style={styles.newBtn} onClick={handleCreate}><Plus size={14} strokeWidth={2.25} /> {t.memoNew}</button>
          {editConflictMessage && <div style={styles.editConflict}>{editConflictMessage}</div>}
        </div>
      <div ref={listRef} className="entry-list" style={styles.list}>
        {displayedMemos.map(m => renderMemoItem(
          m,
          !m.pinned && m.id !== newMemoId,
        ))}
        <div ref={loadMoreRef} className="entry-list-tail" style={styles.loadMore} aria-hidden={!loadingMore}>
          {loadingMore && <div style={styles.loadMoreSpinner} />}
        </div>
      </div>
      {/* Floating ghost clone that follows cursor during drag */}
      {dragGhostPos && draggedId !== null && (
        <div className="memo-drag-ghost" style={{
          position: 'fixed',
          left: dragGhostPos.x + 12,
          top: dragGhostPos.y - 12,
          pointerEvents: 'none',
          zIndex: 9999,
          background: 'var(--memo-contrast-bg, #f5f5f5)',
          border: '1px solid var(--memo-contrast)',
          borderRadius: '6px',
          padding: '6px 12px',
          fontSize: '12px',
          fontWeight: 500,
          color: 'var(--text-primary)',
          maxWidth: '200px',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          whiteSpace: 'nowrap',
          opacity: 0.9,
          boxShadow: '0 2px 8px rgba(0,0,0,0.15)',
        }}>
          {dragGhostContent}
        </div>
      )}
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    flex: 1,
    display: 'flex',
    flexDirection: 'column',
    overflow: 'hidden',
  },
  newBtn: {
    width: '100%',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    gap: '4px',
    minHeight: '34px',
    padding: '8px 0',
  },
  editConflict: {
    marginTop: '6px',
    color: 'var(--danger)',
    fontSize: '11px',
    lineHeight: 1.4,
  },
  list: {
    flex: 1,
    overflowY: 'auto',
    overflowX: 'hidden',
  },
  loadMore: {
    minHeight: '18px',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
  },
  loadMoreSpinner: {
    width: '14px',
    height: '14px',
    border: '2px solid var(--border)',
    borderTopColor: 'var(--memo-contrast)',
    borderRadius: '50%',
    animation: 'spin 0.8s linear infinite',
  },
  empty: {
    flex: 1,
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    justifyContent: 'center',
    gap: '8px',
    padding: '40px 20px',
  },
  emptyIcon: {
    width: '36px',
    height: '36px',
    opacity: 0.5,
  },
  emptyText: {
    fontSize: '13px',
    color: 'var(--text-secondary)',
    fontWeight: 500,
  },
  emptyHint: {
    fontSize: '11px',
    color: 'var(--text-muted)',
  },

  // ─── Memo item ────────────────────────────────────────────
  memoItem: {
    padding: '10px 12px',
    transition: 'background 0.15s ease, border-top 0.15s ease',
    userSelect: 'none' as const,
    cursor: 'default',
  },
  memoItemActive: {
    background: 'var(--memo-item-hover-bg)',
  },
  memoItemDragging: {
    opacity: 0.4,
    background: 'var(--memo-item-hover-bg)',
  },
  memoContent: {
    display: 'flex',
    flexDirection: 'column',
    gap: '4px',
  },
  memoHeader: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    background: 'var(--memo-contrast-bg)',
    padding: '3px 6px',
    borderRadius: '4px',
    margin: '-2px -4px 2px -4px',
    minHeight: '26px',
  },
  memoHeaderEditing: {
    width: '100%',
    boxSizing: 'border-box',
    margin: '0 0 2px 0',
  },
  memoTitle: {
    fontSize: '13px',
    fontWeight: 600,
    color: 'var(--text-primary)',
    flex: 1,
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
    lineHeight: '22px',
  },
  memoActions: {
    display: 'flex',
    alignItems: 'center',
    gap: '4px',
    flexShrink: 0,
  },

  // ─── Drag handle ──────────────────────────────────────────
  dragHandle: {
    width: '18px',
    height: '22px',
    color: 'var(--text-muted)',
    opacity: 0.4,
    cursor: 'grab',
    padding: 0,
    userSelect: 'none' as const,
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
  },
  dragHandleDisabled: {
    cursor: 'default',
    opacity: 0.2,
  },

  // ─── Hover action buttons ─────────────────────────────────
  hoverActions: {
    display: 'flex',
    alignItems: 'center',
    gap: '4px',
  },
  actionBtn: {
    width: '22px',
    height: '22px',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    border: 'none',
    borderRadius: '4px',
    background: 'transparent',
    color: 'var(--text-muted)',
    cursor: 'pointer',
    padding: 0,
    opacity: 0.5,
    transition: 'opacity 0.15s, background 0.12s',
  },
  actionBtnActive: {
    opacity: 1,
    color: 'var(--memo-contrast)',
  },
  deleteBtn: {
    color: '#ef4444',
  },

  // ─── Preview (non-editing) ────────────────────────────────
  memoPreview: {
    fontSize: '13px',
    color: 'var(--text-secondary)',
    margin: 0,
    lineHeight: 1.4,
    maxHeight: '60px',
    overflow: 'hidden',
    wordBreak: 'break-word',
  },
  memoPreviewWithImage: {
    maxHeight: '96px',
  },
  memoPreviewExpanded: {
    maxHeight: 'none',
    overflow: 'visible',
  },
  rawPreview: {
    margin: 0,
    fontSize: '12px',
    lineHeight: 1.4,
    color: 'var(--text-primary)',
    fontFamily: '"Cascadia Code", "Fira Code", "Consolas", monospace',
    padding: '6px 8px',
    borderRadius: '4px',
    whiteSpace: 'pre-wrap',
    wordBreak: 'break-all',
    maxHeight: '88px',
    overflowY: 'hidden',
  },
  rawPreviewExpanded: {
    maxHeight: 'none',
    overflowY: 'visible',
  },
  expandBtn: {
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    minHeight: '18px',
    border: 'none',
    background: 'transparent',
    color: 'var(--text-muted)',
    padding: 0,
    fontSize: '11px',
    fontWeight: 500,
    cursor: 'pointer',
    lineHeight: 1,
    whiteSpace: 'nowrap',
    flexShrink: 0,
  },
  tags: {
    display: 'flex',
    gap: '4px',
    flexWrap: 'wrap',
    alignItems: 'center',
    justifyContent: 'flex-end',
  },
  tag: {
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    minHeight: '18px',
    padding: '0 6px',
    borderRadius: '10px',
    fontSize: '10px',
    fontWeight: 600,
    textTransform: 'uppercase' as const,
    letterSpacing: '0.5px',
    borderWidth: '1px',
    borderStyle: 'solid',
    borderColor: 'transparent',
    background: 'transparent',
    lineHeight: 1,
  },
  manualTag: {
    background: 'transparent',
    borderColor: 'var(--memo-contrast)',
    color: 'var(--memo-contrast)',
  },
  timestampRow: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: '6px',
    marginTop: '2px',
    minHeight: '18px',
  },
  timestampLeft: {
    display: 'flex',
    alignItems: 'center',
    gap: '6px',
    minWidth: 0,
    flexShrink: 1,
  },
  timestampRight: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'flex-end',
    gap: '6px',
    minWidth: 0,
    maxWidth: '72%',
    flex: '0 1 auto',
    flexWrap: 'wrap',
  },
  timestamp: {
    fontSize: '11px',
    color: 'var(--text-muted)',
    lineHeight: '18px',
  },
  editedBadge: {
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    minHeight: '18px',
    fontSize: '10px',
    color: 'var(--memo-contrast)',
    background: 'var(--memo-contrast-bg, rgba(236,95,158,0.08))',
    padding: '0 6px',
    borderRadius: '8px',
    fontWeight: 500,
    lineHeight: 1,
  },
  pinBadge: {
    flexShrink: 0,
  },

  // ─── Inline editor ────────────────────────────────────────
  inlineEditor: {
    display: 'flex',
    flexDirection: 'column',
    gap: '6px',
  },
  editTitle: {
    border: 'none',
    outline: 'none',
    background: 'transparent',
    fontSize: '13px',
    fontWeight: 600,
    color: 'var(--text-primary)',
    padding: '4px 0',
    flex: 1,
    minWidth: 0,
    width: '100%',
    boxSizing: 'border-box',
  },
  editTags: {
    border: 'none',
    outline: 'none',
    background: 'transparent',
    fontSize: '11px',
    color: 'var(--memo-contrast)',
    padding: '4px 0',
  },
  editActions: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'flex-end',
    gap: '8px',
  },
  editHint: {
    fontSize: '10px',
    color: 'var(--text-muted)',
    marginRight: 'auto',
  },
  saveBtn: {
    fontSize: '12px',
    padding: '4px 12px',
    borderRadius: '4px',
    border: 'none',
    background: 'var(--memo-contrast)',
    color: '#fff',
    cursor: 'pointer',
    fontWeight: 500,
  },
  saveBtnDisabled: {
    opacity: 0.6,
    cursor: 'not-allowed',
  },
  cancelBtn: {
    fontSize: '12px',
    padding: '4px 12px',
    borderRadius: '4px',
    border: '1px solid var(--border)',
    background: 'transparent',
    color: 'var(--text-secondary)',
    cursor: 'pointer',
  },
};
