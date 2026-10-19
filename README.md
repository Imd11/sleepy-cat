# Sleepy Cat

**Your personal prompt library.**

## Demos

Previews play automatically. Click one to open the full-resolution video.

| Codex App | Codex Prompt Groups |
| :---: | :---: |
| [![Codex App demo](docs/previews/codex-app-demo.webp)](docs/codex-app-demo.mp4) | [![Codex prompt groups demo](docs/previews/codex-prompt-group-demo.webp)](docs/codex-prompt-group-demo.mp4) |
| **Cursor App** | **Claude App** |
| [![Cursor App demo](docs/previews/cursor-app-demo.webp)](docs/cursor-app-demo.mp4) | [![Claude App demo](docs/previews/claude-app-demo.webp)](docs/claude-app-demo.mp4) |
| **ChatGPT Web** | **Gemini Web** |
| [![ChatGPT web demo](docs/previews/chatgpt-web-demo.webp)](docs/chatgpt-web-demo.mp4) | [![Gemini web demo](docs/previews/gemini-web-demo.webp)](docs/gemini-web-demo.mp4) |
| **CLI** | |
| [![CLI demo](docs/previews/cli-demo.webp)](docs/cli-demo.mp4) | |

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

On first launch, new users start with a bundled Chinese prompt library containing 4 categories and 50 entries (including prompt groups). You can add, edit, or delete any entry, or import your own JSON library. Later launches and upgrades preserve your personal library, including an intentionally emptied library. Existing users' prompts and linked-file settings are never replaced.

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
