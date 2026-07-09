# SuperClipboard

[English](README.md)

SuperClipboard，中文名「超级剪贴板」，是一个基于 Rust、Tauri、React、TypeScript 构建的 Windows 剪贴板管理器。它会在本地保存剪贴板历史，自动分类内容，并提供可选的备忘录、回收站、主题、托盘控制和快捷粘贴能力。

[官网](https://boredlittlenan.github.io/SuperClipboard/) · [最新版本下载](https://github.com/Boredlittlenan/SuperClipboard/releases/latest) · [版本日志](VERSIONS.md) · [更新记录](CHANGELOG.md)

## 下载

从 [GitHub Releases](https://github.com/Boredlittlenan/SuperClipboard/releases/latest) 下载最新 Windows 安装包。

- `SuperClipboard_2.5.0_x64-setup.exe`：推荐使用的 Windows 安装器
- `SuperClipboard_2.5.0_x64_en-US.msi`：MSI 安装包

## 功能亮点

- 智能分类：文本、链接、图片、代码、邮箱、文件路径
- 本地 SQLite 历史记录，支持 SHA-256 去重和索引搜索
- 剪贴板条目支持置顶、编辑、复制、删除和恢复
- 可选备忘录模块：标题、富文本正文、粘贴图片预览、标签、置顶、搜索、拖拽排序
- 可选回收站：剪贴板和备忘录分栏查看，30 天自动清理
- 全局快捷键、托盘控制、单实例启动、开机自启
- 主题支持跟随系统 / 浅色 / 深色，主题强调色可切换
- 存储设置支持本地 / 外部 PostgreSQL 模式，并提供 `.scbackup` 本地备份/恢复工具
- 实验功能面板提供新版界面和剪贴板多标签显示开关
- 首次启动跟随系统语言，支持中文和英文界面
- 通过 GitHub Releases 检查更新，并在面板内预览更新日志

## 系统支持

Windows x64 当前正式支持，提供 NSIS 安装器和 MSI 安装包。

## 默认行为

- 当前版本：`2.5.0`
- 默认快捷键：`Alt+X`
- 启动行为：先完成窗口定位再显示桌面主窗口，同时保留托盘图标
- 界面样式：默认旧版界面，可在实验功能中开启新版界面
- 主题模式：跟随系统
- 主题配色：蓝色
- 开机自启：开启
- 窗口置顶：关闭
- 原格式预览：剪贴板默认关闭；备忘录默认使用格式化预览
- 自动检查更新：开启
- 备忘录和回收站：默认关闭，可在设置中开启

## 使用说明

### 设置快捷键

打开设置面板，点击“快捷键”右侧按钮后直接按下组合键即可保存。支持 `Alt`、`Ctrl`、`Shift`、`Win` 等修饰键。再次点击录制中的按钮会取消本次设置；按下与当前相同的快捷键也会重新保存并生效。

### 使用剪贴板

复制文本、链接、图片、代码、邮箱或路径后，条目会自动进入列表。点击条目会复制回系统剪贴板；如果窗口由全局快捷键唤起，点击条目后会尝试隐藏窗口并向之前的活动应用发送 `Ctrl+V`。

如果窗口已经显示但被其他应用挡住，再按全局快捷键会把窗口拉到前台；只有窗口已经在前台时，快捷键才会隐藏窗口。

### 原格式预览

“原格式预览”只影响剪贴板条目。开启后，文本内容会尽量按原始换行和空格显示，适合查看代码、日志、配置片段；关闭后列表更紧凑。备忘录始终使用格式化预览，不受这个开关影响。

### 配置外部存储

存储入口默认显示在设置按钮左侧。进入后可选择“本地”或“外部”：

- 本地：数据写入本机 SQLite，是默认模式。
- 外部：填写 PostgreSQL 连接信息，保存时会自动测试连接并初始化远端表；外部模式下剪贴板和备忘录正文写入外部数据库。

备份/恢复仅在本地模式显示，使用 `.scbackup` 包格式，包内记录创建版本、数据清单和校验信息。跨版本恢复不保证完全兼容。

### 实验功能

在设置中开启“实验功能”后，标题栏会显示实验功能入口。实验功能和里面的选项默认关闭。

- 新版界面：从经典紧凑界面切换到新版视觉样式。
- 剪贴板多标签显示：在剪贴板条目中显示所有识别出的分类标签，并使用多段颜色标识条。

### 分类与标签识别

剪贴板条目现在同时保存主分类 `category` 和分类标签列表 `category_tags`。分类标签和数量统计可以命中标签列表，主分类继续作为默认样式和兼容锚点。

多标签显示放在实验功能中。关闭时，剪贴板条目只显示主分类，列表更清爽；开启后，包含链接、邮箱等混合内容的条目可以显示多个分类标签。

## 隐私

默认情况下，SuperClipboard 会把剪贴板条目、备忘录和设置保存在本地 SQLite 数据库中。切换到存储设置的外部模式后，剪贴板和备忘录正文会写入用户配置的 PostgreSQL 数据库。开启自动检查更新时，会访问 GitHub Releases 获取版本信息。

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
