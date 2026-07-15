import { describe, expect, it } from 'vitest';
import { manualMemoTags, visibleMemoTags, type MemoTagLabels } from './memoTagPresentation';

const zhLabels: MemoTagLabels = {
  image: '图片',
  email: '邮箱',
  path: '路径',
  link: '链接',
  code: '代码',
};

describe('memo tag presentation', () => {
  it('removes generated labels in either language from manual tags', () => {
    expect(manualMemoTags('工作,EMAIL,邮箱,Link,链接,CODE,代码,work')).toBe('工作,work');
  });

  it('renders each canonical auto tag once in the current language', () => {
    expect(visibleMemoTags('项目,EMAIL,邮箱,LINK', ['email', 'link', 'email'], zhLabels)).toEqual([
      { label: '项目', type: null },
      { label: '邮箱', type: 'email' },
      { label: '链接', type: 'link' },
    ]);
  });
});
