# Always Visible Floating Button Repair Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Repair the failed always-visible floating button implementation so it matches the confirmed product contract: the blue `Prompts` button stays visible while Prompt Picker runs, left click opens only the compact prompt list, right click opens compact button controls, and hidden buttons can be restored from the main app window.

**Architecture:** Preserve the existing Tauri/React three-window model: `main` is the client app window, `prompt-button` is the persistent lightweight overlay, and `prompt-popover` renders either the quick prompt list or the small button-controls panel. Restore the previously broken macOS platform commands instead of stubbing them, and make button visibility controlled by user setting rather than by input-target detection.

**Tech Stack:** Tauri 2, React 19, TypeScript, Vitest, Rust 2021, macOS AppKit non-activating panels, macOS Accessibility/CoreGraphics/System Events as already used by the project.

---

## Repair Scope

This is a repair plan, not a redesign. Do not add Windows support, do not redesign the prompt manager, do not add browser extension behavior, and do not expand the product scope.

The current implementation is not acceptable because it:

- Deleted the original plan file.
- Removed the `npm test` script required by the plan.
- Does not show the button on app startup.
- Does not show a fallback button when no input target exists.
- Does not wire right-click button controls from the actual overlay button.
- Defines `open_main_window` but does not register it with Tauri.
- Stubs `current_input_target()`, `frontmost_app()`, and Accessibility status, breaking runtime behavior.
- Leaves tests passing mostly because mocks do not verify the real production paths.

The repaired implementation must match the confirmed product contract:

- App launch shows the blue `Prompts` button by default.
- Switching to ordinary apps does not hide the button.
- Closing/hiding the main app window does not hide the button.
- Left clicking the button opens only the compact prompt list.
- Right clicking the button opens only compact controls: `Hide Button` and `Open Prompt Picker`.
- `Hide Button` hides the floating button.
- Opening the Prompt Picker main window lets the user restore the button via `Show Floating Button`.
- The prompt list does not show Add/Edit/Delete/Import/Export/Settings/Back.
- Existing paste and macOS target-detection behavior is not destroyed.

---

### Task 1: Restore Plan Traceability and Test Script

**Files:**
- Create or restore: `/Users/yang/Desktop/GitHub-pre/prompt-picker/docs/plans/2026-05-31-always-visible-floating-button.md`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/package.json`

**Step 1: Restore the original plan document**

Recreate `/Users/yang/Desktop/GitHub-pre/prompt-picker/docs/plans/2026-05-31-always-visible-floating-button.md` if it is missing. It must contain the confirmed plan for the always-visible floating button, including:

- product contract
- current code context
- tasks for persistent setting, main-window controls, target-independent visibility, startup button creation, right-click controls, tray non-reliance, and verification

Use the previous confirmed plan content if available in conversation context. If not available, reconstruct it from the repair scope above. Do not replace this repair plan.

**Step 2: Restore the npm test script**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/package.json`, restore:

```json
"test": "vitest run"
```

inside `scripts`.

Expected `scripts` shape:

```json
"scripts": {
  "dev": "vite",
  "build": "tsc && vite build",
  "test": "vitest run",
  "test:watch": "vitest",
  "preview": "vite preview",
  "tauri": "tauri"
}
```

If `test:watch` was intentionally removed, restore it as shown unless doing so conflicts with current package constraints.

**Step 3: Verify**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --runInBand
```

Expected: Vitest may reject `--runInBand`; if it does, run:

```bash
npm test
```

Expected: tests run through `vitest run` instead of failing with `Missing script: "test"`.

**Step 4: Commit**

```bash
git add package.json docs/plans/2026-05-31-always-visible-floating-button.md docs/plans/2026-05-31-always-visible-floating-button-repair.md
git commit -m "chore: restore floating button plan and test script"
```

---

### Task 2: Restore macOS Platform Runtime Behavior

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/mod.rs`

**Step 1: Confirm the bad stubs**

Before editing, verify these bad states exist:

```rust
fn current_input_target() -> Option<platform::InputTarget> {
    None
}
```

```rust
pub fn accessibility_status() -> AccessibilityStatus {
    AccessibilityStatus { trusted: false }
}

pub fn frontmost_app() -> Option<FrontmostApp> {
    None
}
```

These are not acceptable for production.

**Step 2: Restore `current_input_target()` delegation**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`, change:

```rust
#[tauri::command]
fn current_input_target() -> Option<platform::InputTarget> {
    None
}
```

to:

```rust
#[tauri::command]
fn current_input_target() -> Option<platform::InputTarget> {
    platform::macos::current_input_target()
}
```

**Step 3: Restore real macOS Accessibility and frontmost app implementation**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`, restore the implementation that:

- calls `AXIsProcessTrusted()` for `accessibility_status()`
- uses `lsappinfo front` and `lsappinfo info` to return `frontmost_app()`
- implements `current_input_target()` using the frontmost window frame
- excludes Prompt Picker itself as target
- remembers the last target app for paste

At minimum, the restored public functions must include:

```rust
pub fn accessibility_status() -> AccessibilityStatus {
    AccessibilityStatus {
        trusted: unsafe { AXIsProcessTrusted() },
    }
}

pub fn frontmost_app() -> Option<FrontmostApp> {
    frontmost_app_info().map(|info| info.app)
}

pub fn current_input_target() -> Option<InputTarget> {
    // real implementation, not None
}
```

Do not leave `accessibility_status()` returning a hardcoded value.

**Step 4: Keep paste behavior compatible**

Do not remove working paste behavior. If the current branch has only `paste_prompt` and `paste_prompt_to_app`, keep both. If the prior implementation had `paste_prompt_to_last_target`, restore it only if the frontend still uses it. Do not expand paste scope beyond restoring broken behavior.

**Step 5: Run Rust tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test
```

Expected: PASS without newly introduced dead-code warnings for restored commands.

**Step 6: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/platform/macos.rs src-tauri/src/platform/mod.rs
git commit -m "fix: restore macos target detection and accessibility status"
```

---

### Task 3: Make the Floating Button Actually Appear by Default

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/useInputTargetPolling.ts`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/useInputTargetPolling.test.ts`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`

**Step 1: Add a failing hook test for first-run fallback**

Add or strengthen this test in `src/overlay/useInputTargetPolling.test.ts`:

```ts
it("shows the default floating button position on first run when no input target exists", async () => {
  getFrontmostApp.mockResolvedValue({ name: "Finder", bundle_id: "com.apple.finder" });
  getCurrentInputTarget.mockResolvedValue(null);

  renderHook(() => useInputTargetPolling([], { buttonOffset: null }, {}, true));

  await act(async () => {
    vi.advanceTimersByTime(1500);
  });

  await waitFor(() => {
    expect(showPromptButton).toHaveBeenCalledWith(960, 700);
  });
  expect(hidePromptButton).not.toHaveBeenCalled();
});
```

The current implementation is expected to fail this because it only asserts that `hidePromptButton` was not called.

**Step 2: Fix fallback display in the hook**

In `src/overlay/useInputTargetPolling.ts`, define:

```ts
const DEFAULT_BUTTON_POSITION: [number, number] = [960, 700];
```

Initialize:

```ts
const lastButtonPositionRef = useRef<[number, number] | null>(DEFAULT_BUTTON_POSITION);
```

When `floatingButtonVisible` is true and there is no input target, explicitly show the fallback button:

```ts
const [x, y] = lastButtonPositionRef.current ?? DEFAULT_BUTTON_POSITION;
await showPromptButton(x, y);
setShowAttached(false);
```

Do this for:

- no input target
- Prompt Picker self-interaction without a recent target
- frontmost app unavailable
- blacklisted app if blacklist is still used to suppress target attachment

Manual hidden state remains the only normal path that hides the button:

```ts
if (!floatingButtonVisible) {
  setTarget(null);
  setShowAttached(false);
  await hidePromptButton();
  await hidePromptPopover();
  return;
}
```

**Step 3: Startup button creation**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`, inside `.setup(|app| { ... })`, after setting the main window title, call:

```rust
let _ = show_prompt_button(960.0, 700.0, app.handle().clone());
```

Do not parse settings in Rust for this task. React will hide the button after settings load if `floatingButton.visible` is false.

**Step 4: Run focused tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- src/overlay/useInputTargetPolling.test.ts
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/overlay/useInputTargetPolling.ts src/overlay/useInputTargetPolling.test.ts src-tauri/src/lib.rs
git commit -m "fix: show floating button by default"
```

---

### Task 4: Wire Right-Click Button Controls End to End

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/public/overlay.html`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/windows.rs`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/platform/platformApi.ts`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`

**Step 1: Register `open_main_window`**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`, add `open_main_window` to `tauri::generate_handler!`:

```rust
open_main_window,
```

This must remove the `function open_main_window is never used` warning.

**Step 2: Add a Rust command for right-click controls**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/windows.rs`, implement:

```rust
#[tauri::command]
pub fn show_prompt_button_controls_from_button(app: tauri::AppHandle) -> Result<(), String> {
    let position = app
        .get_webview_window(BUTTON_WINDOW_LABEL)
        .and_then(|window| {
            let position = window.outer_position().ok()?;
            let scale = window.scale_factor().unwrap_or(1.0);
            Some((
                position.x as f64 / scale + BUTTON_WIDTH + POPOVER_GAP,
                position.y as f64 / scale,
            ))
        })
        .unwrap_or((100.0, 100.0));

    show_popover_mode(position.0, position.1, "button-controls", app)
}
```

If `BUTTON_WIDTH`, `POPOVER_GAP`, and `show_popover_mode` do not exist in current `windows.rs`, refactor `show_prompt_popover_from_button()` and `show_prompt_button_controls_from_button()` to share one helper:

```rust
fn show_popover_from_button_with_mode(mode: &str, app: tauri::AppHandle) -> Result<(), String> {
    // compute button-relative position once
    // build WebviewUrl::App(format!("index.html?mode={mode}").into())
}
```

Do not leave `show_prompt_popover_from_button()` creating `index.html` without a mode if that causes right-click controls to be impossible.

**Step 3: Register the right-click command**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`, export and register:

```rust
show_prompt_button_controls_from_button,
```

inside both the `pub use windows::{ ... }` list and `tauri::generate_handler![ ... ]`.

**Step 4: Wire `public/overlay.html`**

Add:

```js
let contextMenuOpened = false;

btn.addEventListener('contextmenu', async (event) => {
  event.preventDefault();
  contextMenuOpened = true;
  if (!tauri?.core?.invoke) return;
  await tauri.core.invoke('show_prompt_button_controls_from_button');
});
```

At the start of `pointerup`, add:

```js
if (contextMenuOpened) {
  contextMenuOpened = false;
  start = null;
  dragging = false;
  lastMove = null;
  return;
}
```

Keep left click mapped to `show_prompt_popover_from_button`.

**Step 5: Make button-controls actually hide the button**

In `src/App.tsx`, the `Hide Button` handler currently updates settings and hides the popover. It must also call `hidePromptButton()` after saving:

```ts
await hidePromptButton();
await hidePromptPopover();
```

Import `hidePromptButton` from `src/platform/platformApi.ts`.

**Step 6: Avoid dynamic import for `openMainWindow`**

In `src/App.tsx`, statically import `openMainWindow` from `src/platform/platformApi.ts`. Remove the dynamic import to avoid the Vite warning:

```ts
import { getAccessibilityStatus, hidePromptButton, hidePromptPopover, openMainWindow, pastePrompt } from "./platform/platformApi";
```

Then call:

```ts
await openMainWindow();
```

**Step 7: Run tests and build**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- src/app/App.test.tsx
npm run build
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test
```

Expected:

- PASS
- no Vite dynamic import warning for `platformApi`
- no Rust warning that `open_main_window` is unused

**Step 8: Commit**

```bash
git add public/overlay.html src/App.tsx src/app/App.test.tsx src/platform/platformApi.ts src-tauri/src/lib.rs src-tauri/src/windows.rs
git commit -m "fix: wire floating button context controls"
```

---

### Task 5: Repair Window Size, URL Mode, and Non-Activating Behavior

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/windows.rs`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/public/overlay.html`
- Modify if needed: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/styles.css`

**Step 1: Restore button dimensions**

The current `prompt-button` is 32x32 and only shows `P`. The confirmed UX was a lightweight blue `Prompts` button. Use the agreed compact but legible size:

```rust
const BUTTON_WIDTH: f64 = 112.0;
const BUTTON_HEIGHT: f64 = 40.0;
```

Set new and existing button windows to this size.

**Step 2: Restore overlay button markup**

In `public/overlay.html`, render:

```html
<button id="btn" title="Open Prompt Picker" aria-label="Open Prompt Picker">
  <span class="icon">P</span>
  <span>Prompts</span>
</button>
```

Use the existing compact blue styling:

```css
button {
  width: 112px;
  height: 40px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border-radius: 8px;
  background: #2563eb;
  color: #fff;
}
```

**Step 3: Use URL mode for popovers**

In `windows.rs`, make left-click prompt list open:

```text
index.html?mode=popover
```

and right-click controls open:

```text
index.html?mode=button-controls
```

Do not open the main app UI in the popover window.

**Step 4: Keep prompt popover compact**

Use compact popover dimensions:

```rust
const POPOVER_WIDTH: f64 = 280.0;
const POPOVER_HEIGHT: f64 = 240.0;
```

Controls mode may use the same size or smaller. Do not use the main manager dimensions.

**Step 5: Preserve non-activating panel configuration**

For `prompt-button` and quick `prompt-popover`, keep:

- `always_on_top(true)`
- `visible_on_all_workspaces(true)` on macOS where available
- `focused(false)`
- `focusable(false)` for quick picker where possible
- `skip_taskbar(true)`
- `crate::macos_panels::configure_non_activating_panel(&window)?`

Do not make the main app window non-activating.

**Step 6: Verify build**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm run build
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test
```

Expected: PASS.

**Step 7: Commit**

```bash
git add public/overlay.html src/styles.css src-tauri/src/windows.rs
git commit -m "fix: restore compact non-activating floating button windows"
```

---

### Task 6: Fix Tests So They Catch the Real Bugs

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/useInputTargetPolling.test.ts`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/shared/settingsStore.test.ts`

**Step 1: Strengthen no-target test**

Ensure the no-target test asserts both:

```ts
expect(hidePromptButton).not.toHaveBeenCalled();
expect(showPromptButton).toHaveBeenCalledWith(960, 700);
```

This prevents the current false positive where the test passes without showing the button.

**Step 2: Strengthen right-click controls test**

Add a test or static assertion that the button overlay calls `show_prompt_button_controls_from_button`. Since `public/overlay.html` is plain HTML, a pragmatic test may read the file:

```ts
import { readFileSync } from "node:fs";

it("wires right click to button controls command", () => {
  const overlay = readFileSync("public/overlay.html", "utf8");
  expect(overlay).toContain("contextmenu");
  expect(overlay).toContain("show_prompt_button_controls_from_button");
});
```

Place this in a small test file such as:

```text
/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/overlayHtml.test.ts
```

**Step 3: Strengthen App controls test**

Ensure clicking `Hide Button` in `button-controls` mode calls both:

- settings write
- `hide_prompt_button`
- `hide_prompt_popover`

If mocks make direct command checks hard, mock `platformApi.hidePromptButton` and `hidePromptPopover` through a stable vi mock.

**Step 4: Run all tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test
```

Expected: PASS with tests that would fail against the current broken implementation.

**Step 5: Commit**

```bash
git add src/overlay/useInputTargetPolling.test.ts src/overlay/overlayHtml.test.ts src/app/App.test.tsx src/shared/settingsStore.test.ts
git commit -m "test: cover floating button runtime paths"
```

---

### Task 7: Full Verification Before Completion

**Files:**
- No source changes expected unless verification exposes a bug.

**Step 1: Use verification-before-completion**

Before claiming completion, use `superpowers:verification-before-completion`. Do not report success without fresh command evidence.

**Step 2: Run frontend tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test
```

Expected: PASS. Record test count.

**Step 3: Run frontend build**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm run build
```

Expected: PASS without the dynamic import warning for `platformApi`.

**Step 4: Run Rust tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test
```

Expected: PASS. Investigate and fix new warnings that indicate unregistered or unreachable code.

**Step 5: Run Rust formatting check**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo fmt -- --check
```

Expected: PASS.

**Step 6: Build Tauri app**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm run tauri build
```

Expected: PASS and produce:

```text
/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app
/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/dmg/Prompt Picker_1.0.0_aarch64.dmg
```

**Step 7: Run diff checks**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git diff --check
git status --short
```

Expected:

- `git diff --check` passes.
- Only intentional files are changed.

**Step 8: Manual acceptance test**

Run:

```bash
pkill -x prompt-picker || true
open -na "/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app"
```

Verify:

- Blue `Prompts` button appears after launch.
- The button remains visible when switching to Finder, browser, desktop, and Codex if available.
- Closing/hiding the main app does not hide the button.
- Left click opens only the compact prompt list.
- Right click opens only `Hide Button` / `Open Prompt Picker`.
- `Hide Button` hides the floating button.
- Reopening Prompt Picker main window shows `Status: Hidden`.
- Clicking `Show Floating Button` restores the button.
- Clicking `Open Prompt Picker` from right-click controls opens the main app window.

**Step 9: Commit any verification fixes**

If verification required fixes:

```bash
git add <changed-files>
git commit -m "fix: complete floating button repair"
```

Do not create an empty commit.

---

## Final Acceptance Criteria

The repair is complete only when all of these are true:

- Original plan file exists.
- Repair plan file exists.
- `npm test` works and passes.
- `npm run build` passes.
- `cargo test` passes.
- `cargo fmt -- --check` passes.
- `npm run tauri build` passes.
- Blue `Prompts` button appears on startup.
- Button remains visible without needing an input target.
- Left click opens prompt list only.
- Right click opens control popover only.
- Hidden button can be restored from the main app.
- `open_main_window` is registered and callable.
- macOS Accessibility status is not hardcoded to false.
- `current_input_target()` is not hardcoded to `None`.

If any of these fail, do not mark the task complete and do not push to `main`.
