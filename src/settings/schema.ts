import type { ThemeMode } from '../types';

const parseBoolean = (defaultValue: boolean) => (value: string | undefined): boolean =>
  value == null ? defaultValue : value === 'true';

const serializeBoolean = (value: boolean): string => String(value);

const parseThemeMode = (value: string | undefined): ThemeMode =>
  value === 'light' || value === 'dark' || value === 'system' ? value : 'system';

const parseThemeAccent = (value: string | undefined): 'default' | 'sakura' =>
  value === 'sakura' ? 'sakura' : 'default';

export const APP_SETTING_SCHEMA = {
  memoEnabled: {
    key: 'memo_enabled',
    parse: parseBoolean(false),
    serialize: serializeBoolean,
  },
  memoColor: {
    key: 'memo_color',
    parse: (value: string | undefined): string | null => value || null,
    serialize: (value: string | null): string => value ?? '',
  },
  rawPreview: {
    key: 'raw_preview',
    parse: parseBoolean(false),
    serialize: serializeBoolean,
  },
  archiveEnabled: {
    key: 'archive_enabled',
    parse: parseBoolean(false),
    serialize: serializeBoolean,
  },
  experimentalFeaturesEnabled: {
    key: 'experimental_features_enabled',
    parse: parseBoolean(false),
    serialize: serializeBoolean,
  },
  clipboardMultiTagEnabled: {
    key: 'clipboard_multi_tag_enabled',
    parse: parseBoolean(false),
    serialize: serializeBoolean,
  },
  hideEntryColorStripEnabled: {
    key: 'hide_entry_color_strip_enabled',
    parse: parseBoolean(false),
    serialize: serializeBoolean,
  },
  categoryTabSelectedColorsEnabled: {
    key: 'category_tab_selected_colors_enabled',
    parse: parseBoolean(false),
    serialize: serializeBoolean,
  },
  categoryTabSortingEnabled: {
    key: 'category_tab_sorting_enabled',
    parse: parseBoolean(true),
    serialize: serializeBoolean,
  },
  modernUiEnabled: {
    key: 'modern_ui_enabled',
    parse: parseBoolean(false),
    serialize: serializeBoolean,
  },
  themeAccent: {
    key: 'theme_accent',
    parse: parseThemeAccent,
    serialize: (value: 'default' | 'sakura'): string => value,
  },
  themeMode: {
    key: 'theme_mode',
    parse: parseThemeMode,
    serialize: (value: ThemeMode): string => value,
  },
  autoUpdate: {
    key: 'auto_update',
    parse: parseBoolean(true),
    serialize: serializeBoolean,
  },
  alwaysOnTop: {
    key: 'always_on_top',
    parse: parseBoolean(false),
    serialize: serializeBoolean,
  },
} as const;

export type AppSettingName = keyof typeof APP_SETTING_SCHEMA;
export type AppSettings = {
  [K in AppSettingName]: ReturnType<(typeof APP_SETTING_SCHEMA)[K]['parse']>;
};

export const APP_SETTING_KEYS = Object.values(APP_SETTING_SCHEMA).map(({ key }) => key);

export function parseAppSettings(values: Record<string, string>): AppSettings {
  return Object.fromEntries(
    Object.entries(APP_SETTING_SCHEMA).map(([name, definition]) => [
      name,
      definition.parse(values[definition.key]),
    ]),
  ) as AppSettings;
}

export function serializeAppSetting<K extends AppSettingName>(
  name: K,
  value: AppSettings[K],
): { key: string; value: string } {
  const definition = APP_SETTING_SCHEMA[name];
  return {
    key: definition.key,
    value: definition.serialize(value as never),
  };
}
