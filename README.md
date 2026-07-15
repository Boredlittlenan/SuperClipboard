# SuperClipboard

[中文文档](README.zh-CN.md)

SuperClipboard is a lightweight Windows clipboard manager built with Rust, Tauri, React, and TypeScript. It keeps clipboard history local, categorizes content automatically, and adds optional memos, recycle bin recovery, themes, tray controls, and quick paste workflows.

Chinese display name: `超级剪贴板`.

[Website](https://boredlittlenan.github.io/SuperClipboard/) · [Latest Release](https://github.com/Boredlittlenan/SuperClipboard/releases/latest) · [Version Notes](VERSIONS.md) · [Changelog](CHANGELOG.md)

## Built with OpenAI Codex and GPT-5.6

SuperClipboard 3.x is a human-directed rebuild developed in close collaboration with **OpenAI Codex and GPT-5.6**. An earlier C# prototype established the original idea, but limited experience and development time kept it from reaching the intended quality. Codex made it possible to revisit that idea with a more efficient Rust and Tauri architecture, even though Rust was new to the creator when the rebuild began.

- **Architecture and planning:** Codex and GPT-5.6 helped translate product requirements into the Rust/Tauri backend, React frontend, unified local/external storage boundary, and versioned database migration design.
- **Implementation:** They assisted with Rust, React, TypeScript, SQLite, and PostgreSQL code, including clipboard monitoring, memo editing, classification, search indexing, connection management, and bilingual UI work.
- **Debugging and optimization:** They were used to investigate Windows window and tray behavior, WebView rendering differences, remote-storage latency, concurrency issues, and large Base64 image handling.
- **Quality and delivery:** They helped create regression tests, run lint/build/Clippy checks, maintain documentation, prepare installers, and verify GitHub Releases and the project website.

The product vision, requirements, interface decisions, hands-on testing, and final approval remained human-led throughout the project. Codex and GPT-5.6 were development collaborators, not runtime dependencies: SuperClipboard does not call OpenAI services or send clipboard and memo content to OpenAI while the application is running.

## Download

Download the latest Windows installer from [GitHub Releases](https://github.com/Boredlittlenan/SuperClipboard/releases/latest).

- `SuperClipboard_3.3.0_x64-setup.exe`: recommended Windows installer
- `SuperClipboard_3.3.0_x64_en-US.msi`: MSI package

## Highlights

- Smart categorization for text, links, images, code, emails, and file paths
- Local SQLite history with SHA-256 deduplication and a compact search index that excludes image Base64 payloads
- Pin, edit, copy, delete, and restore clipboard entries
- Optional memo module with title, rich body, pasted image preview, tags, pinning, search, and drag sorting
- Optional recycle bin with separate Clipboard and Memos views and 30-day cleanup
- Global shortcut, tray controls, single-instance launch, and auto-start support
- Theme mode switcher with System / Light / Dark and independent accent colors
- Storage settings with Local / External PostgreSQL modes, fast switching among saved connections, and `.scbackup` local backup/restore tools
- External PostgreSQL operations use a bounded connection pool and background blocking tasks to keep remote search, counts, and list switching responsive
- Local and external searches use the same readable-text index, so memo text and image metadata remain searchable without scanning embedded image data
- Experimental features panel for optional Modern UI, clipboard multi-tag display, and color-strip hiding
- First launch follows the system language, with Chinese and English UI available
- Built-in update check through GitHub Releases with release notes preview

## System Support

Windows x64 is supported now, with NSIS setup and MSI packages.

## Default Behavior

- Version: `3.3.0`
- Default shortcut: `Alt+X`
- Startup: positions the main window before showing it and keeps the tray icon available
- UI style: classic UI by default, with Modern UI available in Experimental Features
- Theme mode: follows system
- Theme accent: blue
- Auto-start: enabled
- Always on top: disabled
- Raw preview: disabled for clipboard entries; memos always use formatted preview
- Auto update check: enabled
- Memos and Recycle Bin: disabled by default, available in Settings

## Usage Notes

### Set a Shortcut

Open Settings, click the shortcut button, then press the desired key combination. `Alt`, `Ctrl`, `Shift`, and `Win` modifiers are supported. Clicking the recording button again cancels the edit; entering the same shortcut again still saves and re-registers it.

### Use Clipboard History

Copied text, links, images, code, emails, and paths are added to the list automatically. Clicking an item copies it back to the system clipboard. If the window was opened by the global shortcut, clicking an item also tries to hide SuperClipboard and send `Ctrl+V` to the previously active app.

When the window is visible but covered by another app, pressing the global shortcut brings it back to the front. The shortcut only hides the window when SuperClipboard is already the foreground window.

After an entry is edited, clicking its visible content copies the latest saved version. Expand Original to inspect the first captured content; clicking that original-content area copies the original version without overwriting it.

### Raw Preview

Raw Preview only affects clipboard entries. Enable it when you want text to preserve original line breaks and spacing, which is useful for code, logs, and config snippets. Disable it for a denser list. Memos always use formatted preview and are not affected by this switch.

### Configure External Storage

The storage entry is always available next to Settings. The panel supports two modes:

- Local: writes data to the local SQLite database and is the default mode.
- External: saves clipboard and memo bodies to a user-provided PostgreSQL database. Saving automatically tests the connection and initializes remote tables.

External databases that connect and switch successfully appear under Saved Connections, with up to 12 profiles retained. Select a profile to refill the form or choose Use to test and switch immediately. Deleting a saved connection removes only the local profile; it never deletes the external database or its data.

Remote clients listen for PostgreSQL change notifications and refresh the active list after a short debounce, so changes made from another device appear without switching tabs. When two devices edit the same clipboard entry or memo, the later save is rejected if its displayed version is stale; the app refreshes the latest content instead of silently overwriting it.

Backup / Restore is shown only in Local mode. Backups use the `.scbackup` package format with source version, data manifest, and checksum metadata. Cross-version restore is not guaranteed.

### Experimental Features

Enable Experimental Features in Settings to show the lab button next to Storage Settings. Experimental options are off by default.

- Modern UI: switches from the classic compact interface to the refreshed glass-style visual system, including a sliding active-tab indicator.
- Clipboard Multi-tag Display: shows every detected category tag on clipboard entries and uses a segmented category color bar.
- Hide Entry Color Strip: hides the left category strip on clipboard entries and the original-content left border.
- Multicolor Mode (Tab Labels): uses the matching category color for a selected clipboard Tab. All, Archive, and Memos retain their existing theme or memo colors.

### Classification and Tags

Clipboard entries store a primary `category` and a `category_tags` list. Category tabs and counts can match entries through the tag list, while the primary category remains the fallback style and compatibility anchor.

Multi-tag display is intentionally behind Experimental Features. When it is off, clipboard entries show only the primary category for a calmer list. When it is on, mixed content such as links plus email addresses can show multiple category badges.

## Privacy

By default, SuperClipboard stores clipboard entries, memos, and settings locally in SQLite under the app data directory. When External mode is selected in Storage Settings, clipboard and memo bodies are written to the user-configured PostgreSQL database instead. Saved external connection profiles and credentials remain in the local settings database and are not written to remote data tables; deleting a profile does not delete remote data. Update checks contact GitHub Releases when enabled.

## License

SuperClipboard is source-available for non-commercial use only. Commercial use is not permitted without explicit written permission from the copyright holder. See [LICENSE.md](LICENSE.md).

## Tech Stack

- Backend: Rust, Tauri v2, SQLite (`rusqlite`), PostgreSQL with `r2d2`, `LISTEN / NOTIFY`, `arboard`
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

# Run frontend tests
pnpm test

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
    remote_storage.rs   # External PostgreSQL storage layer
    search_index.rs     # Readable search text extraction without image Base64
    storage_backend.rs  # Unified local / external storage dispatch
    commands/           # Clipboard and memo Tauri commands
    memo_tags.rs        # Persisted memo auto-tag inference
    update.rs           # GitHub release checks and semantic version comparison
    autostart.rs        # Windows auto-start registry integration
    window_position.rs  # Default window positioning and work-area clamping
    lib.rs              # App lifecycle and cross-domain coordination
    main.rs             # Entry point
src/
  components/           # React UI components
  api/                  # Tauri command wrappers
  hooks/                # Shared settings, pagination, menu, and reorder hooks
  test/                 # Frontend test setup
  i18n/                 # Translations and i18n context
  settings/             # Typed settings schema and context
  storage/              # External storage profile utilities
  types/                # TypeScript types
```
