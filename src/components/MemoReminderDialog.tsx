import { useEffect, useId, useRef, useState } from 'react';
import type { FormEvent, ReactNode } from 'react';
import { Bell, X } from 'lucide-react';
import { useI18n } from '../i18n';
import type { Memo } from '../types';
import type { MemoReminder } from '../api/memoReminders';
import { renderMemoBody } from './MemoBody';
import { formatReminderTime } from '../memoReminderTime';

function localDateTime(epoch: number): string {
  const date = new Date(epoch);
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

interface DialogProps {
  title: string;
  children: ReactNode;
  onClose: () => void;
  busy?: boolean;
}

function ReminderDialog({ title, children, onClose, busy }: DialogProps) {
  const { t } = useI18n();
  const dialog = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  useEffect(() => {
    const element = dialog.current;
    element?.showModal();
    return () => element?.close();
  }, []);
  return (
    <dialog
      ref={dialog}
      className="confirm-dialog reminder-dialog"
      aria-labelledby={titleId}
      onCancel={(event) => {
        event.preventDefault();
        if (!busy) onClose();
      }}
    >
      <header className="reminder-dialog-header">
        <h2 id={titleId}>{title}</h2>
        <button className="reminder-icon-button" type="button" title={t.memoReminderClose} aria-label={t.memoReminderClose} disabled={busy} onClick={onClose}>
          <X size={16} />
        </button>
      </header>
      {children}
    </dialog>
  );
}

interface EditorProps {
  initialDueAt?: number;
  memoTitle: string;
  saveFailed?: boolean;
  onClose: () => void;
  onSave: (dueAt: number) => void | Promise<void>;
  onRemove?: () => void | Promise<void>;
}

export function MemoReminderEditor({ initialDueAt, memoTitle, saveFailed, onClose, onSave, onRemove }: EditorProps) {
  const { t } = useI18n();
  const [value, setValue] = useState(() => localDateTime(initialDueAt && initialDueAt > Date.now() ? initialDueAt : Date.now() + 3_600_000));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(saveFailed ? t.memoReminderPartiallySaved : '');
  const dateId = useId();

  const choose = (minutes: number) => {
    const now = new Date();
    const time = minutes
      ? now.getTime() + minutes * 60_000
      : new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1, 9).getTime();
    setValue(localDateTime(time));
    setError('');
  };

  const submit = async (action: () => void | Promise<void>) => {
    setBusy(true);
    setError('');
    try {
      await action();
      onClose();
    } catch (error) {
      console.error('Memo reminder change failed:', error);
      setError(t.memoReminderSaveError);
    } finally {
      setBusy(false);
    }
  };

  const save = (event: FormEvent) => {
    event.preventDefault();
    const time = new Date(value).getTime();
    if (!Number.isFinite(time) || time <= Date.now() || localDateTime(time) !== value) {
      setError(t.memoReminderFutureTime);
      return;
    }
    void submit(() => onSave(time));
  };

  return (
    <ReminderDialog title={t.memoReminderSet} onClose={onClose} busy={busy}>
      <form className="reminder-editor" onSubmit={save}>
        {memoTitle && <p className="reminder-memo-label">{memoTitle}</p>}
        <div className="reminder-actions">
          <button className="dialog-btn" type="button" disabled={busy} onClick={() => choose(10)}>{t.memoReminderIn10}</button>
          <button className="dialog-btn" type="button" disabled={busy} onClick={() => choose(60)}>{t.memoReminderInHour}</button>
          <button className="dialog-btn" type="button" disabled={busy} onClick={() => choose(0)}>{t.memoReminderTomorrow}</button>
        </div>
        <label htmlFor={dateId}>{t.memoReminderDate}</label>
        <input
          id={dateId}
          type="datetime-local"
          value={value}
          min={localDateTime(Date.now())}
          max="9999-12-31T23:59"
          required
          disabled={busy}
          onChange={(event) => { setValue(event.target.value); setError(''); }}
        />
        <p className="reminder-help">{t.memoReminderDevice}</p>
        {error && <p className="reminder-error" role="alert">{error}</p>}
        <div className="reminder-actions reminder-footer">
          {onRemove && <button className="dialog-btn" type="button" onClick={() => { void submit(onRemove); }} disabled={busy}>{t.memoReminderRemove}</button>}
          <button className="dialog-btn dialog-btn-primary" type="submit" disabled={busy}>{t.save}</button>
        </div>
      </form>
    </ReminderDialog>
  );
}

interface OverviewProps {
  items: MemoReminder[];
  loading: boolean;
  failed: boolean;
  missing?: boolean;
  onClose: () => void;
  onOpen: (token: string) => void;
  onChange: (token: string, snooze?: boolean) => Promise<void>;
}

export function MemoReminderOverview({ items, loading, failed, missing, onClose, onOpen, onChange }: OverviewProps) {
  const { locale, t } = useI18n();
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState(false);

  const change = async (token: string, snooze = false) => {
    setBusy(token);
    setError(false);
    try {
      await onChange(token, snooze);
    } catch (error) {
      console.error('Memo reminder action failed:', error);
      setError(true);
    } finally {
      setBusy(null);
    }
  };

  const renderGroup = (due: boolean, label: string) => {
    const group = items.filter((item) => (item.triggeredAt !== null) === due);
    return (
      <section className="reminder-group">
        <h3>{label}</h3>
        {group.length === 0 && <p className="reminder-help">{due ? t.memoReminderNoneDue : t.memoReminderNoneScheduled}</p>}
        {group.map((item) => (
          <div className="reminder-row" key={item.token}>
            <button className="reminder-memo-link" onClick={() => onOpen(item.token)} disabled={busy !== null}>
              <Bell size={15} />
              <span>
                <strong>{item.title.trim() || t.memoUntitled}</strong>
                <time dateTime={new Date(item.dueAt).toISOString()}>{formatReminderTime(item.dueAt, locale, true)}</time>
              </span>
            </button>
            {due && (
              <div className="reminder-actions">
                <button className="dialog-btn" disabled={busy !== null} onClick={() => { void change(item.token, true); }}>{t.memoReminderSnooze}</button>
                <button className="dialog-btn" disabled={busy !== null} onClick={() => { void change(item.token); }}>{t.memoReminderDone}</button>
              </div>
            )}
          </div>
        ))}
      </section>
    );
  };

  return (
    <ReminderDialog title={t.memoReminderList} onClose={onClose} busy={busy !== null}>
      {missing && <p className="reminder-help" role="status">{t.memoReminderUnavailable}</p>}
      {(failed || error) && <p className="reminder-error" role="alert">{failed ? t.memoReminderLoadError : t.memoReminderSaveError}</p>}
      {loading ? <p role="status">{t.loading}</p> : (
        <>
          {renderGroup(true, t.memoReminderDue)}
          {renderGroup(false, t.memoReminderUpcoming)}
        </>
      )}
    </ReminderDialog>
  );
}

interface DetailProps {
  memo: Memo | null;
  reminder: MemoReminder | undefined;
  failed: boolean;
  onClose: () => void;
  onEdit: (memo: Memo, reminder: MemoReminder) => void;
  onChange: (token: string, snooze?: boolean) => Promise<void>;
}

export function MemoReminderDetail({ memo, reminder, failed, onClose, onEdit, onChange }: DetailProps) {
  const { t, locale } = useI18n();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const change = async (snooze = false) => {
    if (!reminder) return;
    setBusy(true);
    setError(false);
    try {
      await onChange(reminder.token, snooze);
      onClose();
    } catch (error) {
      console.error('Memo reminder action failed:', error);
      setError(true);
    } finally {
      setBusy(false);
    }
  };

  return (
    <ReminderDialog title={memo?.title.trim() || (memo ? t.memoUntitled : t.memoReminderList)} onClose={onClose} busy={busy}>
      {failed && <p className="reminder-error" role="alert">{t.memoReminderLoadError}</p>}
      {!memo && !failed && <p role="status">{t.loading}</p>}
      {memo && (
        <>
          {reminder && <p className="reminder-help">{formatReminderTime(reminder.dueAt, locale)}</p>}
          <div className="reminder-detail-body">{renderMemoBody(memo.body, Infinity, 260)}</div>
          {error && <p className="reminder-error" role="alert">{t.memoReminderSaveError}</p>}
          {reminder && (
            <div className="reminder-actions reminder-footer">
              {reminder.triggeredAt === null ? (
                <button className="dialog-btn" disabled={busy} onClick={() => onEdit(memo, reminder)}>{t.memoReminderSet}</button>
              ) : (
                <>
                  <button className="dialog-btn" disabled={busy} onClick={() => { void change(true); }}>{t.memoReminderSnooze}</button>
                  <button className="dialog-btn dialog-btn-primary" disabled={busy} onClick={() => { void change(); }}>{t.memoReminderDone}</button>
                </>
              )}
            </div>
          )}
        </>
      )}
    </ReminderDialog>
  );
}
