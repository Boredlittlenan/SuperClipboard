# SuperClipboard

[English](README.md)

SuperClipboard，中文名「超级剪贴板」，是一个基于 Rust、Tauri、React、TypeScript 构建的 Windows 剪贴板管理器。它会在本地保存剪贴板历史，自动分类内容，并提供可选的备忘录、回收站、主题、托盘控制和快捷粘贴能力。

[官网](https://boredlittlenan.github.io/SuperClipboard/) · [最新版本下载](https://github.com/Boredlittlenan/SuperClipboard/releases/latest) · [版本日志](VERSIONS.md) · [更新记录](CHANGELOG.md)

## 下载

从 [GitHub Releases](https://github.com/Boredlittlenan/SuperClipboard/releases/latest) 下载最新 Windows 安装包。

- `SuperClipboard_2.30.0_x64-setup.exe`：推荐使用的 Windows 安装器
- `SuperClipboard_2.30.0_x64_en-US.msi`：MSI 安装包

## 功能亮点

- 智能分类：文本、链接、图片、代码、邮箱、文件路径
- 本地 SQLite 历史记录，支持 SHA-256 去重和索引搜索
- 剪贴板条目支持置顶、编辑、复制、删除和恢复
- 可选备忘录模块：标题、富文本正文、粘贴图片预览、标签、置顶、搜索、拖拽排序
- 可选回收站：剪贴板和备忘录分栏查看，30 天自动清理
- 全局快捷键、托盘控制、单实例启动、开机自启
- 主题支持跟随系统 / 浅色 / 深色，主题强调色可切换
- 存储设置(测试版)支持本地 / 外部 PostgreSQL 模式，并提供 `.scbackup` 本地备份/恢复工具
- 首次启动跟随系统语言，支持中文和英文界面
- 通过 GitHub Releases 检查更新，并在面板内预览更新日志

## 系统支持

Windows x64 当前正式支持，提供 NSIS 安装器和 MSI 安装包。

## 默认行为

- 当前版本：`2.30.0`
- 默认快捷键：`Alt+X`
- 启动行为：先完成窗口定位再显示桌面主窗口，同时保留托盘图标
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
- 窗口可见但被其他应用遮挡时，按全局快捷键会把它重新拉到前台；只有窗口已经在前台时，快捷键才会隐藏窗口。
- 鼠标悬浮托盘图标会显示当前语言下的软件名。
- 从 `SuperClipboard3` 升级到 `SuperClipboard` 时，会自动迁移旧本地数据目录。
- 自 v2.1.0 起，图片剪贴板去重会使用真实图片内容，备忘录自动标签由后端分类器统一判断。
- 自 v2.1.6 起，检查更新会先显示 Release 更新说明。
- 自 v2.2.0 起，可通过存储设置(测试版)开启本地 / 外部存储模式配置。
- 自 v2.30.0 起，本地备份/恢复使用新版 `.scbackup` 包格式，内含清单、数据和校验信息。旧 gzip `.scbackup` 和裸 `.json` 备份不再读取。

## 隐私

默认情况下，SuperClipboard 会把剪贴板条目、备忘录和设置保存在本地 SQLite 数据库中。开启存储设置(测试版)并切换到外部模式后，剪贴板和备忘录正文会写入用户配置的 PostgreSQL 数据库。开启自动检查更新时，会访问 GitHub Releases 获取版本信息。

## 开源协议

SuperClipboard 源代码仅允许非商业用途查看、使用、修改和分发。未经版权持有人明确书面许可，不允许商用。详见 [LICENSE.md](LICENSE.md)。

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
    remote_storage.rs   # 外部 PostgreSQL 存储层
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
