import { useState, useRef, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { Archive, Check, Download, ExternalLink, Image as ImageIcon, Maximize2, Pencil, Pin, RotateCcw, Trash2 } from 'lucide-react';
import type { ClipboardEntry } from '../types';
import { getArchiveDaysRemaining, getArchiveTone, getCategoryColor, getCategoryLabel, formatRelativeTime } from '../utils';
import { useI18n } from '../i18n';
import { exportClipboardImage, getEntryContent, type UpdateResult } from '../api/clipboard';
import ImagePreviewDialog from './ImagePreviewDialog';
import { shouldToggleEntrySelection } from '../clipboardMerge';

interface Props {
  entry: ClipboardEntry;
  onCopy: (id: number, useOriginal?: boolean) => void;
  onDelete: (id: number) => void;
  onTogglePin: (id: number) => void;
  onEdit: (id: number, content: string, expectedVersion: number) => Promise<UpdateResult>;
  rawPreview?: boolean;
  isArchive?: boolean;
  archiveEnabled?: boolean;
  multiTagEnabled?: boolean;
  showCategoryIndicator?: boolean;
  onRestore?: (id: number) => void;
  onPermanentDelete?: (id: number) => void;
  selectionMode?: boolean;
  multiSelectEnabled?: boolean;
  selected?: boolean;
  onSelectionToggle?: (entry: ClipboardEntry) => void;
}

export default function ClipboardItem({ entry, onCopy, onDelete, onTogglePin, onEdit, rawPreview, isArchive, archiveEnabled, multiTagEnabled, showCategoryIndicator = true, onRestore, onPermanentDelete, selectionMode = false, multiSelectEnabled = false, selected = false, onSelectionToggle }: Props) {
  const [editing, setEditing] = useState(false);
  const [editContent, setEditContent] = useState(entry.content);
  const [showOriginal, setShowOriginal] = useState(false);
  const [saving, setSaving] = useState(false);
  const [editError, setEditError] = useState('');
  const [imageContent, setImageContent] = useState(entry.category === 'image' ? entry.content : '');
  const [imageLoading, setImageLoading] = useState(false);
  const [exportingImage, setExportingImage] = useState(false);
  const [imageExportError, setImageExportError] = useState('');
  const [imagePreviewOpen, setImagePreviewOpen] = useState(false);
  const [imagePreviewLoading, setImagePreviewLoading] = useState(false);
  const { t } = useI18n();
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const imageContainerRef = useRef<HTMLDivElement>(null);

  const categoryColor = getCategoryColor(entry.category);
  const categoryTags = multiTagEnabled
    ? Array.from(new Set([entry.category, ...(entry.category_tags ?? [])]))
    : [entry.category];
  const categoryBarBackground = categoryTags.length <= 1
    ? categoryColor
    : `linear-gradient(to bottom, ${categoryTags.map((category, index) => {
        const start = (index / categoryTags.length) * 100;
        const end = ((index + 1) / categoryTags.length) * 100;
        const color = getCategoryColor(category);
        return `${color} ${start}% ${end}%`;
      }).join(', ')})`;
  const isImage = entry.category === 'image';
  const isLink = entry.category === 'link';
  const originalContent = entry.original_content;
  const hasOriginal = originalContent != null && originalContent !== entry.content;
  const hasEditMetadata = hasOriginal && entry.updated_at != null;
  const archiveDaysRemaining = isArchive && entry.archived_at ? getArchiveDaysRemaining(entry.archived_at) : null;

  useEffect(() => {
    setImageContent(isImage ? entry.content : '');
  }, [entry.content, entry.id, isImage]);

  useEffect(() => {
    if (selectionMode) {
      setEditing(false);
      setEditError('');
    }
  }, [selectionMode]);

  useEffect(() => {
    if (!isImage || entry.content) return;
    const container = imageContainerRef.current;
    if (!container) return;
    let cancelled = false;
    let requested = false;

    const loadImage = () => {
      if (requested) return;
      requested = true;
      setImageLoading(true);
      getEntryContent(entry.id)
        .then((content) => {
          if (!cancelled && content) setImageContent(content);
        })
        .catch((error) => console.error('Failed to load clipboard image:', error))
        .finally(() => {
          if (!cancelled) setImageLoading(false);
        });
    };

    if (typeof IntersectionObserver === 'undefined') {
      loadImage();
      return () => {
        cancelled = true;
      };
    }

    const observer = new IntersectionObserver((records) => {
      if (records.some((record) => record.isIntersecting)) {
        observer.disconnect();
        loadImage();
      }
    }, { rootMargin: '100px 0px' });
    observer.observe(container);
    return () => {
      cancelled = true;
      observer.disconnect();
    };
  }, [entry.content, entry.id, isImage]);

  const handleOpenInBrowser = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    let url = entry.content.trim();
    // Prepend https:// if no scheme present
    if (!/^https?:\/\//i.test(url) && !/^ftp:\/\//i.test(url)) {
      url = 'https://' + url;
    }
    invoke('open_url', { url }).catch(console.error);
  }, [entry.content]);

  const handleEditClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setEditContent(entry.content);
    setEditError('');
    setEditing(true);
    setTimeout(() => textareaRef.current?.focus(), 50);
  }, [entry.content]);

  const handleSave = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (saving) return;
    if (editContent === entry.content) {
      setEditing(false);
      setEditError('');
      return;
    }
    setSaving(true);
    try {
      const result = await onEdit(entry.id, editContent, entry.version);
      if (result.conflict) {
        setEditError(t.editConflict);
        setEditing(false);
      } else if (result.updated) {
        setEditing(false);
      } else {
        throw new Error('Clipboard entry update failed');
      }
    } catch (err) {
      console.error('Failed to save edit:', err);
    } finally {
      setSaving(false);
    }
  }, [entry.content, entry.id, entry.version, editContent, onEdit, saving, t.editConflict]);

  const handleCancel = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setEditing(false);
    setEditContent(entry.content);
    setEditError('');
  }, [entry.content]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      handleCancel(e as unknown as React.MouseEvent);
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      handleSave(e as unknown as React.MouseEvent);
    }
  }, [handleCancel, handleSave]);

  const handleToggleOriginal = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setShowOriginal(prev => !prev);
  }, []);

  const handleExportImage = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (exportingImage) return;

    const path = await save({
      title: t.exportImage,
      defaultPath: `superclipboard-image-${entry.id}.png`,
      filters: [{ name: 'PNG', extensions: ['png'] }],
    });
    if (!path) return;

    setExportingImage(true);
    setImageExportError('');
    try {
      await exportClipboardImage(entry.id, path);
    } catch (error) {
      console.error('Failed to export clipboard image:', error);
      setImageExportError(t.exportImageFailed);
    } finally {
      setExportingImage(false);
    }
  }, [entry.id, exportingImage, t.exportImage, t.exportImageFailed]);

  const handleOpenImagePreview = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation();
    setImagePreviewOpen(true);
    if (imageContent) return;

    setImagePreviewLoading(true);
    try {
      const content = await getEntryContent(entry.id);
      if (content) setImageContent(content);
    } catch (error) {
      console.error('Failed to load clipboard image preview:', error);
    } finally {
      setImagePreviewLoading(false);
    }
  }, [entry.id, imageContent]);

  return (
    <div
      className={`clipboard-entry${selectionMode ? ' is-selecting' : ''}${selected ? ' is-selected' : ''}`}
      style={styles.container}
      onClick={(event) => {
        if (!editing && shouldToggleEntrySelection(selectionMode, multiSelectEnabled, event.ctrlKey)) {
          if (event.ctrlKey) event.preventDefault();
          onSelectionToggle?.(entry);
        } else if (!editing) {
          onCopy(entry.id);
        }
      }}
      title={selectionMode
        ? selected
          ? t.deselectItem
          : t.selectItem
        : editing
          ? undefined
          : t.clickToCopy}
    >
      {/* Category indicator */}
      {showCategoryIndicator && (
        <div style={{ ...styles.categoryBar, background: categoryBarBackground }} />
      )}

      <div style={styles.body}>
        {/* Header row */}
        <div style={styles.header}>
          <div style={styles.headerLeft}>
            {selectionMode && (
              <button
                type="button"
                className={`entry-selection-check${selected ? ' is-checked' : ''}`}
                role="checkbox"
                aria-checked={selected}
                aria-label={selected ? t.deselectItem : t.selectItem}
                onClick={(event) => {
                  event.stopPropagation();
                  onSelectionToggle?.(entry);
                }}
              >
                {selected && <Check size={12} strokeWidth={2.6} />}
              </button>
            )}
            <div style={styles.categoryBadges}>
              {categoryTags.map((category) => {
                const color = getCategoryColor(category);
                return (
                  <span
                    key={category}
                    className="entry-category-badge"
                    style={{
                      ...styles.categoryBadge,
                      background: `${color}20`,
                      color,
                    }}
                  >
                    {getCategoryLabel(category, t)}
                  </span>
                );
              })}
            </div>
            {!editing && !selectionMode && (
              <div className="entry-actions" style={styles.inlineActions}>
                {isArchive ? (
                  <>
                    {isImage && (
                      <button style={styles.actionBtn} onClick={handleOpenImagePreview} title={t.previewImage} aria-label={t.previewImage}>
                        <Maximize2 size={13} />
                      </button>
                    )}
                    <button
                      style={styles.actionBtn}
                      onClick={(e) => {
                        e.stopPropagation();
                        onRestore?.(entry.id);
                      }}
                      title={t.restore}
                    >
                      <RotateCcw size={13} />
                    </button>
                    <button
                      style={{ ...styles.actionBtn, ...styles.deleteBtn }}
                      onClick={(e) => {
                        e.stopPropagation();
                        onPermanentDelete?.(entry.id);
                      }}
                      title={t.permanentDelete}
                    >
                      <Trash2 size={13} />
                    </button>
                  </>
                ) : (
                  <>
                    {isLink && (
                      <button
                        style={styles.actionBtn}
                        onClick={handleOpenInBrowser}
                        title={t.openInBrowser || 'Open in browser'}
                      >
                        <ExternalLink size={13} />
                      </button>
                    )}
                    {isImage && (
                      <button style={styles.actionBtn} onClick={handleOpenImagePreview} title={t.previewImage} aria-label={t.previewImage}>
                        <Maximize2 size={13} />
                      </button>
                    )}
                    {isImage && (
                      <button
                        style={{ ...styles.actionBtn, ...(exportingImage ? styles.actionBtnDisabled : {}) }}
                        onClick={handleExportImage}
                        title={t.exportImage}
                        aria-label={t.exportImage}
                        disabled={exportingImage}
                      >
                        <Download size={13} />
                      </button>
                    )}
                    {!isImage && (
                      <button
                        style={styles.actionBtn}
                        onClick={handleEditClick}
                        title={t.edit}
                      >
                        <Pencil size={13} />
                      </button>
                    )}
                    <button
                      style={{
                        ...styles.actionBtn,
                        ...(entry.pinned ? styles.actionBtnActive : {}),
                      }}
                      onClick={(e) => {
                        e.stopPropagation();
                        onTogglePin(entry.id);
                      }}
                      title={entry.pinned ? t.unpin : t.pin}
                    >
                      <Pin size={13} />
                    </button>
                    <button
                      style={{ ...styles.actionBtn, ...(archiveEnabled ? styles.archiveBtn : styles.deleteBtn) }}
                      onClick={(e) => {
                        e.stopPropagation();
                        onDelete(entry.id);
                      }}
                      title={archiveEnabled ? t.archiveSetting : t.delete}
                    >
                      {archiveEnabled ? <Archive size={13} /> : <Trash2 size={13} />}
                    </button>
                  </>
                )}
              </div>
            )}
          </div>
          <div style={styles.headerRight}>
            <span className="entry-time" style={styles.time}>{formatRelativeTime(entry.created_at, t)}</span>
            {entry.pinned && !isArchive && <Pin size={12} style={styles.pinBadge} />}
            {isArchive && entry.archived_at && (
              <span style={{ ...styles.archiveTimer, ...styles[`archiveTimer${getArchiveTone(archiveDaysRemaining ?? 0)}`] }}>
                {t.daysRemaining(archiveDaysRemaining ?? 0)}
              </span>
            )}
          </div>
        </div>

        {/* Content area */}
        {editing ? (
          <div style={styles.editContainer} onClick={(e) => e.stopPropagation()}>
            <textarea
              ref={textareaRef}
              value={editContent}
              onChange={(e) => setEditContent(e.target.value)}
              onKeyDown={handleKeyDown}
              style={styles.textarea}
              rows={Math.min(Math.max(editContent.split('\n').length, 3), 15)}
            />
            <div style={styles.editActions}>
              <span style={styles.editHint}>Ctrl+Enter {t.save} / Esc {t.cancel}</span>
              <button style={styles.cancelBtn} onClick={handleCancel} disabled={saving}>
                {t.cancel}
              </button>
              <button
                style={{ ...styles.saveBtn, ...(saving ? styles.saveBtnDisabled : {}) }}
                onClick={handleSave}
                disabled={saving}
              >
                {saving ? '...' : t.save}
              </button>
            </div>
          </div>
        ) : (
          <div className="entry-preview" style={styles.preview}>
            {editError && <div style={styles.editError}>{editError}</div>}
            {imageExportError && <div style={styles.editError}>{imageExportError}</div>}
            {isImage ? (
              <div ref={imageContainerRef} style={styles.imageContainer}>
                {imageContent ? (
                  <img
                    src={`data:image/png;base64,${imageContent}`}
                    alt="Clipboard image"
                    style={styles.imagePreview}
                  />
                ) : (
                  <div style={styles.imagePlaceholder} aria-label={t.loading}>
                    <ImageIcon size={22} strokeWidth={1.6} />
                    {imageLoading && <span>{t.loading}</span>}
                  </div>
                )}
              </div>
            ) : rawPreview ? (
              <pre style={styles.rawPreview}>{entry.content}</pre>
            ) : entry.category === 'code' ? (
              <pre style={styles.codePreview}>{entry.preview}</pre>
            ) : (
              <p style={styles.textPreview}>{entry.preview}</p>
            )}
          </div>
        )}

        {/* Original content and edit time share one compact metadata row. */}
        {hasOriginal && !editing && !isArchive && (
          <>
            <div style={styles.originalMetaRow}>
              <div
                style={styles.originalSection}
                onClick={(e) => {
                  e.stopPropagation();
                  onCopy(entry.id, true);
                }}
              >
                <button style={styles.originalToggle} onClick={handleToggleOriginal}>
                  <span style={{
                    ...styles.toggleArrow,
                    transform: showOriginal ? 'rotate(90deg)' : 'rotate(0deg)',
                  }}>
                    {'\u25B6'}
                  </span>
                  {showOriginal ? t.hideOriginal : t.showOriginal}
                </button>
              </div>
              {hasEditMetadata && (
                <span style={styles.editedBadge}>
                  {t.editedAt(formatRelativeTime(entry.updated_at!, t))}
                </span>
              )}
            </div>
            {showOriginal && (
              <div
                style={{
                  ...styles.originalContent,
                  ...(showCategoryIndicator ? {} : styles.originalContentNoIndicator),
                }}
                onClick={(e) => {
                  e.stopPropagation();
                  onCopy(entry.id, true);
                }}
              >
                <pre style={styles.originalPre}>{originalContent}</pre>
              </div>
            )}
          </>
        )}
      </div>
      {imagePreviewOpen && (
        <ImagePreviewDialog
          content={imageContent}
          loading={imageLoading || imagePreviewLoading}
          onClose={() => setImagePreviewOpen(false)}
        />
      )}
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: 'flex',
    cursor: 'pointer',
    position: 'relative',
  },
  categoryBar: {
    width: '3px',
    flexShrink: 0,
    borderRadius: '0 2px 2px 0',
  },
  body: {
    flex: 1,
    padding: '10px 12px',
    minWidth: 0,
    position: 'relative',
  },
  header: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: '6px',
    gap: '8px',
  },
  headerLeft: {
    display: 'flex',
    alignItems: 'center',
    gap: '6px',
    minWidth: 0,
    flexShrink: 1,
  },
  headerRight: {
    display: 'flex',
    alignItems: 'center',
    gap: '6px',
    flexShrink: 0,
  },
  categoryBadge: {
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    minHeight: '18px',
    fontSize: '10px',
    fontWeight: 600,
    padding: '0 8px',
    borderRadius: '10px',
    textTransform: 'uppercase',
    letterSpacing: '0.5px',
    lineHeight: 1,
  },
  categoryBadges: {
    display: 'flex',
    alignItems: 'center',
    gap: '4px',
    flexWrap: 'wrap',
    minWidth: 0,
  },
  time: {
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
    color: 'var(--accent)',
    background: 'var(--accent-bg, rgba(59,130,246,0.1))',
    padding: '0 6px',
    borderRadius: '8px',
    fontWeight: 500,
    lineHeight: 1,
  },
  originalMetaRow: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: '8px',
    marginTop: '6px',
  },
  pinBadge: {
    fontSize: '12px',
  },
  archiveTimer: {
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    minHeight: '18px',
    fontSize: '10px',
    padding: '0 6px',
    borderRadius: '8px',
    fontWeight: 500,
    lineHeight: 1,
  },
  archiveTimerwarning: {
    color: '#f59e0b',
    background: 'rgba(245,158,11,0.1)',
  },
  archiveTimerdanger: {
    color: '#ef4444',
    background: 'rgba(239,68,68,0.1)',
  },
  archiveBtn: {
    color: '#f59e0b',
  },
  preview: {
    overflow: 'hidden',
  },
  editError: {
    marginBottom: '6px',
    color: 'var(--danger)',
    fontSize: '11px',
    lineHeight: 1.4,
  },
  textPreview: {
    margin: 0,
    fontSize: '13px',
    lineHeight: 1.4,
    color: 'var(--text-primary)',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    display: '-webkit-box',
    WebkitLineClamp: 3,
    WebkitBoxOrient: 'vertical',
    wordBreak: 'break-all',
  },
  codePreview: {
    margin: 0,
    fontSize: '12px',
    lineHeight: 1.4,
    color: 'var(--text-primary)',
    fontFamily: '"Cascadia Code", "Fira Code", "Consolas", monospace',
    background: 'var(--preview-bg)',
    padding: '6px 8px',
    borderRadius: '4px',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    display: '-webkit-box',
    WebkitLineClamp: 3,
    WebkitBoxOrient: 'vertical',
    whiteSpace: 'pre-wrap',
  },
  rawPreview: {
    margin: 0,
    fontSize: '12px',
    lineHeight: 1.4,
    color: 'var(--text-primary)',
    fontFamily: '"Cascadia Code", "Fira Code", "Consolas", monospace',
    background: 'var(--preview-bg)',
    padding: '6px 8px',
    borderRadius: '4px',
    whiteSpace: 'pre-wrap',
    wordBreak: 'break-all',
    maxHeight: '300px',
    overflowY: 'auto',
  },
  imagePreview: {
    maxWidth: '100%',
    maxHeight: '120px',
    borderRadius: '4px',
    objectFit: 'contain',
  },
  imageContainer: {
    minHeight: '72px',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'flex-start',
  },
  imagePlaceholder: {
    minWidth: '88px',
    minHeight: '72px',
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    justifyContent: 'center',
    gap: '4px',
    color: 'var(--text-muted)',
    fontSize: '10px',
  },
  // Edit mode styles
  editContainer: {
    display: 'flex',
    flexDirection: 'column',
    gap: '6px',
  },
  textarea: {
    width: '100%',
    boxSizing: 'border-box',
    fontSize: '13px',
    lineHeight: 1.5,
    fontFamily: '"Cascadia Code", "Fira Code", "Consolas", monospace',
    color: 'var(--text-primary)',
    background: 'var(--preview-bg, #f5f5f5)',
    border: '1px solid var(--accent, #3b82f6)',
    borderRadius: '4px',
    padding: '8px',
    resize: 'vertical' as const,
    outline: 'none',
  },
  editActions: {
    display: 'flex',
    alignItems: 'center',
    gap: '8px',
    justifyContent: 'flex-end',
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
    background: 'var(--accent, #3b82f6)',
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
  // Original content collapsible styles
  originalSection: {
    flexShrink: 0,
  },
  originalToggle: {
    display: 'flex',
    alignItems: 'center',
    gap: '4px',
    background: 'none',
    border: 'none',
    color: 'var(--text-muted)',
    fontSize: '11px',
    cursor: 'pointer',
    padding: '2px 0',
  },
  toggleArrow: {
    fontSize: '8px',
    display: 'inline-block',
    transition: 'transform 0.2s ease',
  },
  originalContent: {
    marginTop: '4px',
    background: 'var(--preview-bg, #f5f5f5)',
    borderRadius: '4px',
    padding: '6px 8px',
    borderLeft: '3px solid var(--text-muted)',
  },
  originalContentNoIndicator: {
    borderLeft: 'none',
  },
  originalPre: {
    margin: 0,
    fontSize: '11px',
    lineHeight: 1.4,
    color: 'var(--text-secondary)',
    fontFamily: '"Cascadia Code", "Fira Code", "Consolas", monospace',
    whiteSpace: 'pre-wrap',
    wordBreak: 'break-all',
    maxHeight: '200px',
    overflowY: 'auto',
  },
  // Action buttons
  inlineActions: {
    display: 'flex',
    gap: '4px',
    flexShrink: 0,
  },
  actionBtn: {
    width: '22px',
    height: '22px',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    border: 'none',
    borderRadius: '4px',
    background: 'var(--surface)',
    color: 'var(--text-secondary)',
    fontSize: '11px',
    cursor: 'pointer',
    transition: 'background 0.12s',
  },
  deleteBtn: {
    color: '#ef4444',
  },
  actionBtnActive: {
    color: 'var(--accent)',
  },
  actionBtnDisabled: {
    opacity: 0.55,
    cursor: 'wait',
  },
};
