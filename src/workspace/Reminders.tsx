import { Bell, CalendarClock, Check, X } from 'lucide-react';
import { useState } from 'react';
import { finishReminder, saveReminder, type Reminder, type Repeat } from './bridge';
import { EmptyState, LoadState, Select } from './controls';
import { useLabels, useOperation, useReminders } from './hooks';
import { formatDate } from './model';

export function ReminderForm({ memoId, reminder, onChange, onError }: {
  memoId: number;
  reminder?: Reminder;
  onChange: () => void;
  onError: (text: string) => void;
}) {
  const { t, locale } = useLabels();
  const [editing, setEditing] = useState(false);
  const [date, setDate] = useState('');
  const [repeat, setRepeat] = useState<Repeat>('once');
  const { run, busy } = useOperation(onError);
  function edit() {
    const due = reminder && new Date(reminder.nextDue * 1000);
    const local = due && new Date(due.getTime() - due.getTimezoneOffset() * 60000);
    setDate(local && due.getTime() > Date.now() ? local.toISOString().slice(0, 16) : '');
    setRepeat(reminder?.repeat ?? 'once');
    setEditing(true);
  }
  const finish = (completed: boolean) => run(async () => {
    await finishReminder(memoId, completed);
    onChange();
  });
  return <section className="reminder-panel">
    <div className="section-label"><Bell size={16}/>{t('提醒', 'Reminder')}</div>
    {reminder && <p className="muted">
      {reminder.status === 'active' ? formatDate(reminder.nextDue, locale, true)
        : reminder.status === 'fired' ? t('已通知 · 待完成', 'Delivered · To do') : t('已完成', 'Completed')}
    </p>}
    {reminder?.error && <p className="inline-error">{reminder.error}</p>}
    {editing ? <form className="reminder-form" onSubmit={(event) => {
      event.preventDefault();
      void run(async () => {
        await saveReminder(memoId, date, repeat);
        setEditing(false);
        onChange();
      });
    }}>
      <label>{t('日期和时间', 'Date and time')}<input required type="datetime-local" value={date} onChange={(event) => setDate(event.target.value)}/></label>
      <Select label={t('重复', 'Repeat')} value={repeat} onChange={(value) => setRepeat(value as Repeat)}>
        <option value="once">{t('仅一次', 'Once')}</option>
        <option value="daily">{t('每天', 'Daily')}</option>
        <option value="weekly">{t('每周', 'Weekly')}</option>
      </Select>
      <div className="button-row">
        <button className="button primary" disabled={busy}>{t('保存提醒', 'Save reminder')}</button>
        <button type="button" className="button" disabled={busy} onClick={() => setEditing(false)}>{t('取消编辑', 'Cancel edit')}</button>
      </div>
    </form> : <div className="button-row">
      <button className="button" onClick={edit} disabled={busy}>{reminder ? t('修改提醒', 'Edit reminder') : t('设置提醒', 'Set reminder')}</button>
      {reminder && reminder.status !== 'completed' && <>
        <button className="button" onClick={() => void finish(true)} disabled={busy}>{t('完成', 'Complete')}</button>
        <button className="button quiet" onClick={() => void finish(false)} disabled={busy}>{t('取消提醒', 'Cancel reminder')}</button>
      </>}
    </div>}
  </section>;
}

export function RemindersView({ workspace, openMemo, onError }: {
  workspace: number; openMemo: (id: number) => void; onError: (text: string) => void;
}) {
  const { t, locale } = useLabels();
  const reminders = useReminders(workspace);
  const { run, busy } = useOperation(onError);
  if (!reminders.data) return <LoadState error={reminders.error} retry={reminders.refresh}/>;
  if (!reminders.data.length) return <EmptyState icon={<Bell size={25}/>} title={t('没有提醒', 'No reminders')}
    message={t('打开一条备忘录，为它安排提醒。', 'Open a memo to schedule a reminder.')}/>;
  return <div className="settings-scroll reminder-list">
    {reminders.data.map((reminder) => {
      const close = (completed: boolean) => run(async () => {
        await finishReminder(reminder.memoId, completed);
        reminders.refresh();
      });
      return <div className="reminder-row" key={reminder.memoId}>
        <CalendarClock size={19}/>
        <div className="grow">
          <button className="text-button" onClick={() => openMemo(reminder.memoId)}>{reminder.title}</button>
          <div className="muted">{reminder.status === 'active' ? formatDate(reminder.nextDue, locale, true)
            : reminder.status === 'fired' ? t('已通知 · 待完成', 'Delivered · To do') : t('已完成', 'Completed')}</div>
          {reminder.error && <div className="inline-error">{reminder.error}</div>}
        </div>
        {reminder.status !== 'completed' && <button className="icon-button" title={t('完成', 'Complete')}
          aria-label={t('完成', 'Complete')} disabled={busy} onClick={() => void close(true)}><Check size={17}/></button>}
        <button className="icon-button" title={t('取消提醒', 'Cancel reminder')}
          aria-label={t('取消提醒', 'Cancel reminder')} disabled={busy} onClick={() => void close(false)}><X size={17}/></button>
      </div>;
    })}
  </div>;
}
