import type { MemoAutoTagType } from './api/memos';

export type MemoTagLabels = Record<MemoAutoTagType, string>;

export interface VisibleMemoTag {
  label: string;
  type: MemoAutoTagType | null;
}

const AUTO_TAG_ALIASES: Record<MemoAutoTagType, string[]> = {
  image: ['image', '图片'],
  email: ['email', '邮箱'],
  path: ['path', '路径'],
  link: ['link', '链接'],
  code: ['code', '代码'],
};

function normalizeTag(tag: string): string {
  return tag.trim().toLocaleLowerCase();
}

function splitUniqueTags(tags: string): string[] {
  const seen = new Set<string>();
  return tags
    .split(',')
    .map((tag) => tag.trim())
    .filter((tag) => {
      if (!tag) return false;
      const key = normalizeTag(tag);
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
}

function isGeneratedTagLabel(tag: string): boolean {
  const key = normalizeTag(tag);
  return Object.values(AUTO_TAG_ALIASES).some((aliases) =>
    aliases.some((alias) => normalizeTag(alias) === key),
  );
}

/** Keep only labels explicitly supplied by the user. */
export function manualMemoTags(tags: string): string {
  return splitUniqueTags(tags)
    .filter((tag) => !isGeneratedTagLabel(tag))
    .join(',');
}

/** Combine manual labels with one localized label for each canonical auto-tag type. */
export function visibleMemoTags(
  tags: string,
  autoTags: MemoAutoTagType[],
  labels: MemoTagLabels,
): VisibleMemoTag[] {
  const visible: VisibleMemoTag[] = splitUniqueTags(manualMemoTags(tags)).map((label) => ({
    label,
    type: null,
  }));
  const seenTypes = new Set<MemoAutoTagType>();

  for (const type of autoTags) {
    if (seenTypes.has(type) || !labels[type]) continue;
    seenTypes.add(type);
    visible.push({ label: labels[type], type });
  }

  return visible;
}
