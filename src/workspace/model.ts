import type { Category, ClipboardEntry, Memo } from '../types';

export type View = 'history' | 'memos' | 'favorites' | 'reminders' | 'trash' | 'storage' | 'settings';
export type RecordKind = 'clipboard' | 'memo';
export type RecordView = 'history' | 'memos' | 'favorites' | 'trash';
export type Item = { kind: 'clipboard'; data: ClipboardEntry } | { kind: 'memo'; data: Memo };
export type Translate = (zh: string, en: string) => string;

export const categories: Category[] = ['text', 'link', 'image', 'code', 'email', 'file_path'];

export function categoryName(category: Category, t: Translate) {
  return {
    text: t('文本', 'Text'), link: t('链接', 'Links'), image: t('图片', 'Images'),
    code: t('代码', 'Code'), email: t('邮箱', 'Email'), file_path: t('路径', 'Paths'),
  }[category] ?? category;
}

export function itemTitle(item: Item, t: Translate) {
  return item.kind === 'memo'
    ? item.data.title.trim() || t('无标题备忘录', 'Untitled memo')
    : item.data.preview.trim() || categoryName(item.data.category, t);
}

export function formatDate(value: string | number, locale: string, withTime = false) {
  const date = new Date(typeof value === 'number' ? value * 1000 : value);
  if (Number.isNaN(date.getTime())) return String(value);
  return new Intl.DateTimeFormat(locale, withTime
    ? { dateStyle: 'medium', timeStyle: 'short' }
    : { month: 'short', day: 'numeric' }).format(date);
}

export function describeError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
