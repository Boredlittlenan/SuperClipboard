# SuperClipboard

[English](README.md)

SuperClipboard，中文名「超级剪贴板」，是一个基于 Rust、Tauri、React、TypeScript 构建的 Windows 剪贴板管理器。它会在本地保存剪贴板历史，自动分类内容，并提供可选的备忘录、回收站、主题、托盘控制和快捷粘贴能力。

[最新版本下载](https://github.com/Boredlittlenan/SuperClipboard/releases/latest) · [版本日志](VERSIONS.md) · [更新记录](CHANGELOG.md)

## 下载

从 [GitHub Releases](https://github.com/Boredlittlenan/SuperClipboard/releases/latest) 下载最新 Windows 安装包。

- `SuperClipboard_2.0.2_x64-setup.exe`：推荐使用的 Windows 安装器
- `SuperClipboard_2.0.2_x64_en-US.msi`：MSI 安装包

## 功能亮点

- 智能分类：文本、链接、图片、代码、邮箱、文件路径
- 本地 SQLite 历史记录，支持 SHA-256 去重和索引搜索
- 剪贴板条目支持置顶、编辑、复制、删除和恢复
- 可选备忘录模块：标题、富文本正文、粘贴图片预览、标签、置顶、搜索、拖拽排序
- 可选回收站：剪贴板和备忘录分栏查看，30 天自动清理
- 全局快捷键、托盘控制、单实例启动、开机自启
- 主题支持跟随系统 / 浅色 / 深色，主题强调色可切换
- 首次启动跟随系统语言，支持中文和英文界面
- 通过 GitHub Releases 检查更新

## 默认行为

- 当前版本：`2.0.2`
- 默认快捷键：`Alt+X`
- 启动行为：启动后直接显示桌面主窗口，同时保留托盘图标
- 主题模式：跟随系统
- 主题配色：蓝色
- 开机自启：开启
- 窗口置顶：关闭
- 原格式预览：剪贴板默认关闭；备忘录默认使用格式化预览
- 自动检查更新：开启
- 备忘录和回收站：默认关闭，可在设置中开启

## 使用说明

- 点击剪贴板条目会复制回系统剪贴板。
- 如果窗口是通过全局快捷键唤起的，点击剪贴板条目还会隐藏窗口，并模拟 `Ctrl+V` 粘贴到之前的活动应用。
- 点击即粘贴现状：当前实现依赖 SuperClipboard 隐藏后原输入窗口重新获得焦点，再延迟发送 `Ctrl+V`；部分软件或焦点切换较慢时可能表现为不够灵敏，后续会单独重构。
- 反复点击软件快捷图标会唤起已有实例，不会生成重复托盘图标。
- 从 `SuperClipboard3` 升级到 `SuperClipboard` 时，会自动迁移旧本地数据目录。

## 隐私

SuperClipboard 会把剪贴板条目、备忘录和设置保存在本地 SQLite 数据库中。软件不会上传剪贴板内容。开启自动检查更新时，会访问 GitHub Releases 获取版本信息。

## 技术栈

- 后端：Rust、Tauri v2、SQLite（`rusqlite`）、`arboard`
- 前端：React 19、TypeScript、Vite 8
- 目标平台：Windows x64

## 开发

```bash
# 安装依赖
pnpm install

# 开发模式运行
pnpm tauri:dev

# 构建前端
pnpm build

# 构建 Windows 安装包
pnpm tauri:build
```

## 项目结构

```text
src-tauri/
  src/
    clipboard.rs        # 剪贴板监控服务
    classifier.rs       # 内容类型分类
    storage.rs          # SQLite 存储层
    autostart.rs        # Windows 开机自启注册表集成
    window_position.rs  # 默认窗口定位与工作区边界修正
    lib.rs              # Tauri 命令与应用初始化
    main.rs             # 入口文件
src/
  components/           # React UI 组件
  api/                  # Tauri 命令封装
  i18n/                 # 翻译与国际化 Context
  types/                # TypeScript 类型定义
```

## 后续规划

- 提升点击即粘贴可靠性：记录上一个活动窗口，确认焦点恢复后再发送 `Ctrl+V`。
- 大量剪贴板历史下引入虚拟列表，降低滚动和渲染压力。
- 在交互稳定后重新设计插入符跟随和保存窗口位置功能。
