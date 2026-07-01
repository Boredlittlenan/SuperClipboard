# SuperClipboard

[中文文档](README.zh-CN.md)

SuperClipboard is a lightweight Windows clipboard manager built with Rust, Tauri, React, and TypeScript. It keeps clipboard history local, categorizes content automatically, and adds optional memos, recycle bin recovery, themes, tray controls, and quick paste workflows.

Chinese display name: `超级剪贴板`.

[Latest Release](https://github.com/Boredlittlenan/SuperClipboard/releases/latest) · [Version Notes](VERSIONS.md) · [Changelog](CHANGELOG.md)

## Download

Download the latest Windows installer from [GitHub Releases](https://github.com/Boredlittlenan/SuperClipboard/releases/latest).

- `SuperClipboard_2.0.1_x64-setup.exe`: recommended Windows installer
- `SuperClipboard_2.0.1_x64_en-US.msi`: MSI package

## Highlights

- Smart categorization for text, links, images, code, emails, and file paths
- Local SQLite history with SHA-256 deduplication and indexed search
- Pin, edit, copy, delete, and restore clipboard entries
- Optional memo module with title, rich body, pasted image preview, tags, pinning, search, and drag sorting
- Optional recycle bin with separate Clipboard and Memos views and 30-day cleanup
- Global shortcut, tray controls, single-instance launch, and auto-start support
- Theme mode switcher with System / Light / Dark and independent accent colors
- First launch follows the system language, with Chinese and English UI available
- Built-in update check through GitHub Releases

## Default Behavior

- Version: `2.0.1`
- Default shortcut: `Alt+X`
- Startup: shows the main window on the desktop and keeps the tray icon available
- Theme mode: follows system
- Theme accent: blue
- Auto-start: enabled
- Always on top: disabled
- Raw preview: disabled for clipboard entries; memos always use formatted preview
- Auto update check: enabled
- Memos and Recycle Bin: disabled by default, available in Settings

## Usage Notes

- Click a clipboard item to copy it back to the system clipboard.
- If the window was opened by the global shortcut, clicking a clipboard item also hides the window and simulates `Ctrl+V` into the previously active app.
- Paste-to-caret is currently timing-sensitive: it depends on the previous app regaining focus after SuperClipboard hides, then sends `Ctrl+V` after a short delay. Some apps or slow focus transitions may make it feel intermittent; this is documented for a future redesign.
- Repeatedly launching the app shortcut focuses the existing instance instead of creating duplicate tray icons.
- When upgrading from `SuperClipboard3`, the old local data directory is migrated automatically.
- Title easter egg: double-click the Settings version badge to restore the default title, triple-click for `小楠の剪贴板`, and five-click for `瑛楠の剪贴板`.

## Privacy

SuperClipboard stores clipboard entries, memos, and settings locally in SQLite under the app data directory. Clipboard content is not uploaded by the app. Update checks contact GitHub Releases when enabled.

## Tech Stack

- Backend: Rust, Tauri v2, SQLite (`rusqlite`), `arboard`
- Frontend: React 19, TypeScript, Vite 8
- Platform target: Windows x64

## Development

```bash
# Install dependencies
pnpm install

# Run in dev mode
pnpm tauri:dev

# Build frontend
pnpm build

# Build Windows installers
pnpm tauri:build
```

## Project Structure

```text
src-tauri/
  src/
    clipboard.rs        # Clipboard monitoring service
    classifier.rs       # Content type classification
    storage.rs          # SQLite storage layer
    autostart.rs        # Windows auto-start registry integration
    window_position.rs  # Default window positioning and work-area clamping
    lib.rs              # Tauri commands and app setup
    main.rs             # Entry point
src/
  components/           # React UI components
  api/                  # Tauri command wrappers
  i18n/                 # Translations and i18n context
  types/                # TypeScript types
```

## Roadmap

- Improve paste-to-caret reliability by tracking the previously active window and confirming focus before sending `Ctrl+V`.
- Add virtual scrolling for very large clipboard histories.
- Revisit caret follow mode and saved window position after the interaction model is stable.
