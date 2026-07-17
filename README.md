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

- `SuperClipboard_<version>_x64-setup.exe`: recommended Windows installer
- `SuperClipboard_<version>_x64_en-US.msi`: MSI package

## Highlights

- Smart categorization for text, links, images, code, emails, and file paths
- Drop text, links, images, or image files onto the main window to add them to the system clipboard and history
- Local SQLite history with SHA-256 deduplication and a compact search index that excludes image Base64 payloads
- Edited clipboard content participates in deduplication, so copying a modified entry does not create a duplicate
- Pin, edit, copy, delete, restore, preview, and export clipboard entries; images can open in an in-app preview or save as PNG
- Optional memo module with title, rich body, pasted image preview, tags, pinning, search, and drag sorting
- Optional recycle bin with separate Clipboard and Memos views, 30-day cleanup, and confirmed per-view emptying
- Global shortcut, tray controls, single-instance launch, and auto-start support
- Theme mode switcher with System / Light / Dark and independent accent colors
- Storage settings with Local / External PostgreSQL modes, fast switching among saved connections, and `.scbackup` local backup/restore tools
- External PostgreSQL operations use a bounded connection pool and background blocking tasks to keep remote search, counts, and list switching responsive
- Local and external searches use the same readable-text index, so memo text and image metadata remain searchable without scanning embedded image data
- Experimental features panel for optional Modern UI, multi-select workflows, clipboard multi-tag display, and color-strip hiding
- First launch follows the system language, with Chinese and English UI available
- Built-in update check through GitHub Releases with release notes preview and a public-page fallback when the GitHub API is rate-limited

## System Support

Windows x64 is supported now, with NSIS setup and MSI packages.

## Default Behavior

- Version: `3.5.0` (2026-07-17)
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

You can also drag text, links, images, or PNG/JPEG/GIF/WebP files onto the main window. SuperClipboard writes the dropped content to the system clipboard, then uses the same classification and deduplication flow as a normal copy. Dropping into a memo editor keeps its normal in-editor image insertion behavior.

When the window is visible but covered by another app, pressing the global shortcut brings it back to the front. The shortcut only hides the window when SuperClipboard is already the foreground window.

After an entry is edited, clicking its visible content copies the latest saved version. The first captured version and the latest edit both participate in deduplication, so copying either one does not create a redundant history entry. Expand Original to inspect the first captured content; clicking that original-content area copies the original version without overwriting it.

Enable Multi-select Mode in Experimental Features to show Select Entries. You can also Ctrl+click entries to start selecting and press Delete to open the batch-delete confirmation. Mixed categories can be deleted together. Merging requires 2 to 20 entries with the same primary category and always asks whether to keep or remove the originals; removal follows the current Recycle Bin setting.

Clear History keeps its existing confirmation on the All tab. When used from a category tab, the dialog lets you clear only the current category or all non-pinned clipboard history. The dialog also states the actual outcome: entries move to Recycle Bin and remain recoverable when it is enabled, or are permanently deleted when it is disabled. With Clipboard Multi-tag Display enabled, auxiliary category matches are included in the current-tab scope.

Recycle Bin keeps Clipboard and Memos in separate views. Empty permanently removes every record in the currently visible view after confirmation; it does not affect the other view. Permanent deletion physically removes the selected body data in both Local SQLite and External PostgreSQL modes. Clear History only acts on active history and does not silently remove records already in Recycle Bin.

Image entries show Preview and Save Image actions on hover. Preview opens inside SuperClipboard; Save Image writes a PNG file to a location you choose.

### Raw Preview

Raw Preview only affects clipboard entries. Enable it when you want text to preserve original line breaks and spacing, which is useful for code, logs, and config snippets. Disable it for a denser list. Memos always use formatted preview and are not affected by this switch.

### Configure External Storage

The storage entry is always available next to Settings. The panel supports two modes:

- Local: writes data to the local SQLite database and is the default mode.
- External: saves clipboard and memo bodies to a user-provided PostgreSQL database. Saving automatically tests the connection and initializes remote tables.

External databases that connect and switch successfully appear under Saved Connections, with up to 12 profiles retained. Select a profile to refill the form or choose Use to test and switch immediately. Deleting a saved connection removes only the local profile; it never deletes the external database or its data.

Remote clients listen for PostgreSQL change notifications and refresh the active list after a short debounce, so changes made from another device appear without switching tabs. The notification channel does not maintain a second event log. When two devices edit the same clipboard entry or memo, the later save is rejected if its displayed version is stale; the app refreshes the latest content instead of silently overwriting it.

Backup / Restore is shown only in Local mode. Backups use the `.scbackup` package format with source version, data manifest, and checksum metadata. Cross-version restore is not guaranteed.

### Experimental Features

Enable Experimental Features in Settings to show the lab button next to Storage Settings. Experimental options are off by default.

- Modern UI: switches from the classic compact interface to the refreshed glass-style visual system, including a sliding active-tab indicator.
- Multi-select Mode: shows Select Entries and enables Ctrl+click selection plus the Delete shortcut for confirmed batch deletion.
- Clipboard Multi-tag Display: shows every detected category tag, uses a segmented category color bar, and lets auxiliary tags participate in category tabs and counts.
- Hide Entry Color Strip: hides the left category strip on clipboard entries and the original-content left border.
- Multicolor Mode (Tab Labels): uses the matching category color for a selected clipboard Tab. All, Archive, and Memos retain their existing theme or memo colors.
- Update History Categories: manually applies the current classification rules to existing entries in the active local or external storage. The panel shows the active rules version and whether history has been updated with it. The operation is recommended after an app or rule update and always requires confirmation.

### Classification and Tags

Clipboard entries store a primary `category` and a `category_tags` list. With Clipboard Multi-tag Display disabled, category tabs and counts use only the primary category. Enabling it also lets auxiliary tags participate, so one mixed-content entry can appear in multiple matching tabs.

Multi-tag display is intentionally behind Experimental Features. Switching it refreshes both the visible clipboard list and category counts without rewriting stored entries. Classification changes apply automatically only to newly captured or edited content; SuperClipboard never reclassifies old data during startup or a storage connection. Use Update History Categories explicitly when needed. The operation skips images and changes only category metadata plus the search index. Applied rules versions are tracked per active storage: in local SQLite settings or in the selected PostgreSQL database, so switching external connections cannot reuse another database's status.

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
    storage_migrations.rs # SQLite schema and version migrations
    remote_storage.rs   # External PostgreSQL storage layer
    remote_migrations.rs # PostgreSQL schema and version migrations
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
  hooks/                # Settings, clipboard/archive paging, menu, and reorder hooks
  test/                 # Frontend test setup
  i18n/                 # Translations and i18n context
  settings/             # Typed settings schema and context
  storage/              # External storage profile utilities
  types/                # TypeScript types
.github/workflows/
  ci.yml                # Frontend/Rust checks and disposable PostgreSQL integration test
```
