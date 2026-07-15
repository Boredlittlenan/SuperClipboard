import { describe, expect, it } from 'vitest';
import type { Memo } from './types';
import { orderMemosForDisplay } from './memoListOrdering';

function memo(id: number, pinned = false): Memo {
  return {
    id,
    title: '',
    body: '',
    tags: '',
    auto_tags: [],
    pinned,
    sort_order: id,
    created_at: '',
    updated_at: '',
    version: 1,
  };
}

describe('orderMemosForDisplay', () => {
  it('places the active new draft above pinned entries', () => {
    const result = orderMemosForDisplay([memo(3), memo(2, true), memo(1, true)], 3);
    expect(result.map((item) => item.id)).toEqual([3, 2, 1]);
  });

  it('restores pinned-first ordering after the draft closes', () => {
    const result = orderMemosForDisplay([memo(3), memo(2, true), memo(1, true)], null);
    expect(result.map((item) => item.id)).toEqual([2, 1, 3]);
  });
});
