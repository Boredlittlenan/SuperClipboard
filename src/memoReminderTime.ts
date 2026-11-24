export function formatReminderTime(dueAt: number, locale: string, relative = false): string {
  const date = new Date(dueAt);
  const now = new Date();
  const tomorrow = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
  if (relative && (date.toDateString() === now.toDateString() || date.toDateString() === tomorrow.toDateString())) {
    const day = date.toDateString() === now.toDateString() ? 0 : 1;
    const dayLabel = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' }).format(day, 'day');
    const time = new Intl.DateTimeFormat(locale, { timeStyle: 'short' }).format(date);
    return `${dayLabel} ${time}`;
  }
  return new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' }).format(date);
}
