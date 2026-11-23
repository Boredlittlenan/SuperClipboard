# SuperClipboard 4.0

[简体中文](README.zh-CN.md) · [4.0 design and legacy audit](docs/4.0-design.md) · [Changelog](CHANGELOG.md) · [GitHub Releases](https://github.com/Boredlittlenan/SuperClipboard/releases)

SuperClipboard is a Windows workspace for clipboard history, memos, favorites, reminders, and Trash.

## Layouts

Version 4 opens in the compact classic layout by default. Enable **Experimental Features** in Settings and turn on **Workspace mode (beta)** from the flask menu. In the workspace, turn it off under **Settings → Experimental Features**. The selection is saved across launches, and both layouts share the same data.

The classic layout provides import and backup through its storage button and memo reminders through the bell on each memo.

## Use SuperClipboard

- Clipboard history captures text and images and supports text or image drops. Browse by category, find content with `Ctrl+F`, and open an item to see its full content and actions. Double-click to copy. Select multiple entries to merge items of the same category or move them to Trash.
- Use `Alt+X` from another app to open quick paste. Search and press Enter or select an item and click Paste. If Windows cannot restore focus, the content remains on the system clipboard.
- Create a memo with text, images, and tags. Star items to collect them in Favorites, and recover deleted items from Trash.
- Set a memo reminder for a specific local date and time. Choose once, daily, or weekly. A Windows scheduled task checks after the app exits and on sign-in. Missed repeating reminders deliver once before the schedule resumes. Use Settings to test toast delivery.
- Closing the window hides it to the tray. Use the tray menu to exit. Settings controls capture, the startup option, the global shortcut, language, and appearance.

## Upgrade and back up your data

Open **Storage Settings** in the classic layout or **Storage & import** in the workspace to preview a detected older SQLite database, a legacy JSON backup, or a `.scbackup` package. To import from PostgreSQL, connect to that server and preview its legacy schema. Review the counts and sample, then merge the data. The operation keeps the source and existing records intact, skips duplicates, and rolls back on errors.

SQLite and PostgreSQL each expose a separate workspace. Version 4 stores local data in `workspace.db` alongside the legacy database. The PostgreSQL schema is `superclipboard_v4`. Export from one workspace and import into another to transfer data. Backup exports contain clipboard and memo records, including embedded images. Connection credentials are protected with Windows DPAPI and kept local. Reminders use this Windows user's schedule and local device data.

## Develop

Install WebView2, MSVC C++ build tools, Node.js 22+, pnpm 10, and stable Rust. Run:

```powershell
pnpm install --frozen-lockfile
pnpm tauri:dev
pnpm lint
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --locked
pnpm tauri:build
```

See the [4.0 design and legacy audit](docs/4.0-design.md) for the product and storage decisions.

[License](LICENSE.md)
