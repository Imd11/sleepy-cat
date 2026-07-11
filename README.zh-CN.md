# Prompt Drawer

> 为 Codex 准备的提示词库，随时可用。

**阅读语言：** [English](README.md) | **简体中文** | [हिन्दी](README.hi.md) | [Español](README.es.md) | [العربية](README.ar.md)

Prompt Drawer 是一个为 Codex 打造的本地桌面提示词库。它会在 Codex 当前输入框附近显示悬浮按钮，打开紧凑的提示词面板，并将选中的提示词填入正在使用的位置。

这个应用使用 Tauri、React 和 Rust 构建。提示词数据存储在用户自己的电脑本地。

## 功能

- 悬浮提示词按钮和紧凑提示词列表。
- 本地提示词管理器，支持单条提示词和分组提示词序列。
- 分类功能，用于整理不同提示词集合。
- 支持“只粘贴”和“粘贴并发送”两种插入模式。
- 支持用 JSON 导入和导出提示词库。
- 可选的“链接并同步”模式，可以让选中的 JSON 文件和 App 内编辑保持同步。
- 本地优先存储；提示词数据不会上传到服务器。
- macOS 菜单栏应用打包，支持 Developer ID 签名和公证。
- 通过 GitHub Actions 构建 Windows 安装包。

## 下载

最新版本可以在 GitHub Release 下载：

https://github.com/Imd11/prompt-drawer/releases/latest

当前提供的打包版本：

- macOS Apple Silicon DMG
- Windows x64 安装包

在 macOS 上，Prompt Drawer 需要辅助功能权限，才能向其他应用粘贴文本并执行发送操作。

## 示例提示词库

这个仓库包含两个示例提示词库：

- `examples/prompts/prompts-zh.json`
- `examples/prompts/prompts-en.json`

它们包含一套开发工作流提示词，覆盖计划、执行、审查、调试和发布。

使用方式：

1. 打开 Prompt Drawer。
2. 进入提示词管理器。
3. 点击 Import。
4. 从 `examples/prompts/` 中选择一个 JSON 文件。
5. 选择导入为 App 内部副本，或勾选“链接并同步这个文件”。

以副本方式导入会替换当前 App 内部提示词库。如果你想保留现有提示词，请先导出备份。如果选择链接并同步，Prompt Drawer 会记录这个文件路径，之后你在 App 内修改提示词时会写回这个 JSON 文件。App 不会扫描桌面，也不会自动选择某个提示词文件。

## 本地数据

Prompt Drawer 会在本地存储用户数据。

在 macOS 上，提示词存储在：

```text
~/Library/Application Support/local.promptpicker.dev/prompts.json
```

设置文件存储在同一目录：

```text
~/Library/Application Support/local.promptpicker.dev/settings.json
```

导出提示词会创建一个独立的 JSON 备份文件，不会改变应用默认的数据存储位置。

导入 JSON 文件时，Prompt Drawer 默认使用内部 `prompts.json`。链接并同步是针对某个导入文件的主动选择，也可以在提示词管理器里取消；取消同步不会删除外部 JSON 文件。

## 开发

安装依赖：

```bash
npm install
```

启动前端开发服务器：

```bash
npm run dev
```

运行测试：

```bash
npm test
```

构建前端：

```bash
npm run build
```

构建 Tauri 应用：

```bash
npm run tauri -- build
```

## macOS 发布构建

Tauri 配置已经支持 Developer ID 签名。公开发布 macOS 版本时，需要构建、公证并 staple DMG：

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

验证 Gatekeeper 是否接受：

```bash
spctl --assess --type open --context context:primary-signature --verbose=4 \
  "src-tauri/target/release/bundle/dmg/Prompt Drawer_<version>_aarch64.dmg"
```

## Windows 发布构建

仓库包含一个 GitHub Actions 工作流：

```text
.github/workflows/build-windows.yml
```

在 GitHub Actions 中运行它，可以生成 Windows NSIS 安装包 artifact。

## 技术栈

- Tauri 2
- Rust 2021
- React 19
- TypeScript
- Vite
- Vitest

## 许可证

MIT
