import { useEffect } from 'react';
import { createPortal } from 'react-dom';
import { Image as ImageIcon, X } from 'lucide-react';
import { useI18n } from '../i18n';

interface Props {
  content: string;
  loading: boolean;
  onClose: () => void;
}

export default function ImagePreviewDialog({ content, loading, onClose }: Props) {
  const { t } = useI18n();

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  const dialog = (
    <div className="dialog-backdrop image-preview-backdrop" onMouseDown={onClose}>
      <div
        className="image-preview-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={t.previewImage}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="image-preview-header">
          <span>{t.previewImage}</span>
          <button className="image-preview-close" onClick={onClose} title={t.closePreview} aria-label={t.closePreview}>
            <X size={16} />
          </button>
        </div>
        <div className="image-preview-content">
          {content ? (
            <img src={`data:image/png;base64,${content}`} alt={t.previewImage} />
          ) : (
            <div className="image-preview-loading">
              <ImageIcon size={26} strokeWidth={1.6} />
              {loading && <span>{t.loading}</span>}
            </div>
          )}
        </div>
      </div>
    </div>
  );

  const portalTarget = document.querySelector<HTMLElement>('.app-root');
  return portalTarget ? createPortal(dialog, portalTarget) : dialog;
}
