const translations = {
  'zh-CN': {
    skipLink: '跳到主要内容',
    navFeatures: '功能',
    navSupport: '系统支持',
    navPrivacy: '隐私',
    navDownload: '下载',
    eyebrow: 'Windows 剪贴板管理器',
    heroTitle: 'SuperClipboard',
    heroLead: '默认本地保存剪贴板历史，自动识别内容类型，并提供备忘录、回收站、主题和托盘快捷操作。',
    downloadLatest: '下载最新版',
    viewGithub: '查看源码',
    visualSearch: '搜索剪贴板',
    visualItemOneTitle: '项目路径',
    visualItemTwoTitle: '备忘录',
    visualItemTwoBody: '图片、标签和正文默认保存在本地。',
    featuresEyebrow: '核心能力',
    featuresTitle: '为每天高频复制粘贴而做',
    featureLocalTitle: '本地历史',
    featureLocalBody: '默认保存在本机 SQLite，也可在测试版存储设置中切换外部 PostgreSQL。',
    featureClassifyTitle: '智能分类',
    featureClassifyBody: '自动识别文本、链接、图片、代码、邮箱和文件路径。',
    featureMemoTitle: '备忘录',
    featureMemoBody: '可选启用标题、正文、图片预览、标签和拖拽排序。',
    featureTrayTitle: '托盘和快捷键',
    featureTrayBody: '全局快捷键、托盘菜单、单实例启动和开机自启。',
    supportEyebrow: '系统支持',
    supportTitle: 'Windows x64 当前正式支持',
    supportWindowsStatus: '当前支持',
    supportWindowsBody: '当前正式支持 Windows x64，安装包包括 NSIS 安装器和 MSI 包。',
    privacyEyebrow: '隐私',
    privacyTitle: '内容默认留在你的电脑上',
    privacyBody: 'SuperClipboard 默认不上传剪贴板内容。切换外部存储时，数据写入用户自己配置的 PostgreSQL。只有启用检查更新时，软件会访问 GitHub Releases 获取版本信息。',
    downloadEyebrow: '当前版本',
    downloadBody: '推荐下载 Windows x64 安装器，也可以选择 MSI 包。',
    siteVersion: '官网 v1.0.1',
    footerRepo: 'GitHub 仓库',
  },
  en: {
    skipLink: 'Skip to main content',
    navFeatures: 'Features',
    navSupport: 'System support',
    navPrivacy: 'Privacy',
    navDownload: 'Download',
    eyebrow: 'Clipboard manager for Windows',
    heroTitle: 'SuperClipboard',
    heroLead: 'Keep clipboard history local by default, classify content automatically, and work faster with memos, recycle bin recovery, themes, and tray shortcuts.',
    downloadLatest: 'Download latest',
    viewGithub: 'View source',
    visualSearch: 'Search clipboard',
    visualItemOneTitle: 'Project path',
    visualItemTwoTitle: 'Memo',
    visualItemTwoBody: 'Images, tags, and rich text stay local by default.',
    featuresEyebrow: 'Core features',
    featuresTitle: 'Built for high-frequency copy and paste',
    featureLocalTitle: 'Local history',
    featureLocalBody: 'Data stays in local SQLite by default, with beta external PostgreSQL storage settings available.',
    featureClassifyTitle: 'Smart categories',
    featureClassifyBody: 'Detect text, links, images, code snippets, emails, and file paths automatically.',
    featureMemoTitle: 'Memos',
    featureMemoBody: 'Enable titles, rich body content, image previews, tags, pinning, and drag sorting.',
    featureTrayTitle: 'Tray and shortcut',
    featureTrayBody: 'Use a global shortcut, tray menu, single-instance launch, and auto-start support.',
    supportEyebrow: 'System support',
    supportTitle: 'Windows x64 is supported now',
    supportWindowsStatus: 'Supported now',
    supportWindowsBody: 'Windows x64 is the current supported platform, with NSIS setup and MSI packages.',
    privacyEyebrow: 'Privacy',
    privacyTitle: 'Your clipboard stays on your PC',
    privacyBody: 'SuperClipboard does not upload clipboard content by default. In External storage mode, data is written to the user-configured PostgreSQL database. When update checks are enabled, it only contacts GitHub Releases for version information.',
    downloadEyebrow: 'Current version',
    downloadBody: 'The Windows x64 setup installer is recommended. An MSI package is also available.',
    siteVersion: 'Website v1.0.1',
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
