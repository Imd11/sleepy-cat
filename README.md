# Sleepy Cat

**Your personal prompt library.**

![Sleepy Cat demo](docs/sleepy-cat-demo.gif)

**Read this in:** **English** | [简体中文](README.zh-CN.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [العربية](README.ar.md)

Everyone deserves a personal prompt library.

Sleepy Cat is a local prompt library for desktop apps, terminals, and browser-based AI tools, including Codex, Cursor, Claude, ChatGPT, Gemini, and more. Select a saved prompt and Sleepy Cat fills it into the active input and sends it in one action. No repeated copying, pasting, or pressing Return. Switch to **Insert only** when you want to review before sending.

Create prompt groups to send a sequence of prompts in order.

## Use it

1. Create individual prompts, prompt groups, and categories in your library.
2. Focus the input where you want to work.
3. Open Sleepy Cat and choose a prompt or group.

## Download

Get the latest macOS Apple Silicon DMG or Windows x64 installer from [GitHub Releases](https://github.com/Imd11/sleepy-cat/releases/latest).

macOS needs Accessibility permission to insert and send prompts in supported apps.

## Your prompt library

Sleepy Cat stores your library locally and never uploads prompt content to a server. Import or export JSON libraries whenever you need a backup or want to move your prompts.

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
