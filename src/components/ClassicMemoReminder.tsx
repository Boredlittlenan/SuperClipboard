import { useState } from 'react';
import { useI18n } from '../i18n';
import { LoadState } from '../workspace/controls';
import { useReminders } from '../workspace/hooks';
import { ReminderForm } from '../workspace/Reminders';
import '../workspace/workspace.css';

export default function ClassicMemoReminder({ memoId, revision, theme }: {
  memoId: number; revision: number; theme: 'light' | 'dark';
}) {
  const { t } = useI18n();
  const reminders = useReminders(revision);
  const [error, setError] = useState<string>();
  return <section className="classic-feature" data-theme={theme} aria-label={t.memoReminder}
    style={{ marginTop: 8 }} onClick={(event) => event.stopPropagation()}>
    {error && <p role="alert" className="inline-error">{error}</p>}
    {reminders.data ? <ReminderForm memoId={memoId}
      reminder={reminders.data.find((reminder) => reminder.memoId === memoId)}
      onChange={() => { setError(undefined); reminders.refresh(); }} onError={setError}/>
      : <LoadState error={reminders.error} retry={reminders.refresh}/>}
  </section>;
}
