import { useI18n } from '../i18n';

export interface ConfirmDialogState {
  title: string;
  message: string;
  confirmLabel: string;
  tone?: 'danger' | 'normal';
  resolve: (confirmed: boolean) => void;
}

interface Props {
  dialog: ConfirmDialogState;
  onClose: () => void;
}

export default function ConfirmDialog({ dialog, onClose }: Props) {
  const { t } = useI18n();

  const resolve = (confirmed: boolean) => {
    dialog.resolve(confirmed);
    onClose();
  };

  return (
    <div className="dialog-backdrop" onMouseDown={() => resolve(false)}>
      <div className="confirm-dialog" onMouseDown={(e) => e.stopPropagation()}>
        <div className="confirm-dialog-title">{dialog.title}</div>
        <div className="confirm-dialog-message">{dialog.message}</div>
        <div className="confirm-dialog-actions">
          <button className="dialog-btn" onClick={() => resolve(false)}>
            {t.cancel}
          </button>
          <button
            className={`dialog-btn dialog-btn-primary ${dialog.tone === 'danger' ? 'dialog-btn-danger' : ''}`}
            onClick={() => resolve(true)}
          >
            {dialog.confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
