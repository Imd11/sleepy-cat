# Always Visible Floating Button Repair V2 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Finish the always-visible floating button repair by fixing the remaining real runtime failures: macOS target detection, startup visibility, persistent right-click hide/show controls, main-window lifecycle, and verification warnings.

**Architecture:** Keep the current Tauri/React three-window model. `main` is the client management window, `prompt-button` is the lightweight always-visible overlay, and `prompt-popover` is the only UI surface for quick prompts and button controls. Do not introduce a second right-click UI inside `overlay.html`; the overlay button must delegate left/right actions to Tauri commands that open the correct React popover mode.

**Tech Stack:** Tauri 2, React 19, TypeScript, Vitest, Rust 2021, macOS AppKit non-activating panels, macOS Accessibility/System Events/lsappinfo as already used by the project.

---

## Current Failure Summary

This is the second repair pass. The previous repair improved test/build coverage but still failed the product contract in important ways:

- `frontmost_app_info()` parses `lsappinfo front` incorrectly for the real output `ASN:0x0-...:`.
- `get_focused_input_element()` returns 4 pipe-separated fields but the parser expects at least 5 and reads from `parts[1]`, so `current_input_target()` can still fail in real use.
- `public/overlay.html` implements its own right-click HTML menu instead of opening `prompt-popover` in `button-controls` mode.
- Right-click `Hide Prompt Button` calls only `hide_prompt_button`; it does not persist `floatingButton.visible=false`.
- React `button-controls` mode exists but the real button never opens it.
- React `button-controls` `Hide Button` does not call `hidePromptButton()`.
- Tauri `setup` does not call `show_prompt_button(960, 700, ...)` on startup.
- Tauri `setup` does not intercept main window close to hide instead of terminating the app.
- `package.json` has a duplicate `preview` key.
- `src/App.tsx` still dynamically imports `platformApi`, causing a Vite warning that the repair plan explicitly required removing.
- `cargo test` still emits a dead-code warning for `choose_main_input`.

Do not repeat the previous mistake of making tests pass while leaving the actual runtime path broken.

---

### Task 1: Fix macOS Frontmost App and Input Target Detection

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`
- Modify tests if needed: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`

**Step 1: Add tests for actual `lsappinfo front` output**

In the existing `#[cfg(test)] mod tests` in `src-tauri/src/platform/macos.rs`, add parser tests around helper functions. If there is no helper yet, create pure helper functions first:

```rust
fn parse_front_asn(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.starts_with("ASN:") {
        return Some(trimmed.trim_end_matches(':').to_string());
    }
    None
}
```

Test:

```rust
#[test]
fn parses_lsappinfo_front_asn_output() {
    assert_eq!(
        parse_front_asn("ASN:0x0-0x46046:\n").as_deref(),
        Some("ASN:0x0-0x46046")
    );
}
```

Also keep compatibility with any existing parser if prior code used direct app IDs.

**Step 2: Add tests for System Events output parsing**

The AppleScript currently returns:

```text
window_x,window_y|window_w,window_h|elem_x,elem_y|elem_w,elem_h
```

Add a pure parser:

```rust
fn parse_focused_input_output(raw: &str, app: FrontmostApp) -> Option<InputTarget> {
    let parts: Vec<&str> = raw.trim().split('|').collect();
    if parts.len() != 4 {
        return None;
    }
    // parse parts[0], parts[1], parts[2], parts[3]
}
```

Test:

```rust
#[test]
fn parses_focused_input_output() {
    let app = FrontmostApp {
        name: "Codex".to_string(),
        bundle_id: "com.openai.codex".to_string(),
    };
    let target = parse_focused_input_output("10,20|1200,800|700,680|500,96", app).unwrap();

    assert_eq!(target.window_frame.x, 10.0);
    assert_eq!(target.window_frame.width, 1200.0);
    assert_eq!(target.frame.x, 700.0);
    assert_eq!(target.frame.height, 96.0);
    assert_eq!(target.button_position, (1200.0, 776.0));
}
```

**Step 3: Run the failing Rust tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test platform::macos
```

Expected: FAIL until implementation is updated.

**Step 4: Fix `frontmost_app_info()`**

Replace the current pid parser. The real command flow should be:

```rust
let front = Command::new("lsappinfo").arg("front").output().ok()?;
let asn = parse_front_asn(String::from_utf8_lossy(&front.stdout).as_ref())?;

let info = Command::new("lsappinfo")
    .args(["info", &asn])
    .output()
    .ok()?;
```

Then parse name, bundle id, and pid from the `info` output. Use robust helpers that support these common fields:

- `bundleID="com.openai.codex"`
- `bundleID = "com.openai.codex"`
- `CFBundleIdentifier = "com.openai.codex"`
- first quoted app name from the first line
- `pid = 12345`

Do not rely on `lsappinfo info -app <pid>` unless you have verified it works locally.

**Step 5: Fix focused input parser**

Update `get_focused_input_element()` to call `parse_focused_input_output(trimmed, app)` and parse exactly 4 fields:

```rust
let parts: Vec<&str> = trimmed.split('|').collect();
if parts.len() != 4 {
    return None;
}
let window_pos = parse_xy(parts[0])?;
let window_size = parse_xy(parts[1])?;
let elem_pos = parse_xy(parts[2])?;
let elem_size = parse_xy(parts[3])?;
```

**Step 6: Run Rust tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test
```

Expected: PASS.

**Step 7: Commit**

```bash
git add src-tauri/src/platform/macos.rs
git commit -m "fix: parse macos frontmost app and focused input target"
```

---

### Task 2: Replace Overlay HTML Right-Click Menu with Tauri Popover Controls

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/public/overlay.html`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/windows.rs`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`
- Create or modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/overlayHtml.test.ts`

**Step 1: Add failing overlay HTML test**

Create `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/overlayHtml.test.ts`:

```ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

describe("overlay button html", () => {
  it("opens Tauri button controls on right click instead of an inline menu", () => {
    const html = readFileSync("public/overlay.html", "utf8");

    expect(html).toContain("contextmenu");
    expect(html).toContain("show_prompt_button_controls_from_button");
    expect(html).not.toContain("id=\"menu\"");
    expect(html).not.toContain("hide_prompt_button");
  });
});
```

**Step 2: Run the failing test**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- src/overlay/overlayHtml.test.ts
```

Expected: FAIL because current `overlay.html` contains inline menu and calls `hide_prompt_button`.

**Step 3: Add Tauri command for right-click controls**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/windows.rs`, add a shared helper:

```rust
const BUTTON_WIDTH: f64 = 112.0;
const BUTTON_HEIGHT: f64 = 40.0;
const POPOVER_WIDTH: f64 = 280.0;
const POPOVER_HEIGHT: f64 = 240.0;
const POPOVER_GAP: f64 = 8.0;

fn button_relative_popover_position(app: &tauri::AppHandle) -> (f64, f64) {
    app.get_webview_window(BUTTON_WINDOW_LABEL)
        .and_then(|window| {
            let position = window.outer_position().ok()?;
            let scale = window.scale_factor().unwrap_or(1.0);
            Some((
                position.x as f64 / scale + BUTTON_WIDTH + POPOVER_GAP,
                position.y as f64 / scale,
            ))
        })
        .unwrap_or((100.0, 100.0))
}
```

Add:

```rust
#[tauri::command]
pub fn show_prompt_button_controls_from_button(app: tauri::AppHandle) -> Result<(), String> {
    let (x, y) = button_relative_popover_position(&app);
    show_popover_mode(x, y, "button-controls", app)
}
```

If `show_popover_mode` does not exist, refactor `show_prompt_popover_from_button()` so both left and right click use:

```rust
fn show_popover_mode(x: f64, y: f64, mode: &str, app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(POPOVER_WINDOW_LABEL) {
        window.close().map_err(|e| e.to_string())?;
    }

    let url = format!("index.html?mode={mode}");
    let window = WebviewWindowBuilder::new(&app, POPOVER_WINDOW_LABEL, WebviewUrl::App(url.into()))
        .title("Prompt Picker")
        .inner_size(POPOVER_WIDTH, POPOVER_HEIGHT)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .position(x, y)
        .build()
        .map_err(|e| e.to_string())?;
    crate::macos_panels::configure_non_activating_panel(&window)?;
    Ok(())
}
```

Left click must open `mode=popover`; right click must open `mode=button-controls`.

**Step 4: Register the command**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`:

- add `show_prompt_button_controls_from_button` to `pub use windows::{ ... }`
- add it to `tauri::generate_handler![ ... ]`

**Step 5: Replace `overlay.html` right-click UI**

Remove inline menu markup:

```html
<div id="menu">...</div>
```

Remove `mi-hide` / `mi-open` handlers.

Use:

```js
let contextMenuOpened = false;

btn.addEventListener('contextmenu', async (event) => {
  event.preventDefault();
  contextMenuOpened = true;
  if (!tauri?.core?.invoke) return;
  await tauri.core.invoke('show_prompt_button_controls_from_button');
});
```

At the start of `pointerup`:

```js
if (contextMenuOpened) {
  contextMenuOpened = false;
  start = null;
  dragging = false;
  lastMove = null;
  return;
}
```

**Step 6: Restore button visual dimensions**

The button should be lightweight but legible, not a tiny unexplained `P`. Use:

```html
<button id="btn" title="Open Prompt Picker" aria-label="Open Prompt Picker">
  <span class="icon">P</span><span>Prompts</span>
</button>
```

Set CSS width/height to `112px` x `40px`.

**Step 7: Run tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- src/overlay/overlayHtml.test.ts
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test
```

Expected: PASS.

**Step 8: Commit**

```bash
git add public/overlay.html src/overlay/overlayHtml.test.ts src-tauri/src/windows.rs src-tauri/src/lib.rs
git commit -m "fix: route right click through button controls popover"
```

---

### Task 3: Make Button Controls Persist Hide State and Hide the Actual Button

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/platform/platformApi.ts`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`

**Step 1: Add failing App test**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`, strengthen the `button-controls` test so it verifies the actual commands:

```ts
import { invoke } from "@tauri-apps/api/core";

it("button controls hide persists state and hides the floating button", async () => {
  currentWindowLabel = "prompt-popover";
  window.history.pushState({}, "", "/?mode=button-controls");
  const { readTextFile, writeTextFile } = await import("@tauri-apps/plugin-fs");

  const files = new Map<string, string>();
  (readTextFile as ReturnType<typeof vi.fn>).mockImplementation(async (path: string) => {
    const value = files.get(path);
    if (!value) throw new Error("missing file");
    return value;
  });
  (writeTextFile as ReturnType<typeof vi.fn>).mockImplementation(async (path: string, value: string) => {
    files.set(path, value);
  });

  await act(async () => {
    render(<App />);
  });

  fireEvent.click(await screen.findByRole("button", { name: "Hide Button" }));

  await waitFor(() => {
    expect(invoke).toHaveBeenCalledWith("hide_prompt_button");
    expect(invoke).toHaveBeenCalledWith("hide_prompt_popover");
  });
});
```

Adapt imports/mocks to the current test style.

**Step 2: Run the failing test**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- src/app/App.test.tsx
```

Expected: FAIL because `Hide Button` does not call `hide_prompt_button`.

**Step 3: Static import platform commands**

In `src/App.tsx`, replace the current import:

```ts
import { getAccessibilityStatus, hidePromptPopover, pastePrompt } from "./platform/platformApi";
```

with:

```ts
import {
  getAccessibilityStatus,
  hidePromptButton,
  hidePromptPopover,
  openMainWindow,
  pastePrompt,
} from "./platform/platformApi";
```

Remove the dynamic import at `Open Prompt Picker`.

**Step 4: Fix `button-controls` Hide Button**

Update:

```ts
await settingsStoreRef.current.setFloatingButtonVisible(false);
setActiveSettings(await settingsStoreRef.current.get());
await hidePromptPopover();
```

to:

```ts
await settingsStoreRef.current.setFloatingButtonVisible(false);
setActiveSettings(await settingsStoreRef.current.get());
await hidePromptButton();
await hidePromptPopover();
```

Update `Open Prompt Picker`:

```ts
await openMainWindow();
await hidePromptPopover();
```

**Step 5: Run tests/build**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- src/app/App.test.tsx
npm run build
```

Expected:

- PASS
- no Vite warning about dynamic import of `platformApi`

**Step 6: Commit**

```bash
git add src/App.tsx src/app/App.test.tsx src/platform/platformApi.ts
git commit -m "fix: persist and execute floating button hide controls"
```

---

### Task 4: Show Button at Startup and Keep App Alive When Main Window Closes

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`

**Step 1: Add startup button show**

In `.setup(|app| { ... })`, after setting the title:

```rust
let _ = show_prompt_button(960.0, 700.0, app.handle().clone());
```

This makes the button appear before React polling finishes.

**Step 2: Add main close-to-hide behavior**

Import `WindowEvent`:

```rust
use tauri::{Manager, WindowEvent};
```

In setup:

```rust
let window = app.get_webview_window("main").unwrap();
window.set_title("Prompt Picker").unwrap();
let main_window = window.clone();
window.on_window_event(move |event| {
    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        let _ = main_window.hide();
    }
});
let _ = show_prompt_button(960.0, 700.0, app.handle().clone());
```

This preserves the process and floating button when the user closes the main client window.

**Step 3: Run Rust tests/build**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test
```

Expected: PASS.

**Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "fix: keep floating button alive after main window closes"
```

---

### Task 5: Clean Build Warnings and Dead Code

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/package.json`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/mod.rs`
- Modify if needed: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`

**Step 1: Remove duplicate package script**

In `package.json`, remove the duplicate `preview` key so scripts contain only one:

```json
"preview": "vite preview"
```

**Step 2: Resolve `choose_main_input` warning**

If `choose_main_input` is no longer used by macOS target detection, either:

- re-use it when choosing estimated/focused target, if that matches existing behavior; or
- remove it and its tests only if no code needs it.

Prefer reusing if possible to avoid deleting established logic:

```rust
let frame = choose_main_input(&[focused_frame])?;
```

If reusing introduces unnecessary complexity, remove the unused function and related tests in the same commit.

**Step 3: Run verification commands**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm run build
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test
```

Expected:

- no duplicate key warning
- no Vite dynamic import warning for `platformApi`
- no dead-code warning for `choose_main_input`

**Step 4: Commit**

```bash
git add package.json src-tauri/src/platform/mod.rs src-tauri/src/platform/macos.rs
git commit -m "chore: clean floating button verification warnings"
```

---

### Task 6: Strengthen Runtime-Path Tests

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/useInputTargetPolling.test.ts`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/overlayHtml.test.ts`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`

**Step 1: Strengthen no-target fallback test**

Ensure `src/overlay/useInputTargetPolling.test.ts` has a test that asserts:

```ts
expect(showPromptButton).toHaveBeenCalledWith(960, 700);
expect(hidePromptButton).not.toHaveBeenCalled();
```

for first run with `getCurrentInputTarget.mockResolvedValue(null)`.

Do not settle for only asserting `hidePromptButton` was not called.

**Step 2: Strengthen overlay HTML test**

Ensure `src/overlay/overlayHtml.test.ts` asserts:

```ts
expect(html).toContain("show_prompt_button_controls_from_button");
expect(html).not.toContain("id=\"menu\"");
expect(html).not.toContain("hide_prompt_button");
```

**Step 3: Strengthen button-controls App test**

Ensure App tests verify that:

- `Hide Button` writes setting and calls `hide_prompt_button`
- `Open Prompt Picker` calls `open_main_window`
- `button-controls` mode does not render manager actions

**Step 4: Run all tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/overlay/useInputTargetPolling.test.ts src/overlay/overlayHtml.test.ts src/app/App.test.tsx
git commit -m "test: cover floating button real interaction paths"
```

---

### Task 7: Full Verification and Manual Acceptance

**Files:**
- No planned source changes.

**Step 1: Use verification-before-completion**

Before claiming done, use `superpowers:verification-before-completion`.

**Step 2: Run complete verification**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test
npm run build
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test
cargo fmt -- --check
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm run tauri build
git diff --check
git status --short
```

Expected:

- all commands pass
- no duplicate package key warning
- no Vite dynamic import warning for `platformApi`
- no Rust warning for unregistered command or dead code introduced by this work

**Step 3: Manual startup acceptance**

Run:

```bash
pkill -x prompt-picker || true
open -na "/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app"
```

Verify manually:

- Blue `Prompts` button appears on startup.
- Button is 112x40-ish and reads `Prompts`, not just a tiny `P`.
- Left click opens compact prompt list only.
- Right click opens compact `Hide Button` / `Open Prompt Picker` controls via `prompt-popover`.
- `Hide Button` hides the button and main window status becomes `Hidden`.
- Main window `Show Floating Button` restores it.
- Closing the main window does not quit the app and does not remove the floating button.

**Step 4: Commit verification fixes if needed**

If verification exposes fixes:

```bash
git add <changed-files>
git commit -m "fix: complete floating button v2 repair"
```

Do not create an empty commit.

---

## Final Acceptance Criteria

This repair can be accepted only if:

- Original plan file exists.
- Repair v1 and v2 plan files exist.
- `npm test` passes.
- `npm run build` passes without the known duplicate-key and dynamic-import warnings.
- `cargo test` passes without warnings from broken/unused code introduced here.
- `cargo fmt -- --check` passes.
- `npm run tauri build` passes.
- Button appears on startup without relying on input target detection.
- Left click opens prompt list only.
- Right click opens `button-controls` popover, not an inline HTML menu.
- Hiding from right-click controls persists hidden state.
- Showing from main app restores the button.
- Main window close hides the main window rather than terminating the app.
- `current_input_target()` can parse realistic macOS command output and is not effectively dead.

If any of these fail, do not push and do not claim completion.
