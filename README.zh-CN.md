# Sleepy Cat

**你的个人提示词库。**

## 演示

预览会自动循环播放。点击任意预览可打开高清原视频。

| Codex App | Codex 提示词组 |
| :---: | :---: |
| [![Codex App 演示](docs/previews/codex-app-demo.webp)](docs/codex-app-demo.mp4) | [![Codex 提示词组演示](docs/previews/codex-prompt-group-demo.webp)](docs/codex-prompt-group-demo.mp4) |
| **Cursor App** | **Claude App** |
| [![Cursor App 演示](docs/previews/cursor-app-demo.webp)](docs/cursor-app-demo.mp4) | [![Claude App 演示](docs/previews/claude-app-demo.webp)](docs/claude-app-demo.mp4) |
| **ChatGPT 网页版** | **Gemini 网页版** |
| [![ChatGPT 网页版演示](docs/previews/chatgpt-web-demo.webp)](docs/chatgpt-web-demo.mp4) | [![Gemini 网页版演示](docs/previews/gemini-web-demo.webp)](docs/gemini-web-demo.mp4) |
| **CLI** | |
| [![CLI 演示](docs/previews/cli-demo.webp)](docs/cli-demo.mp4) | |

**阅读语言：** [English](README.md) | **简体中文** | [हिन्दी](README.hi.md) | [Español](README.es.md) | [العربية](README.ar.md)

每个人都值得拥有一个属于自己的提示词库。

Sleepy Cat 是一个适用于桌面应用、终端和浏览器 AI 工具的本地提示词库，支持 Codex、Cursor、Claude、ChatGPT、Gemini 等常用工具。直接从自己的提示词库中选择一条提示词，Sleepy Cat 就能自动填入当前输入框并发送，无需反复复制、粘贴和按回车。需要先检查内容时，也可以切换为“只填入”。

你还可以创建提示词组，按顺序连续发送多条提示词。

## 使用方式

1. 在提示词库中创建单条提示词、提示词组和分类。
2. 将焦点放在需要工作的输入框中。
3. 打开 Sleepy Cat，选择一条提示词或提示词组。

## 下载

在 [GitHub Releases](https://github.com/Imd11/sleepy-cat/releases/latest) 下载最新的 macOS Apple Silicon DMG 或 Windows x64 安装包。

macOS 需要授予辅助功能权限，才能向已支持的应用填入并发送提示词。

## 你的提示词库

Sleepy Cat 在本地保存提示词库，不会把提示词内容上传到服务器。你可以随时导入或导出 JSON 提示词库，用于备份或迁移。

新用户首次打开时，会自动加载内置的中文提示词库，包含 4 个分类、50 个条目（含提示词组）。你可以自由新增、编辑、删除这些内容，或导入自己的 JSON 提示词库。后续启动和升级均保留个人提示词库；主动清空后也不会重新填充内置内容。已有用户的提示词和关联文件设置不会被覆盖。

仓库提供两份示例提示词库：

- `examples/prompts/prompts-zh.json`
- `examples/prompts/prompts-en.json`

## 开发

```bash
npm install
npm test
npm run tauri -- build
```

## 许可证

MIT
