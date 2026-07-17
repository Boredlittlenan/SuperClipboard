import { describe, expect, it } from 'vitest';
import { isImageOnlyMemo } from './MemoBody';

describe('isImageOnlyMemo', () => {
  it('accepts image-only memo bodies with editor whitespace', () => {
    expect(isImageOnlyMemo('\n![image](data:image/png;base64,abc)\n')).toBe(true);
  });

  it('keeps automatic code tags available when readable text is present', () => {
    expect(isImageOnlyMemo('const value = 1;\n![image](data:image/png;base64,abc)')).toBe(false);
  });
});
