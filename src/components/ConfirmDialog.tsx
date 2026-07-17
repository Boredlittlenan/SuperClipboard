import { createPortal } from 'react-dom';
import { useI18n } from '../i18n';

export interface ConfirmDialogState {
  title: string;
  message: string;
  confirmLabel: string;
  tone?: 'danger' | 'normal';
  secondaryLabel?: string;
  secondaryTone?: 'danger' | 'normal';
  onSecondary?: () => void;
  secondaryAfterPrimary?: boolean;
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

  const primaryAction = (
    <button
      className={`dialog-btn dialog-btn-primary ${dialog.tone === 'danger' ? 'dialog-btn-danger' : ''}`}
      onClick={() => resolve(true)}
    >
      {dialog.confirmLabel}
    </button>
  );
  const secondaryAction = dialog.secondaryLabel && dialog.onSecondary ? (
    <button
      className={`dialog-btn dialog-btn-secondary ${dialog.secondaryTone === 'danger' ? 'dialog-btn-danger' : ''}`}
      onClick={() => {
        dialog.onSecondary?.();
        onClose();
      }}
    >
      {dialog.secondaryLabel}
    </button>
  ) : null;

  const content = (
    <div className="dialog-backdrop" onMouseDown={() => resolve(false)}>
      <div className="confirm-dialog" onMouseDown={(e) => e.stopPropagation()}>
        <div className="confirm-dialog-title">{dialog.title}</div>
        <div className="confirm-dialog-message">{dialog.message}</div>
        <div className={`confirm-dialog-actions${dialog.secondaryLabel ? ' has-secondary' : ''}`}>
          <button className="dialog-btn" onClick={() => resolve(false)}>
            {t.cancel}
          </button>
          {dialog.secondaryAfterPrimary ? primaryAction : secondaryAction}
          {dialog.secondaryAfterPrimary ? secondaryAction : primaryAction}
        </div>
      </div>
    </div>
  );

  const portalTarget = document.querySelector<HTMLElement>('.app-root');
  return portalTarget ? createPortal(content, portalTarget) : content;
}
