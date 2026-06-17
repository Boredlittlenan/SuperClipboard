import type { Category, FilterTab } from './types';

/** Category badge color map */
const CATEGORY_COLORS: Record<Category, string> = {
  text: '#6b7280',
  link: '#3b82f6',
  image: '#8b5cf6',
  code: '#10b981',
  email: '#f59e0b',
  file_path: '#ef4444',
};

/** Human-readable category labels */
const CATEGORY_LABELS: Record<Category, string> = {
  text: 'Text',
  link: 'Link',
  image: 'Image',
  code: 'Code',
  email: 'Email',
  file_path: 'Path',
};

const TAB_LABELS: Record<FilterTab, string> = {
  all: 'All',
  text: 'Text',
  link: 'Link',
  image: 'Image',
  code: 'Code',
  email: 'Email',
  file_path: 'Path',
};

export function getCategoryColor(category: Category): string {
  return CATEGORY_COLORS[category] ?? '#6b7280';
}

export function getCategoryLabel(category: Category): string {
  return CATEGORY_LABELS[category] ?? category;
}

export function getTabLabel(tab: FilterTab): string {
  return TAB_LABELS[tab] ?? tab;
}

/** Format relative time string */
export function formatRelativeTime(isoString: string): string {
  const date = new Date(isoString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSec = Math.floor(diffMs / 1000);

  if (diffSec < 60) return 'just now';
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m ago`;
  if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`;
  if (diffSec < 604800) return `${Math.floor(diffSec / 86400)}d ago`;

  return date.toLocaleDateString();
}
