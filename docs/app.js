const translations = {
  'zh-CN': {
    skipLink: '跳到主要内容',
    navFeatures: '功能',
    navStorage: '存储',
    navSupport: '系统',
    navDownload: '下载',
    eyebrow: 'Windows 剪贴板工作台',
    heroTitle: '复制过的内容，不该只停留几秒。',
    heroLead: 'SuperClipboard 把剪贴板历史、备忘录、回收站、主题和托盘快捷操作收进一个轻量窗口里。默认本地保存，也可以在测试版存储设置中连接自己的 PostgreSQL。',
    downloadLatest: '下载最新版',
    viewGithub: '查看源码',
    viewLicense: '查看许可',
    visualSearch: '搜索剪贴板、链接、备忘录',
    visualEntryOneTitle: '项目路径',
    visualEntryTwoTitle: '发布记录',
    visualEntryThreeTitle: '备忘录',
    visualEntryThreeBody: '图片、标签和正文默认保存在本地。',
    sceneStorageLabel: '存储设置',
    sceneStorageMode: '本地 / 外部',
    sceneMemoPill: '备忘录',
    factWindows: 'Windows x64',
    factLocal: '默认本地存储',
    factShortcut: 'Alt+X 唤起',
    featuresEyebrow: '核心能力',
    featuresTitle: '像工作台一样整理复制历史',
    featuresLead: '高频复制、查找、备忘和恢复都在同一个窗口里完成，不把简单动作变成复杂流程。',
    featureClassifyTitle: '智能分类',
    featureClassifyBody: '自动识别文本、链接、图片、代码、邮箱和文件路径，搜索时更快定位。',
    featureMemoTitle: '备忘录',
    featureMemoBody: '标题、正文、图片预览、标签、置顶和拖拽排序，为临时信息留一个清爽位置。',
    featureEditTitle: '编辑与原文',
    featureEditBody: '剪贴板条目可以编辑，原始内容可保留查看，备忘录默认使用格式化预览。',
    featureArchiveTitle: '回收站',
    featureArchiveBody: '误删内容可恢复，剪贴板和备忘录分栏查看，并按剩余天数提示清理状态。',
    featureThemeTitle: '主题细节',
    featureThemeBody: '支持浅色、深色、跟随系统和蓝色/粉色强调色，备忘录配色保持独立。',
    featureTrayTitle: '托盘和快捷键',
    featureTrayBody: '托盘常驻、开机自启、全局快捷键和单实例唤起，适合每天反复打开。',
    storageEyebrow: '数据与隐私',
    storageTitle: '先本地可靠，再给外部存储留入口',
    storageLead: '默认数据保存在本机 SQLite。需要多设备方案时，可开启存储设置(测试版)，连接自己的 PostgreSQL 数据库。',
    storageLocalLabel: '默认模式',
    storageLocalTitle: '本地 SQLite',
    storageLocalBody: '剪贴板、备忘录和设置默认写入本机应用数据目录，软件不会主动上传内容。',
    storageExternalLabel: '测试版',
    storageExternalTitle: '外部 PostgreSQL',
    storageExternalBody: '用户可以配置自己的数据库连接。外部模式下正文数据写入外部数据库。',
    storageBackupLabel: '工具',
    storageBackupTitle: '备份 / 恢复',
    storageBackupBody: '备份入口已归入存储设置，本地模式使用新版 .scbackup 包并记录版本与校验信息。',
    supportEyebrow: '系统支持',
    supportTitle: '当前专注 Windows x64',
    supportBody: '提供 NSIS 安装器和 MSI 包，适合普通安装、重复启动唤起和系统托盘常驻使用。',
    supportWindowsStatus: '当前支持',
    supportWindowsBody: 'SuperClipboard 2.3.2',
    downloadEyebrow: '当前版本',
    downloadBody: '推荐下载 Windows x64 安装器，也可以选择 MSI 包。源码以非商用许可开放。',
    siteVersion: '官网 v1.1.0',
    footerRepo: 'GitHub 仓库',
  },
  en: {
    skipLink: 'Skip to main content',
    navFeatures: 'Features',
    navStorage: 'Storage',
    navSupport: 'System',
    navDownload: 'Download',
    eyebrow: 'Clipboard workspace for Windows',
    heroTitle: 'Copied content deserves more than a few seconds.',
    heroLead: 'SuperClipboard keeps clipboard history, memos, recycle bin recovery, themes, and tray shortcuts in one lightweight window. Data stays local by default, with beta storage settings for your own PostgreSQL database.',
    downloadLatest: 'Download latest',
    viewGithub: 'View source',
    viewLicense: 'View license',
    visualSearch: 'Search clipboard, links, memos',
    visualEntryOneTitle: 'Project path',
    visualEntryTwoTitle: 'Release note',
    visualEntryThreeTitle: 'Memo',
    visualEntryThreeBody: 'Images, tags, and rich text stay local by default.',
    sceneStorageLabel: 'Storage settings',
    sceneStorageMode: 'Local / External',
    sceneMemoPill: 'Memos',
    factWindows: 'Windows x64',
    factLocal: 'Local by default',
    factShortcut: 'Alt+X shortcut',
    featuresEyebrow: 'Core features',
    featuresTitle: 'Organize clipboard history like a workspace',
    featuresLead: 'Frequent copying, search, memo capture, and recovery live in one focused window instead of becoming a chore.',
    featureClassifyTitle: 'Smart categories',
    featureClassifyBody: 'Detect text, links, images, code snippets, emails, and file paths so search lands faster.',
    featureMemoTitle: 'Memos',
    featureMemoBody: 'Titles, rich body content, image previews, tags, pinning, and drag sorting keep temporary notes tidy.',
    featureEditTitle: 'Edit and original',
    featureEditBody: 'Clipboard entries can be edited, original content can be kept, and memos use formatted preview by default.',
    featureArchiveTitle: 'Recycle bin',
    featureArchiveBody: 'Recover deleted content, view clipboard and memos separately, and see cleanup timing at a glance.',
    featureThemeTitle: 'Theme details',
    featureThemeBody: 'Light, dark, system mode, and blue/pink accents are supported while memo color stays independent.',
    featureTrayTitle: 'Tray and shortcut',
    featureTrayBody: 'Tray residency, auto-start, global shortcut, and single-instance launch are built for repeated daily use.',
    storageEyebrow: 'Data and privacy',
    storageTitle: 'Reliable local storage first, external storage when needed',
    storageLead: 'Data is stored in local SQLite by default. For multi-device plans, enable Storage Settings (Beta) and connect your own PostgreSQL database.',
    storageLocalLabel: 'Default',
    storageLocalTitle: 'Local SQLite',
    storageLocalBody: 'Clipboard entries, memos, and settings are written to the local app data directory by default. The app does not upload content on its own.',
    storageExternalLabel: 'Beta',
    storageExternalTitle: 'External PostgreSQL',
    storageExternalBody: 'Users can configure their own database connection. In External mode, body content is written to the external database.',
    storageBackupLabel: 'Tooling',
    storageBackupTitle: 'Backup / Restore',
    storageBackupBody: 'Backup controls now live inside Storage Settings. Local mode uses the new .scbackup package with version and checksum metadata.',
    supportEyebrow: 'System support',
    supportTitle: 'Focused on Windows x64 now',
    supportBody: 'NSIS setup and MSI packages are available for normal installs, repeat launches, and tray-based daily use.',
    supportWindowsStatus: 'Supported now',
    supportWindowsBody: 'SuperClipboard 2.3.2',
    downloadEyebrow: 'Current version',
    downloadBody: 'The Windows x64 setup installer is recommended. An MSI package is also available. Source is available under a non-commercial license.',
    siteVersion: 'Website v1.1.0',
    footerRepo: 'GitHub repository',
  },
};

const preferredLanguage = () => {
  const saved = window.localStorage.getItem('site-language');
  if (saved === 'zh-CN' || saved === 'en') return saved;
  return navigator.language.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en';
};

const applyLanguage = (lang) => {
  const dict = translations[lang] ?? translations['zh-CN'];
  document.documentElement.lang = lang;
  document.querySelectorAll('[data-i18n]').forEach((node) => {
    const key = node.getAttribute('data-i18n');
    if (key && dict[key]) node.textContent = dict[key];
  });
  document.querySelectorAll('[data-lang]').forEach((button) => {
    const active = button.getAttribute('data-lang') === lang;
    button.classList.toggle('active', active);
    button.setAttribute('aria-pressed', String(active));
  });
  window.localStorage.setItem('site-language', lang);
};

document.querySelectorAll('[data-lang]').forEach((button) => {
  button.addEventListener('click', () => {
    const lang = button.getAttribute('data-lang');
    if (lang) applyLanguage(lang);
  });
});

applyLanguage(preferredLanguage());
