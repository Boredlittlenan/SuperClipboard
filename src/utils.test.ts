import { afterEach, describe, expect, it, vi } from 'vitest';
import { zhCN } from './i18n/translations';
import { formatRelativeTime } from './utils';

describe('formatRelativeTime', () => {
  afterEach(() => vi.useRealTimers());

  it('treats legacy SQLite timestamps without a timezone as UTC', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-07-16T00:45:30Z'));

    expect(formatRelativeTime('2026-07-16 00:45:00', zhCN)).toBe(zhCN.justNow);
  });
});
