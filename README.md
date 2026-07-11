# Prompt Drawer

> Your prompt library for Codex, ready when you are.

![Prompt Drawer demo](docs/prompt-drawer-demo.gif)

**Read this in:** **English** | [简体中文](README.zh-CN.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [العربية](README.ar.md)

Prompt Drawer is a local desktop prompt library built for Codex. It keeps a floating button near the active Codex input, opens a compact prompt picker, and inserts the selected prompt where you are working.

The app is built with Tauri, React, and Rust. Prompt data is stored locally on the user's machine.

## Features

- Floating prompt button with a compact prompt list.
- Local prompt manager for single prompts and grouped prompt sequences.
- Category support for organizing prompt collections.
- Paste-only and paste-and-submit insertion modes.
- Import and export prompt libraries as JSON.
- Optional link-and-sync mode for keeping a chosen JSON file in sync with edits made in the app.
- Local-first storage; prompt data is not uploaded to a server.
- macOS menu bar app packaging with Developer ID signing and notarization.
- Windows installer build through GitHub Actions.

## Download

The latest release is available on GitHub:

https://github.com/Imd11/prompt-drawer/releases/latest

Current packaged builds:

- macOS Apple Silicon DMG
- Windows x64 installer

On macOS, Prompt Drawer requires Accessibility permission to paste into and submit text in other apps.

## Example Prompt Libraries

This repository includes two example prompt libraries:

- `examples/prompts/prompts-zh.json`
- `examples/prompts/prompts-en.json`

They contain a development workflow prompt set with planning, execution, review, debugging, and release prompts.

To use one of them:

1. Open Prompt Drawer.
2. Go to the prompt manager.
3. Click Import.
4. Select one of the JSON files from `examples/prompts/`.
5. Choose whether to import it as the app's internal copy or link and sync the selected JSON file.

Importing as a copy replaces the current internal prompt library, so export your current prompts first if you want a backup. If you choose link and sync, Prompt Drawer stores the selected file path and writes future in-app prompt edits back to that JSON file. The app never scans your Desktop or automatically chooses a prompt file.

## Local Data

Prompt Drawer stores user data locally.

On macOS, prompts are stored at:

```text
~/Library/Application Support/local.promptpicker.dev/prompts.json
```

Settings are stored next to it:

```text
~/Library/Application Support/local.promptpicker.dev/settings.json
```

Exporting prompts creates a separate JSON backup. It does not change the app's default storage location.

When you import a JSON file, Prompt Drawer uses the internal `prompts.json` by default. Link and sync is opt-in per imported file, and it can be removed from the prompt manager without deleting the external JSON file.

## Development

Install dependencies:

```bash
npm install
```

Run the frontend development server:

```bash
npm run dev
```

Run tests:

```bash
npm test
```

Build the frontend:

```bash
npm run build
```

Build the Tauri app:

```bash
npm run tauri -- build
```

## macOS Release Build

The Tauri config is set up for Developer ID signing. For a public macOS release, build, notarize, and staple the DMG:

```bash
npm run tauri -- build --bundles dmg
xcrun notarytool submit "src-tauri/target/release/bundle/dmg/Prompt Drawer_<version>_aarch64.dmg" \
  --key /path/to/AuthKey_<KEY_ID>.p8 \
  --key-id <KEY_ID> \
  --issuer <ISSUER_ID> \
  --wait
xcrun stapler staple "src-tauri/target/release/bundle/dmg/Prompt Drawer_<version>_aarch64.dmg"
xcrun stapler validate "src-tauri/target/release/bundle/dmg/Prompt Drawer_<version>_aarch64.dmg"
```

Verify Gatekeeper acceptance:

```bash
spctl --assess --type open --context context:primary-signature --verbose=4 \
  "src-tauri/target/release/bundle/dmg/Prompt Drawer_<version>_aarch64.dmg"
```

## Windows Release Build

The repository includes a GitHub Actions workflow:

```text
.github/workflows/build-windows.yml
```

Run it from GitHub Actions to produce the Windows NSIS installer artifact.

## Tech Stack

- Tauri 2
- Rust 2021
- React 19
- TypeScript
- Vite
- Vitest

## License

MIT
