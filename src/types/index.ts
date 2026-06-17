export type Category = 'text' | 'link' | 'image' | 'code' | 'email' | 'file_path';

export interface ClipboardEntry {
  id: number;
  category: Category;
  content_type: string;
  content: string;
  preview: string;
  hash: string;
  pinned: boolean;
  created_at: string;
}

export interface QueryFilter {
  category?: Category;
  search?: string;
  limit?: number;
  offset?: number;
}

export interface Stats {
  total: number;
  text: number;
  link: number;
  image: number;
  code: number;
}

export type FilterTab = 'all' | Category;
