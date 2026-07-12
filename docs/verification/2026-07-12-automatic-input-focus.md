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
- Claude's sparse Electron tree is resolved with bounded system/application hit testing; the returned `AXTextArea` must belong to the exact captured window before focus is accepted.
- WeChat 4.1.2 exposes neither a focused editable element nor a usable AX hit-test API (`kAXErrorNotImplemented`). Its calibrated profile therefore clicks one versioned point inside the exact captured window, restores the pointer, and uses a paced Command-V sequence. Other WeChat versions fail closed.
- Search, secure, hidden, disabled, zero-size, wrong-process, wrong-window, `AXWebArea`, and ambiguous candidates fail closed.
- Clipboard replacement occurs only after focus succeeds. Paste and submit events are each single-attempt operations.
- Paste-only transactions cannot enter the submit phase.

## Automated Gates

- Previous GitHub Actions run: `29194628631`
- macOS `cargo fmt --check`: passed
- macOS `cargo check`: passed
- macOS `cargo test --lib`: 276 passed, 0 failed, 2 ignored
- macOS `npm test`: 27 files passed, 320 tests passed
- macOS `npm run build`: passed
- Windows `cargo check`: passed
- Windows `npm run build`: passed
- Fixture transaction repetition: covered by the Rust suite above; 100 paste-only and 100 paste-and-submit transactions complete with one paste and at most one submit per transaction.
- `git diff --check`: passed before the final verification-record update.

## Real-App Gate

Read-only probes and non-submitting calibration were run without recording conversation content:

- Claude Desktop 1.18286.0: one `paste_only` production-path trial started with the composer unfocused, resolved the exact `AXTextArea`, pasted the complete marker once, emitted no Return, and the marker was removed afterward.
- WeChat 4.1.2: one direct calibrated-click trial and one full `paste_only` production-path trial pasted the complete marker once into the currently selected chat input, emitted no Return, and the marker was removed afterward. The generic zero-delay Command-V sequence was rejected because it did not paste reliably; the paced sequence passed.

Real Claude/WeChat submit trials are not run without action-time confirmation that a disposable test conversation/account is selected. Until that confirmation and submit matrix are completed, this document does not claim real-app paste-and-submit acceptance.
