# Prompt Drawer

![Prompt Drawer demo](docs/prompt-drawer-demo.gif)

**Read this in:** **English** | [简体中文](README.zh-CN.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [العربية](README.ar.md)

Everyone deserves a personal prompt library.

Prompt Drawer is a local prompt library for Codex, Cursor, and CLI. Select a saved prompt and Prompt Drawer fills it into the active input and sends it in one action. No repeated copying, pasting, or pressing Return. Switch to **Insert only** when you want to review before sending.

Create prompt groups to send a sequence of prompts in order.

## Use it

1. Create individual prompts, prompt groups, and categories in your library.
2. Focus the input where you want to work.
3. Open Prompt Drawer and choose a prompt or group.

## Download

Get the latest macOS Apple Silicon DMG or Windows x64 installer from [GitHub Releases](https://github.com/Imd11/prompt-drawer/releases/latest).

macOS needs Accessibility permission to insert and send prompts in supported apps.

## Your prompt library

Prompt Drawer stores your library locally and never uploads prompt content to a server. Import or export JSON libraries whenever you need a backup or want to move your prompts.

Example libraries are available in:

- `examples/prompts/prompts-zh.json`
- `examples/prompts/prompts-en.json`

## Development

```bash
npm install
npm test
npm run tauri -- build
```

## License

MIT
