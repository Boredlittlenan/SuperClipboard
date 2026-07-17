export type Locale = 'zh-CN' | 'en';

export interface Translations {
  // Title bar
  appTitle: string;

  // Search
  searchPlaceholder: string;
  clearSearch: string;

  // Category tabs
  tabAll: string;
  tabText: string;
  tabLink: string;
  tabImage: string;
  tabCode: string;
  tabEmail: string;
  tabPath: string;
  tabArchive: string;

  // Clipboard items
  justNow: string;
  minutesAgo: (n: number) => string;
  hoursAgo: (n: number) => string;
  clickToCopy: string;
  pin: string;
  unpin: string;
  delete: string;
  edit: string;
  save: string;
  editConflict: string;
  cancel: string;
  openInBrowser: string;
  exportImage: string;
  exportImageFailed: string;
  previewImage: string;
  closePreview: string;
  restore: string;
  archive: string;
  permanentDelete: string;
  permanentDeleteConfirm: string;
  archiveEmpty: string;
  archiveEmptyHint: string;
  daysRemaining: (n: number) => string;
  archiveSubTab: string;
  memoSubTab: string;
  editedAt: (time: string) => string;
  originalContent: string;
  showOriginal: string;
  hideOriginal: string;
  dragToReorder: string;
  selectEntries: string;
  selectItem: string;
  deselectItem: string;
  selectedCount: (n: number) => string;
  deleteSelected: string;
  deleteSelectedTitle: string;
  deleteSelectedConfirm: (n: number, archived: boolean) => string;
  deleteSelectedDone: (n: number, archived: boolean) => string;
  deleteSelectedFailed: string;
  mergeEntries: string;
  mergeToMemo: string;
  mergeNeedTwo: string;
  mergeSameTypeOnly: string;
  mergeLimitReached: (n: number) => string;
  mergeChoiceTitle: string;
  mergeChoiceMessage: (n: number, archived: boolean) => string;
  mergeOnly: string;
  mergeAndDeleteOriginals: string;
  mergeCreated: string;
  mergeMemoCreated: string;
  mergeDuplicate: string;
  mergeCreatedAndRemoved: (archived: boolean) => string;
  mergeMemoCreatedAndRemoved: (archived: boolean) => string;
  mergeDuplicateAndRemoved: (archived: boolean) => string;
  mergeFailed: string;
  mergeMemoRequired: string;
  mergedImagesTitle: (n: number) => string;

  // List states
  loading: string;
  noEntries: string;
  noEntriesHint: string;

  // Footer
  itemsCount: (n: number) => string;
  clipboardStorage: (formatted: string) => string;
  memoStorage: (formatted: string) => string;
  clearHistory: string;
  clearConfirm: string;
  clearScopeConfirm: (category: string) => string;
  clearMovesToArchive: string;
  clearDeletesPermanently: string;
  clearCurrentTab: string;
  clearAllHistory: string;
  emptyArchive: string;
  emptyArchivePending: string;
  emptyArchiveTitle: string;
  emptyArchiveConfirm: (scope: string, count: number) => string;
  emptyArchiveSuccess: (scope: string, count: number) => string;
  emptyArchiveFailed: string;

  // Memos
  memoTab: string;
  memoSearchPlaceholder: string;
  memoNew: string;
  memoUntitled: string;
  memoTitlePlaceholder: string;
  memoBodyPlaceholder: string;
  memoTagsPlaceholder: string;
  memoEmpty: string;
  memoEmptyHint: string;
  memoShowMore: string;
  memoShowLess: string;
  memoSetting: string;
  memoSettingDesc: string;
  memoColor: string;
  memoColorDesc: string;
  memoColorReset: string;
  archiveSetting: string;
  archiveSettingDesc: string;
  savePosition: string;
  savePositionDesc: string;

  // Toast
  copied: string;
  dropImportPrompt: string;
  dropImportHint: string;
  dropImporting: string;
  dropImportDone: string;
  dropImportFailed: string;

  // Settings
  settings: string;
  language: string;
  langZhCN: string;
  langEn: string;
  themeColor: string;
  themeColorDesc: string;
  themeDefault: string;
  themeSakura: string;
  themeMode: string;
  themeModeDesc: string;
  themeSystem: string;
  themeLight: string;
  themeDark: string;
  modernUi: string;
  modernUiDesc: string;
  autostart: string;
  autostartDesc: string;
  alwaysOnTop: string;
  alwaysOnTopDesc: string;
  rawPreview: string;
  rawPreviewDesc: string;
  followMode: string;
  followModeDesc: string;
  autoUpdate: string;
  autoUpdateDesc: string;
  experimentalFeatures: string;
  experimentalFeaturesDesc: string;
  multiSelectMode: string;
  multiSelectModeDesc: string;
  clipboardMultiTag: string;
  clipboardMultiTagDesc: string;
  hideEntryColorStrip: string;
  hideEntryColorStripDesc: string;
  categoryTabSelectedColors: string;
  categoryTabSelectedColorsDesc: string;
  reclassifyHistory: string;
  reclassifyHistoryDesc: string;
  reclassifyHistoryPending: string;
  reclassifyHistoryConfirmTitle: string;
  reclassifyHistoryConfirm: string;
  reclassifyHistoryConfirmAction: string;
  reclassifyHistorySuccess: (count: number) => string;
  reclassifyHistoryFailed: string;
  classificationRulesCurrent: (version: number) => string;
  classificationRulesOutdated: (version: number) => string;
  categoryTabSorting: string;
  categoryTabSortingDesc: string;
  version: string;
  shortcut: string;
  shortcutDesc: string;
  shortcutRecording: string;
  shortcutInvalid: string;
  checkUpdate: string;
  checking: string;
  upToDate: string;
  hasUpdate: (version: string) => string;
  updateFailed: string;
  downloadUpdate: string;
  updateCurrent: (version: string) => string;
  updateLatest: (version: string) => string;
  releaseNotes: string;
  noReleaseNotes: string;
  backupRestore: string;
  backupRestoreDesc: string;
  backupCompatibilityNotice: string;
  backupVersionMeta: (backupVersion: string, currentVersion: string) => string;
  unknownVersion: string;
  createBackup: string;
  creatingBackup: string;
  backupCreated: string;
  openBackupFolder: string;
  restoreBackup: string;
  restoringBackup: string;
  noBackups: string;
  restoreBackupConfirm: string;
  restoreBackupDone: (clipboard: number, memos: number, settings: number) => string;
  backupFailed: string;
  systemSettings: string;
  appearanceSettings: string;
  featureSettings: string;
  remoteStorage: string;
  storageMode: string;
  storageModeLocal: string;
  storageModeRemote: string;
  storageStatusLocal: string;
  storageStatusConnected: string;
  storageStatusFailed: string;
  storageStatusNotReady: string;
  localModeHint: string;
  remoteModeHint: string;
  remoteStoragePending: string;
  connectionMode: string;
  connectionUrl: string;
  connectionManual: string;
  remoteProfiles: string;
  noRemoteProfiles: string;
  useRemoteProfile: string;
  deleteRemoteProfile: string;
  deleteRemoteProfileConfirm: (name: string) => string;
  lastUsedRemoteProfile: (time: string) => string;
  databaseUrl: string;
  databaseHost: string;
  databasePort: string;
  databaseName: string;
  databaseUser: string;
  databasePassword: string;
  databaseSsl: string;
  testingConnection: string;
  storageConnectionReady: string;
  storageConnectionFailed: string;
  saveAndUseLocal: string;
  saveAndUseRemote: string;
  savingStorageConfig: string;
  storageConfigSaved: string;
  storageConfigFailed: string;
}

export const zhCN: Translations = {
  appTitle: '超级剪贴板',

  searchPlaceholder: '搜索剪贴板...',
  clearSearch: '清除搜索',

  tabAll: '全部',
  tabText: '文本',
  tabLink: '链接',
  tabImage: '图片',
  tabCode: '代码',
  tabEmail: '邮箱',
  tabPath: '路径',
  tabArchive: '回收站',

  justNow: '刚刚',
  minutesAgo: (n) => `${n} 分钟前`,
  hoursAgo: (n) => `${n} 小时前`,
  clickToCopy: '点击复制',
  pin: '置顶',
  unpin: '取消置顶',
  delete: '删除',
  edit: '编辑',
  save: '保存',
  editConflict: '内容已被其他设备修改，已刷新最新内容，请重新编辑后再保存',
  cancel: '取消',
  openInBrowser: '在浏览器中打开',
  exportImage: '另存为图片',
  exportImageFailed: '图片导出失败，请重试',
  previewImage: '预览图片',
  closePreview: '关闭预览',
  restore: '恢复',
  archive: '回收站',
  permanentDelete: '彻底删除',
  permanentDeleteConfirm: '确定要彻底删除此条目吗？此操作不可恢复。',
  archiveEmpty: '回收站为空',
  archiveEmptyHint: '删除的条目会暂存在回收站，30天后自动清除',
  daysRemaining: (n) => `${n} 天后清除`,
  archiveSubTab: '剪贴板',
  memoSubTab: '备忘录',
  editedAt: (time) => `编辑于 ${time}`,
  originalContent: '原始内容',
  showOriginal: '查看原文',
  hideOriginal: '收起原文',
  dragToReorder: '拖拽排序',
  selectEntries: '选择条目',
  selectItem: '选择此条目',
  deselectItem: '取消选择',
  selectedCount: (n) => `已选 ${n} 条`,
  deleteSelected: '删除所选',
  deleteSelectedTitle: '删除所选条目',
  deleteSelectedConfirm: (n, archived) => archived
    ? `将选中的 ${n} 条记录移入回收站吗？`
    : `永久删除选中的 ${n} 条记录吗？此操作无法撤销。`,
  deleteSelectedDone: (n, archived) => archived
    ? `已将 ${n} 条记录移入回收站`
    : `已删除 ${n} 条记录`,
  deleteSelectedFailed: '删除所选条目失败，请重试',
  mergeEntries: '合并为新条目',
  mergeToMemo: '合并为图片备忘录',
  mergeNeedTwo: '至少选择两个同类型条目',
  mergeSameTypeOnly: '只能合并相同主类型的条目',
  mergeLimitReached: (n) => `一次最多合并 ${n} 条`,
  mergeChoiceTitle: '合并所选条目',
  mergeChoiceMessage: (n, archived) => archived
    ? `请选择合并方式。合并并删除后，${n} 条原记录将移入回收站。`
    : `请选择合并方式。合并并删除后，${n} 条原记录将被永久删除。`,
  mergeOnly: '仅合并',
  mergeAndDeleteOriginals: '合并并删除原条目',
  mergeCreated: '已生成新的合并条目',
  mergeMemoCreated: '已生成图片备忘录',
  mergeDuplicate: '相同的合并内容已经存在',
  mergeCreatedAndRemoved: (archived) => archived
    ? '已生成合并条目，原条目已移入回收站'
    : '已生成合并条目并删除原条目',
  mergeMemoCreatedAndRemoved: (archived) => archived
    ? '已生成图片备忘录，原条目已移入回收站'
    : '已生成图片备忘录并删除原条目',
  mergeDuplicateAndRemoved: (archived) => archived
    ? '相同的合并内容已存在，原条目已移入回收站'
    : '相同的合并内容已存在，原条目已删除',
  mergeFailed: '合并失败，请重试',
  mergeMemoRequired: '合并图片前请先在设置中开启备忘录',
  mergedImagesTitle: (n) => `合并图片 (${n})`,

  loading: '加载中...',
  noEntries: '暂无剪贴板记录',
  noEntriesHint: '复制一些内容开始使用',

  itemsCount: (n) => `${n} 条记录`,
  clipboardStorage: (s) => `剪贴板 ${s}`,
  memoStorage: (s) => `备忘录 ${s}`,
  clearHistory: '清除历史',
  clearConfirm: '确定清除所有未置顶的记录吗？',
  clearScopeConfirm: (category) => `要仅清除“${category}”标签内的未置顶记录，还是清除全部未置顶记录？`,
  clearMovesToArchive: '回收站已开启：清除的条目将进入回收站，可在回收站中恢复。',
  clearDeletesPermanently: '回收站未开启：清除的条目将被永久删除，无法恢复。',
  clearCurrentTab: '仅清除当前标签',
  clearAllHistory: '清除全部',
  emptyArchive: '清空',
  emptyArchivePending: '清空中',
  emptyArchiveTitle: '清空回收站',
  emptyArchiveConfirm: (scope, count) => `确定永久删除回收站中的 ${count} 条${scope}记录吗？删除后无法恢复。`,
  emptyArchiveSuccess: (scope, count) => `已清空 ${count} 条${scope}记录`,
  emptyArchiveFailed: '清空回收站失败，请稍后重试',

  memoTab: '备忘录',
  memoSearchPlaceholder: '搜索备忘录...',
  memoNew: '新建备忘录',
  memoUntitled: '无标题',
  memoTitlePlaceholder: '标题',
  memoBodyPlaceholder: '写点什么...',
  memoTagsPlaceholder: '添加标签，逗号分隔',
  memoEmpty: '还没有备忘录',
  memoEmptyHint: '点击新建开始记录',
  memoShowMore: '显示更多',
  memoShowLess: '收起',
  memoSetting: '备忘录',
  memoSettingDesc: '在标签栏显示备忘录入口',
  memoColor: '备忘录配色',
  memoColorDesc: '自定义备忘录模块配色，不受主题影响',
  memoColorReset: '重置',
  archiveSetting: '回收站',
  archiveSettingDesc: '删除条目时暂存回收站，30天内可恢复',
  savePosition: '保存位置',
  savePositionDesc: '记录上次窗口位置，下次启动时恢复',

  copied: '已复制！',
  dropImportPrompt: '松开以收录到剪贴板',
  dropImportHint: '支持拖入文字、图片或图片文件',
  dropImporting: '正在收录...',
  dropImportDone: '已收录到剪贴板',
  dropImportFailed: '收录失败，请重试',

  settings: '设置',
  language: '语言',
  langZhCN: '中文',
  langEn: 'English',
  themeColor: '主题配色',
  themeColorDesc: '切换界面强调色，深浅色仍跟随系统',
  themeDefault: '少年蓝',
  themeSakura: '樱花粉',
  themeMode: '主题模式',
  themeModeDesc: '自动跟随系统浅色/深色，或手动指定',
  themeSystem: '跟随系统',
  themeLight: '浅色',
  themeDark: '深色',
  modernUi: '新版界面',
  modernUiDesc: '切换新版视觉样式，关闭后恢复旧版简洁界面',
  autostart: '开机自启',
  autostartDesc: '系统启动时自动运行',
  alwaysOnTop: '窗口置顶',
  alwaysOnTopDesc: '窗口始终显示在最前面',
  rawPreview: '原格式预览',
  rawPreviewDesc: '以原始格式显示剪贴板内容',
  followMode: '跟随模式',
  followModeDesc: '快捷键唤起时窗口跟随插入符位置',
  autoUpdate: '自动检查更新',
  autoUpdateDesc: '每次启动时自动检查更新',
  experimentalFeatures: '实验功能',
  experimentalFeaturesDesc: '显示实验功能入口，实验功能默认关闭',
  multiSelectMode: '多选模式',
  multiSelectModeDesc: '显示选择条目入口，支持 Ctrl+点击多选和 Delete 删除',
  clipboardMultiTag: '剪贴板多标签显示',
  clipboardMultiTagDesc: '在剪贴板条目中显示所有识别出的分类标签',
  hideEntryColorStrip: '隐藏条目色条',
  hideEntryColorStripDesc: '隐藏剪贴板条目左侧的分类色条和原文左侧边框',
  categoryTabSelectedColors: '多色模式(Tab标签)',
  categoryTabSelectedColorsDesc: '让选中的 Tab 标签使用对应分类颜色',
  reclassifyHistory: '更新历史分类',
  reclassifyHistoryDesc: '建议在版本更新或分类规则变化后重新识别当前存储中的历史条目',
  reclassifyHistoryPending: '正在更新分类',
  reclassifyHistoryConfirmTitle: '重新识别历史条目',
  reclassifyHistoryConfirm: '将按当前版本的规则重新识别当前存储中的历史剪贴板条目。只更新分类标签和搜索索引，不修改正文、时间、置顶或编辑记录。是否继续？',
  reclassifyHistoryConfirmAction: '开始重新识别',
  reclassifyHistorySuccess: (count) => `历史分类已更新，共调整 ${count} 条记录`,
  reclassifyHistoryFailed: '历史分类更新失败，请稍后重试',
  classificationRulesCurrent: (version) => `分类规则 v${version} · 历史数据已更新`,
  classificationRulesOutdated: (version) => `分类规则 v${version} · 建议重新识别历史数据`,
  categoryTabSorting: '标签排序',
  categoryTabSortingDesc: '允许直接拖动剪贴板分类标签调整顺序',
  version: '版本号',
  shortcut: '快捷键',
  shortcutDesc: '唤起/隐藏窗口',
  shortcutRecording: '按下新的组合键...',
  shortcutInvalid: '需要至少一个修饰键',
  checkUpdate: '检查更新',
  checking: '检查中...',
  upToDate: '已是最新版本',
  hasUpdate: (v) => `发现新版本 ${v}`,
  updateFailed: '检查失败，请稍后重试',
  downloadUpdate: '前往下载',
  updateCurrent: (v) => `当前 ${v}`,
  updateLatest: (v) => `最新 ${v}`,
  releaseNotes: '更新日志',
  noReleaseNotes: '暂无更新说明',
  backupRestore: '备份/恢复',
  backupRestoreDesc: '导出或恢复本地剪贴板、备忘录和设置',
  backupCompatibilityNotice: '备份文件为新版 .scbackup 包。恢复会记录备份创建版本和当前恢复版本，旧 gzip .scbackup / .json 不再读取，跨版本恢复不保证可恢复。',
  backupVersionMeta: (backupVersion, currentVersion) => `备份版本 v${backupVersion} · 当前版本 v${currentVersion}`,
  unknownVersion: '未知',
  createBackup: '创建备份',
  creatingBackup: '备份中...',
  backupCreated: '备份已创建',
  openBackupFolder: '打开目录',
  restoreBackup: '恢复备份',
  restoringBackup: '恢复中...',
  noBackups: '暂无备份',
  restoreBackupConfirm: '恢复会覆盖当前剪贴板、备忘录和设置。确定继续吗？',
  restoreBackupDone: (clipboard, memos, settings) => `已恢复 ${clipboard} 条剪贴板、${memos} 条备忘录、${settings} 项设置`,
  backupFailed: '操作失败',
  systemSettings: '系统设置',
  appearanceSettings: '外观设置',
  featureSettings: '功能设置',
  remoteStorage: '存储设置',
  storageMode: '存储模式',
  storageModeLocal: '本地',
  storageModeRemote: '外部',
  storageStatusLocal: '本地存储',
  storageStatusConnected: '外部存储',
  storageStatusFailed: '外部存储',
  storageStatusNotReady: '外部存储',
  localModeHint: '默认模式。剪贴板和备忘录只保存在本机 SQLite 数据库中。',
  remoteModeHint: '连接外部 PostgreSQL，剪贴板和备忘录正文只写入外部数据库。',
  remoteStoragePending: '保存时会自动测试连接并初始化远端表',
  connectionMode: '连接方式',
  connectionUrl: '连接 URL',
  connectionManual: '手动填写',
  remoteProfiles: '已保存连接',
  noRemoteProfiles: '成功切换过的外部数据库会记录在这里',
  useRemoteProfile: '使用',
  deleteRemoteProfile: '删除连接',
  deleteRemoteProfileConfirm: (name) => `确定删除已保存连接“${name}”吗？这不会删除外部数据库中的数据。`,
  lastUsedRemoteProfile: (time) => `上次 ${time}`,
  databaseUrl: '数据库 URL',
  databaseHost: '地址',
  databasePort: '端口',
  databaseName: '数据库',
  databaseUser: '账号',
  databasePassword: '密码',
  databaseSsl: 'SSL',
  testingConnection: '测试中...',
  storageConnectionReady: '连接成功，远端表已就绪',
  storageConnectionFailed: '连接失败',
  saveAndUseLocal: '保存并切换到本地',
  saveAndUseRemote: '保存并切换到外部',
  savingStorageConfig: '保存中...',
  storageConfigSaved: '存储设置已保存',
  storageConfigFailed: '保存失败',
};

export const en: Translations = {
  appTitle: 'SuperClipboard',

  searchPlaceholder: 'Search clipboard...',
  clearSearch: 'Clear search',

  tabAll: 'All',
  tabText: 'Text',
  tabLink: 'Link',
  tabImage: 'Image',
  tabCode: 'Code',
  tabEmail: 'Email',
  tabPath: 'Path',
  tabArchive: 'Recycle Bin',

  justNow: 'just now',
  minutesAgo: (n) => `${n}m ago`,
  hoursAgo: (n) => `${n}h ago`,
  clickToCopy: 'Click to copy',
  pin: 'Pin',
  unpin: 'Unpin',
  delete: 'Delete',
  edit: 'Edit',
  save: 'Save',
  editConflict: 'This item changed on another device. The latest content has been refreshed; review and save again.',
  cancel: 'Cancel',
  openInBrowser: 'Open in browser',
  exportImage: 'Save image as',
  exportImageFailed: 'Could not save the image. Please try again.',
  previewImage: 'Preview image',
  closePreview: 'Close preview',
  restore: 'Restore',
  archive: 'Recycle Bin',
  permanentDelete: 'Delete Forever',
  permanentDeleteConfirm: 'Permanently delete this item? This cannot be undone.',
  archiveEmpty: 'Recycle bin is empty',
  archiveEmptyHint: 'Deleted items stay in recycle bin, auto-cleared after 30 days',
  daysRemaining: (n) => `${n} day${n !== 1 ? 's' : ''} until cleared`,
  archiveSubTab: 'Clipboard',
  memoSubTab: 'Memos',
  editedAt: (time) => `Edited ${time}`,
  originalContent: 'Original',
  showOriginal: 'Show original',
  hideOriginal: 'Hide original',
  dragToReorder: 'Drag to reorder',
  selectEntries: 'Select entries',
  selectItem: 'Select this entry',
  deselectItem: 'Deselect',
  selectedCount: (n) => `${n} selected`,
  deleteSelected: 'Delete selected',
  deleteSelectedTitle: 'Delete selected entries',
  deleteSelectedConfirm: (n, archived) => archived
    ? `Move ${n} selected item${n === 1 ? '' : 's'} to the Recycle Bin?`
    : `Permanently delete ${n} selected item${n === 1 ? '' : 's'}? This cannot be undone.`,
  deleteSelectedDone: (n, archived) => archived
    ? `Moved ${n} item${n === 1 ? '' : 's'} to the Recycle Bin`
    : `Deleted ${n} item${n === 1 ? '' : 's'}`,
  deleteSelectedFailed: 'Could not delete the selected entries',
  mergeEntries: 'Merge into new entry',
  mergeToMemo: 'Merge into image memo',
  mergeNeedTwo: 'Select at least two entries of the same type',
  mergeSameTypeOnly: 'Only entries with the same primary type can be merged',
  mergeLimitReached: (n) => `Up to ${n} entries can be merged at once`,
  mergeChoiceTitle: 'Merge selected entries',
  mergeChoiceMessage: (n, archived) => archived
    ? `Choose how to merge. Deleting the originals will move all ${n} items to the Recycle Bin.`
    : `Choose how to merge. Deleting the originals will permanently remove all ${n} items.`,
  mergeOnly: 'Merge only',
  mergeAndDeleteOriginals: 'Merge and delete originals',
  mergeCreated: 'Created a new merged entry',
  mergeMemoCreated: 'Created an image memo',
  mergeDuplicate: 'The same merged content already exists',
  mergeCreatedAndRemoved: (archived) => archived
    ? 'Created the merged entry and moved the originals to the Recycle Bin'
    : 'Created the merged entry and deleted the originals',
  mergeMemoCreatedAndRemoved: (archived) => archived
    ? 'Created the image memo and moved the originals to the Recycle Bin'
    : 'Created the image memo and deleted the originals',
  mergeDuplicateAndRemoved: (archived) => archived
    ? 'The merged content already exists; the originals were moved to the Recycle Bin'
    : 'The merged content already exists; the originals were deleted',
  mergeFailed: 'Could not merge the selected entries',
  mergeMemoRequired: 'Enable Memos in Settings before merging images',
  mergedImagesTitle: (n) => `Merged Images (${n})`,

  loading: 'Loading...',
  noEntries: 'No clipboard entries yet',
  noEntriesHint: 'Copy something to get started',

  itemsCount: (n) => `${n} item${n !== 1 ? 's' : ''}`,
  clipboardStorage: (s) => `Clipboard ${s}`,
  memoStorage: (s) => `Memo ${s}`,
  clearHistory: 'Clear History',
  clearConfirm: 'Clear all non-pinned entries?',
  clearScopeConfirm: (category) => `Clear non-pinned entries in the “${category}” tab only, or clear all non-pinned entries?`,
  clearMovesToArchive: 'Recycle Bin is enabled: cleared entries will be moved there and can be restored.',
  clearDeletesPermanently: 'Recycle Bin is disabled: cleared entries will be permanently deleted and cannot be restored.',
  clearCurrentTab: 'Clear This Tab',
  clearAllHistory: 'Clear All',
  emptyArchive: 'Empty',
  emptyArchivePending: 'Emptying',
  emptyArchiveTitle: 'Empty Recycle Bin',
  emptyArchiveConfirm: (scope, count) => `Permanently delete all ${count} ${scope.toLowerCase()} item${count === 1 ? '' : 's'} from the Recycle Bin? This cannot be undone.`,
  emptyArchiveSuccess: (scope, count) => `Removed ${count} ${scope.toLowerCase()} item${count === 1 ? '' : 's'} from the Recycle Bin`,
  emptyArchiveFailed: 'Could not empty the Recycle Bin. Please try again.',

  memoTab: 'Memos',
  memoSearchPlaceholder: 'Search memos...',
  memoNew: 'New Memo',
  memoUntitled: 'Untitled',
  memoTitlePlaceholder: 'Title',
  memoBodyPlaceholder: 'Write something...',
  memoTagsPlaceholder: 'Tags, comma separated',
  memoEmpty: 'No memos yet',
  memoEmptyHint: 'Click to create your first memo',
  memoShowMore: 'Show more',
  memoShowLess: 'Show less',
  memoSetting: 'Memos',
  memoSettingDesc: 'Show memo tab in sidebar',
  memoColor: 'Memo Color',
  memoColorDesc: 'Customize memo module color, independent of theme',
  memoColorReset: 'Reset',
  archiveSetting: 'Recycle Bin',
  archiveSettingDesc: 'Keep deleted items in recycle bin, recoverable within 30 days',
  savePosition: 'Save Position',
  savePositionDesc: 'Remember last window position across restarts',

  copied: 'Copied!',
  dropImportPrompt: 'Drop to add to clipboard',
  dropImportHint: 'Text, images, and image files are supported',
  dropImporting: 'Adding to clipboard...',
  dropImportDone: 'Added to clipboard',
  dropImportFailed: 'Could not add to clipboard. Try again.',

  settings: 'Settings',
  language: 'Language',
  langZhCN: '中文',
  langEn: 'English',
  themeColor: 'Theme Color',
  themeColorDesc: 'Switch accent color while light/dark follows system',
  themeDefault: 'Blue',
  themeSakura: 'Pink',
  themeMode: 'Theme Mode',
  themeModeDesc: 'Follow system light/dark automatically or choose manually',
  themeSystem: 'System',
  themeLight: 'Light',
  themeDark: 'Dark',
  modernUi: 'Modern UI',
  modernUiDesc: 'Use the refreshed visual style, or turn off for the classic compact UI',
  autostart: 'Auto-start',
  autostartDesc: 'Launch on system startup',
  alwaysOnTop: 'Always on Top',
  alwaysOnTopDesc: 'Keep window above others',
  rawPreview: 'Raw Preview',
  rawPreviewDesc: 'Show clipboard content in raw format',
  followMode: 'Follow Mode',
  followModeDesc: 'Window follows caret position when shown via shortcut',
  autoUpdate: 'Auto Update',
  autoUpdateDesc: 'Check for updates on startup',
  experimentalFeatures: 'Experimental Features',
  experimentalFeaturesDesc: 'Show the experimental features entry. Experimental features are off by default.',
  multiSelectMode: 'Multi-select Mode',
  multiSelectModeDesc: 'Show Select Entries and enable Ctrl+click selection and Delete',
  clipboardMultiTag: 'Clipboard Multi-tag Display',
  clipboardMultiTagDesc: 'Show every detected category tag on clipboard entries',
  hideEntryColorStrip: 'Hide Entry Color Strip',
  hideEntryColorStripDesc: 'Hide the category color strip and original-content left border on clipboard entries',
  categoryTabSelectedColors: 'Multicolor Mode (Tab Labels)',
  categoryTabSelectedColorsDesc: 'Use the matching category color for the selected Tab label background',
  reclassifyHistory: 'Update History Categories',
  reclassifyHistoryDesc: 'Recommended after an app update or classification-rule change. Existing data is never updated automatically.',
  reclassifyHistoryPending: 'Updating Categories',
  reclassifyHistoryConfirmTitle: 'Reclassify History',
  reclassifyHistoryConfirm: 'Reclassify historical clipboard entries in the active storage using the current rules? Only category metadata and the search index will change. Content, dates, pins, and edit history remain untouched.',
  reclassifyHistoryConfirmAction: 'Start Reclassification',
  reclassifyHistorySuccess: (count) => `History categories updated: ${count} record(s) changed`,
  reclassifyHistoryFailed: 'Could not update history categories. Please try again.',
  classificationRulesCurrent: (version) => `Rules v${version} · history is current`,
  classificationRulesOutdated: (version) => `Rules v${version} · reclassification recommended`,
  categoryTabSorting: 'Tab Sorting',
  categoryTabSortingDesc: 'Allow dragging clipboard category tabs to reorder them',
  version: 'Version',
  shortcut: 'Shortcut',
  shortcutDesc: 'Show/hide window',
  shortcutRecording: 'Press new shortcut...',
  shortcutInvalid: 'Requires at least one modifier',
  checkUpdate: 'Check for Updates',
  checking: 'Checking...',
  upToDate: 'You are up to date',
  hasUpdate: (v) => `New version ${v} available`,
  updateFailed: 'Check failed, try again later',
  downloadUpdate: 'Download',
  updateCurrent: (v) => `Current ${v}`,
  updateLatest: (v) => `Latest ${v}`,
  releaseNotes: 'Release notes',
  noReleaseNotes: 'No release notes',
  backupRestore: 'Backup / Restore',
  backupRestoreDesc: 'Export or restore local clipboard, memos, and settings',
  backupCompatibilityNotice: 'New .scbackup package files record source and current app versions. Older gzip .scbackup / .json backups are no longer loaded. Cross-version restore is not guaranteed.',
  backupVersionMeta: (backupVersion, currentVersion) => `Backup v${backupVersion} · Current v${currentVersion}`,
  unknownVersion: 'Unknown',
  createBackup: 'Create Backup',
  creatingBackup: 'Backing up...',
  backupCreated: 'Backup created',
  openBackupFolder: 'Open Folder',
  restoreBackup: 'Restore Backup',
  restoringBackup: 'Restoring...',
  noBackups: 'No backups yet',
  restoreBackupConfirm: 'Restoring will replace current clipboard entries, memos, and settings. Continue?',
  restoreBackupDone: (clipboard, memos, settings) => `Restored ${clipboard} clipboard, ${memos} memo, ${settings} setting items`,
  backupFailed: 'Operation failed',
  systemSettings: 'System Settings',
  appearanceSettings: 'Appearance',
  featureSettings: 'Feature Settings',
  remoteStorage: 'Storage Settings',
  storageMode: 'Storage Mode',
  storageModeLocal: 'Local',
  storageModeRemote: 'External',
  storageStatusLocal: 'Local Storage',
  storageStatusConnected: 'External Storage',
  storageStatusFailed: 'External Storage',
  storageStatusNotReady: 'External Storage',
  localModeHint: 'Default mode. Clipboard entries and memos stay in the local SQLite database.',
  remoteModeHint: 'Use external PostgreSQL. Clipboard and memo bodies are written only to the external database.',
  remoteStoragePending: 'Saving will test the connection and initialize remote tables automatically',
  connectionMode: 'Connection',
  connectionUrl: 'URL',
  connectionManual: 'Manual',
  remoteProfiles: 'Saved Connections',
  noRemoteProfiles: 'External databases you switch to successfully will appear here',
  useRemoteProfile: 'Use',
  deleteRemoteProfile: 'Delete connection',
  deleteRemoteProfileConfirm: (name) => `Delete saved connection "${name}"? This will not delete data in the external database.`,
  lastUsedRemoteProfile: (time) => `Last ${time}`,
  databaseUrl: 'Database URL',
  databaseHost: 'Host',
  databasePort: 'Port',
  databaseName: 'Database',
  databaseUser: 'User',
  databasePassword: 'Password',
  databaseSsl: 'SSL',
  testingConnection: 'Testing...',
  storageConnectionReady: 'Connected and schema ready',
  storageConnectionFailed: 'Connection failed',
  saveAndUseLocal: 'Save and use local',
  saveAndUseRemote: 'Save and use remote',
  savingStorageConfig: 'Saving...',
  storageConfigSaved: 'Storage settings saved',
  storageConfigFailed: 'Save failed',
};

export const translationsMap: Record<Locale, Translations> = {
  'zh-CN': zhCN,
  en,
};
