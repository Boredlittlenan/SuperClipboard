const translations = {
  'zh-CN': {
    skipLink: '跳到主要内容',
    navFeatures: '功能',
    navWorkflow: '使用方式',
    navStorage: '存储',
    navFaq: '常见问题',
    navDownload: '下载',
    eyebrow: 'Windows 剪贴板工作台',
    heroTitle: '复制过的内容，不该只停留几秒。',
    heroLead: '自动保存并整理复制历史，也能收纳备忘录、恢复误删内容。数据默认留在本机，需要时再连接你自己的 PostgreSQL。',
    downloadSetup: '下载 Windows 安装版',
    viewGithub: '查看 GitHub',
    viewSource: '查看源码',
    viewLicense: '查看许可',
    viewRelease: '查看更新日志',
    visualSearch: '搜索剪贴板、链接、备忘录',
    visualEntryOneTitle: '项目路径',
    visualEntryTwoTitle: '发布记录',
    visualEntryThreeTitle: '备忘录',
    visualEntryThreeBody: '图片、标签和正文默认保存在本地。',
    visualFooterCount: '96 条记录',
    visualFooterStorage: '本地存储',
    sceneStorageLabel: '存储设置',
    sceneStorageMode: '本地 / 外部',
    sceneSavedConnections: '已保存连接 3',
    sceneMemoPill: '备忘录',
    factWindows: 'Windows x64',
    factLocal: '默认本地存储',
    factShortcut: 'Alt+X 唤起',
    releaseEyebrow: '当前版本',
    releaseTitle: '图片工作流与编辑去重进一步完善',
    releasePointOne: '文字、链接和图片可直接拖入窗口收录',
    releasePointTwo: '图片支持应用内预览、导出与轻量搜索',
    releasePointThree: '重复编辑、无改动保存与原文状态更准确',
    featuresEyebrow: '核心能力',
    featuresTitle: '把高频复制变成可找、可改、可恢复的工作流',
    featuresLead: '窗口不大，但把每天真正会反复使用的动作放在了一起。',
    featureClassifyTitle: '智能分类与搜索',
    featureClassifyBody: '自动识别文本、链接、图片、代码、邮箱和文件路径；混合内容可显示多个标签，搜索时更快定位。',
    featureMemoTitle: '图片备忘录',
    featureMemoBody: '标题、正文、粘贴图片、标签、置顶和拖拽排序都在同一条目里完成。',
    featureEditTitle: '编辑与原文',
    featureEditBody: '点击条目复制最新编辑结果；展开原文后可单独查看和复制首次捕获的内容。',
    featureArchiveTitle: '回收站',
    featureArchiveBody: '误删内容可恢复，剪贴板和备忘录分栏查看，并按剩余天数提示清理状态。',
    featureThemeTitle: '经典与新版 UI',
    featureThemeBody: '浅色、深色、跟随系统和蓝色/粉色主题均可切换；实验功能提供新版界面、多标签和多色 Tab 标签。',
    featureTrayTitle: '托盘和快捷键',
    featureTrayBody: '托盘常驻、开机自启、窗口置顶、全局快捷键和单实例唤起，适合每天反复打开。',
    workflowEyebrow: '使用方式',
    workflowTitle: '不改变复制习惯，只让历史随时可用',
    workflowLead: '继续使用 Ctrl+C，需要找回、编辑或粘贴时再唤出 SuperClipboard。',
    workflowOneTitle: '照常复制',
    workflowOneBody: '文本、链接、图片和文件路径自动进入历史，并完成分类与去重。',
    workflowTwoTitle: '按 Alt+X 唤出',
    workflowTwoBody: '按分类浏览或直接搜索；窗口被遮挡时再次按快捷键会回到前台。',
    workflowThreeTitle: '点击复制或粘贴',
    workflowThreeBody: '点击条目复制内容；由快捷键唤出时，软件还会尝试把内容输入回之前的应用。',
    workflowFourTitle: '重要内容继续整理',
    workflowFourBody: '置顶、编辑、转为备忘录，或者在误删后从回收站恢复。',
    storageEyebrow: '数据与隐私',
    storageTitle: '默认留在本地，外部存储由你决定',
    storageLead: '本地模式使用 SQLite。切换到外部模式后，剪贴板和备忘录正文写入你配置的 PostgreSQL；连接配置仍保存在本机。',
    storageStatusLocal: '本地模式 · 默认',
    storageStatusExternal: '外部模式 · 可选',
    storageLocalLabel: '默认模式',
    storageLocalTitle: '本地 SQLite',
    storageLocalBody: '剪贴板、备忘录和设置写入本机应用数据目录，软件不会主动把内容上传到第三方服务。',
    storageExternalLabel: '可选模式',
    storageExternalTitle: '自己的 PostgreSQL',
    storageExternalBody: '可保存最多 12 个已验证连接并快速切换。外部数据库由用户自行提供和管理。',
    storageBackupLabel: '本地工具',
    storageBackupTitle: '.scbackup 备份 / 恢复',
    storageBackupBody: '本地模式可创建带版本、数据清单和校验信息的备份包；跨版本恢复不保证完全兼容。',
    storageNote: '提示：已保存的数据库账号与凭据位于本机设置数据库中。删除连接记录不会删除外部数据库及其中的数据。',
    supportEyebrow: '系统支持',
    supportTitle: '为 Windows x64 构建',
    supportBody: '当前提供 Windows 安装版和 MSI 包。应用支持系统托盘、全局快捷键、开机自启与深浅色模式。',
    supportPlatformLabel: '平台',
    supportVersionLabel: '当前版本',
    supportPackageLabel: '安装包',
    supportLicenseLabel: '许可',
    supportLicenseValue: '非商业使用',
    faqTitle: '使用前常见问题',
    faqLead: '把存储边界、自动粘贴和备份限制先说清楚。',
    faqLocalQuestion: '默认会把内容上传到云端吗？',
    faqLocalAnswer: '不会。默认使用本地 SQLite；只有用户主动配置并切换到外部模式后，正文数据才会写入指定 PostgreSQL。',
    faqPasteQuestion: '点击条目一定会自动粘贴吗？',
    faqPasteAnswer: '快捷键唤出窗口后点击条目时，软件会尝试恢复之前应用的焦点并发送 Ctrl+V。不同软件的焦点恢复速度不同，偶尔可能只完成复制。',
    faqBackupQuestion: '备份包含外部数据库吗？',
    faqBackupAnswer: '不包含。当前 .scbackup 仅用于本地模式；外部 PostgreSQL 应使用数据库自身的备份方案。',
    faqCommercialQuestion: '可以商用或二次分发吗？',
    faqCommercialAnswer: '源代码允许非商业用途查看、使用、修改和分发。商业使用需要获得版权持有人的明确书面许可。',
    downloadEyebrow: '当前版本 · 3.4.0',
    downloadTitle: '现在开始整理你的剪贴板',
    downloadBody: '普通用户推荐 Windows 安装版；需要集中部署或熟悉 MSI 的用户可选择 MSI 包。',
    downloadRecommended: '推荐',
    downloadMsi: '下载 MSI 包',
    siteVersion: '官网 v2.0',
    footerTagline: '给 Windows 的轻量剪贴板工作台。',
    footerRepo: 'GitHub 仓库',
  },
  en: {
    skipLink: 'Skip to main content',
    navFeatures: 'Features',
    navWorkflow: 'How it works',
    navStorage: 'Storage',
    navFaq: 'FAQ',
    navDownload: 'Download',
    eyebrow: 'Clipboard workspace for Windows',
    heroTitle: 'Copied content deserves more than a few seconds.',
    heroLead: 'Save and organize clipboard history automatically, keep visual memos, and recover accidental deletions. Data stays local by default, with your own PostgreSQL available when needed.',
    downloadSetup: 'Download Windows setup',
    viewGithub: 'View GitHub',
    viewSource: 'View source',
    viewLicense: 'View license',
    viewRelease: 'View release notes',
    visualSearch: 'Search clipboard, links, memos',
    visualEntryOneTitle: 'Project path',
    visualEntryTwoTitle: 'Release page',
    visualEntryThreeTitle: 'Memo',
    visualEntryThreeBody: 'Images, tags, and rich text stay local by default.',
    visualFooterCount: '96 entries',
    visualFooterStorage: 'Local storage',
    sceneStorageLabel: 'Storage settings',
    sceneStorageMode: 'Local / External',
    sceneSavedConnections: '3 saved connections',
    sceneMemoPill: 'Memos',
    factWindows: 'Windows x64',
    factLocal: 'Local by default',
    factShortcut: 'Alt+X shortcut',
    releaseEyebrow: 'Current release',
    releaseTitle: 'A smoother image workflow with cleaner edit history',
    releasePointOne: 'Drop text, links, and images directly into the app',
    releasePointTwo: 'Preview, export, and search images without Base64 noise',
    releasePointThree: 'Accurate deduplication and no-op edit handling',
    featuresEyebrow: 'Core features',
    featuresTitle: 'Turn frequent copying into a searchable, editable, recoverable workflow',
    featuresLead: 'A compact window that keeps the actions you actually repeat every day together.',
    featureClassifyTitle: 'Smart categories and search',
    featureClassifyBody: 'Detect text, links, images, code, emails, and file paths. Mixed content can show multiple labels for faster search.',
    featureMemoTitle: 'Visual memos',
    featureMemoBody: 'Titles, rich text, pasted images, tags, pinning, and drag sorting all live in one memo entry.',
    featureEditTitle: 'Edit and original',
    featureEditBody: 'Click an entry to copy the latest edit, or expand the original to inspect and copy the first captured content.',
    featureArchiveTitle: 'Recycle bin',
    featureArchiveBody: 'Recover accidental deletions, browse clipboard and memos separately, and see cleanup timing at a glance.',
    featureThemeTitle: 'Classic and Modern UI',
    featureThemeBody: 'Switch light, dark, system, and blue/pink themes. Experimental options add Modern UI, multi-label entries, and multicolor Tab labels.',
    featureTrayTitle: 'Tray and shortcuts',
    featureTrayBody: 'Tray residency, auto-start, always-on-top, global shortcuts, and single-instance launch are built for daily use.',
    workflowEyebrow: 'How it works',
    workflowTitle: 'Keep your copy habits, make the history reusable',
    workflowLead: 'Continue using Ctrl+C. Open SuperClipboard only when you need to find, edit, or paste something again.',
    workflowOneTitle: 'Copy as usual',
    workflowOneBody: 'Text, links, images, and file paths enter history automatically with categorization and deduplication.',
    workflowTwoTitle: 'Open with Alt+X',
    workflowTwoBody: 'Browse by category or search directly. If another window covers SuperClipboard, the shortcut brings it forward.',
    workflowThreeTitle: 'Click to copy or paste',
    workflowThreeBody: 'Click an entry to copy it. When opened by shortcut, the app also tries to type it back into the previous app.',
    workflowFourTitle: 'Keep important content organized',
    workflowFourBody: 'Pin it, edit it, turn it into a memo, or recover it later from the recycle bin.',
    storageEyebrow: 'Data and privacy',
    storageTitle: 'Local by default, external only when you choose',
    storageLead: 'Local mode uses SQLite. In External mode, clipboard and memo bodies go to your PostgreSQL database while connection settings stay on this device.',
    storageStatusLocal: 'Local mode · Default',
    storageStatusExternal: 'External mode · Optional',
    storageLocalLabel: 'Default mode',
    storageLocalTitle: 'Local SQLite',
    storageLocalBody: 'Clipboard entries, memos, and settings stay in the local app-data directory. The app does not upload content to third-party services on its own.',
    storageExternalLabel: 'Optional mode',
    storageExternalTitle: 'Your PostgreSQL',
    storageExternalBody: 'Save and quickly switch between up to 12 verified connections. You provide and manage the external database.',
    storageBackupLabel: 'Local tool',
    storageBackupTitle: '.scbackup Backup / Restore',
    storageBackupBody: 'Local mode creates packages with source version, data manifest, and checksums. Cross-version restore is not guaranteed.',
    storageNote: 'Note: saved database accounts and credentials remain in the local settings database. Removing a connection never deletes the external database or its data.',
    supportEyebrow: 'System support',
    supportTitle: 'Built for Windows x64',
    supportBody: 'Windows setup and MSI packages are available. The app supports the system tray, global shortcuts, auto-start, and light/dark modes.',
    supportPlatformLabel: 'Platform',
    supportVersionLabel: 'Current version',
    supportPackageLabel: 'Packages',
    supportLicenseLabel: 'License',
    supportLicenseValue: 'Non-commercial use',
    faqTitle: 'Questions before you start',
    faqLead: 'Clear answers about storage boundaries, automatic paste, and backup limits.',
    faqLocalQuestion: 'Does the app upload content by default?',
    faqLocalAnswer: 'No. Local SQLite is the default. Body data is written to PostgreSQL only after you configure and switch to External mode.',
    faqPasteQuestion: 'Does clicking an entry always paste automatically?',
    faqPasteAnswer: 'After shortcut launch, the app tries to restore focus to the previous window and send Ctrl+V. Focus timing differs between apps, so it may occasionally only copy.',
    faqBackupQuestion: 'Does the backup include external databases?',
    faqBackupAnswer: 'No. The current .scbackup format covers Local mode only. Use PostgreSQL backup tools for an external database.',
    faqCommercialQuestion: 'Can I use or redistribute it commercially?',
    faqCommercialAnswer: 'The source may be viewed, used, modified, and redistributed for non-commercial purposes. Commercial use requires explicit written permission from the copyright holder.',
    downloadEyebrow: 'Current release · 3.4.0',
    downloadTitle: 'Start organizing your clipboard',
    downloadBody: 'Windows setup is recommended for most users. Choose MSI if you manage deployment or prefer MSI packages.',
    downloadRecommended: 'Recommended',
    downloadMsi: 'Download MSI package',
    siteVersion: 'Website v2.0',
    footerTagline: 'A focused clipboard workspace for Windows.',
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

const navLinks = Array.from(document.querySelectorAll('.nav a[href^="#"]'));
const navSections = navLinks
  .map((link) => document.querySelector(link.getAttribute('href')))
  .filter(Boolean);

if ('IntersectionObserver' in window) {
  const navObserver = new IntersectionObserver((entries) => {
    const visible = entries
      .filter((entry) => entry.isIntersecting)
      .sort((a, b) => b.intersectionRatio - a.intersectionRatio)[0];
    if (!visible) return;
    navLinks.forEach((link) => {
      link.classList.toggle('is-active', link.getAttribute('href') === `#${visible.target.id}`);
    });
  }, { rootMargin: '-20% 0px -65% 0px', threshold: [0.05, 0.2, 0.5] });
  navSections.forEach((section) => navObserver.observe(section));
}

applyLanguage(preferredLanguage());
