# Automatic Input Focus Verification

## Environment

- macOS: 26.5
- Prompt Drawer: 1.0.11 development build
- Codex: installed `com.openai.codex`; version not exposed by the local metadata query
- Claude Desktop: 1.18286.0
- WeChat: 4.1.2

No prompt text, conversation text, clipboard content, or raw Accessibility values are recorded here.

## Structural Evidence

- Codex remains on the captured-application first-responder route and does not require an AX window.
- Claude uses its main application AX tree and enables manual accessibility only after a sparse-tree result.
- WeChat accepts the main process plus a `WeChatAppEx` process only when bundle path and ancestry match the captured WeChat application; candidate geometry must remain inside the captured window.
- Search, secure, hidden, disabled, zero-size, wrong-process, wrong-window, `AXWebArea`, and ambiguous candidates fail closed.
- Clipboard replacement occurs only after focus succeeds. Paste and submit events are each single-attempt operations.
- Paste-only transactions cannot enter the submit phase.

## Automated Gates

The final command results are recorded during Task 14 after formatting, Rust, frontend, Windows-target, fixture-repetition, and diff checks finish.

## Real-App Gate

Read-only probes were run without recording content. Real Claude/WeChat submit trials are intentionally not run without explicit confirmation that a disposable test conversation/account is selected. Until that approval and matrix are completed, this document does not claim real-app paste-and-submit acceptance.
