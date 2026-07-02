# OpenWhip-Style Autosend Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make clicking a prompt from the Calico popover reliably insert the prompt into the previously focused app input field and press Enter, for Codex, WeChat, and other macOS apps.

**Architecture:** Replace the current coordinate-first autosend path with an OpenWhip-style keyboard automation path: hide the prompt popover, restore the last target app, then send text and Enter through macOS System Events. Keep coordinate clicking only as a fallback, and add diagnostics so failures are visible instead of looking like "nothing happened."

**Tech Stack:** Tauri 2, React, TypeScript, Rust, macOS AppleScript/System Events, `pbcopy`, existing Tauri commands and Vitest/Cargo tests.

---

### Task 1: Add Script Builders for Direct Keyboard Autosend

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`

**Step 1: Write failing tests**

Add tests in the existing `#[cfg(test)] mod tests` block in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`:

```rust
#[test]
fn direct_type_and_submit_script_activates_target_before_typing() {
    let script = direct_type_and_submit_to_app_script("com.openai.codex", "讨论方案");

    assert!(script.contains("tell application id \"com.openai.codex\" to activate"));
    assert!(script.contains("tell application \"System Events\""));
    assert!(script.contains("keystroke \"讨论方案\""));
    assert!(script.contains("key code 36"));
}

#[test]
fn direct_type_and_submit_script_escapes_quotes_and_backslashes() {
    let script = direct_type_and_submit_to_app_script("com.test.App", "say \"hi\" \\ ok");

    assert!(script.contains("keystroke \"say \\\"hi\\\" \\\\ ok\""));
}

#[test]
fn direct_type_strategy_prefers_paste_for_multiline_text() {
    assert!(!should_direct_type("line 1\nline 2"));
}

#[test]
fn direct_type_strategy_prefers_paste_for_long_text() {
    let long = "x".repeat(700);

    assert!(!should_direct_type(&long));
}

#[test]
fn direct_type_strategy_allows_short_single_line_text() {
    assert!(should_direct_type("使用 brainstorming skill，先和我讨论方案。"));
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib platform::macos::tests::direct_type -- --nocapture
```

Expected: FAIL because `direct_type_and_submit_to_app_script` and `should_direct_type` do not exist.

**Step 3: Implement minimal helpers**

Add these helpers near the existing paste script builders in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`:

```rust
const DIRECT_TYPE_MAX_CHARS: usize = 500;

fn should_direct_type(body: &str) -> bool {
    !body.contains('\n') && body.chars().count() <= DIRECT_TYPE_MAX_CHARS
}

fn escape_applescript_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn direct_type_and_submit_to_app_script(bundle_id: &str, body: &str) -> String {
    let escaped = escape_applescript_string(body);
    format!(
        r#"tell application id "{}" to activate
delay 0.15
tell application "System Events"
    keystroke "{}"
    delay 0.08
    key code 36
end tell"#,
        bundle_id, escaped
    )
}
```

**Step 4: Run tests to verify they pass**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib platform::macos::tests::direct_type -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src-tauri/src/platform/macos.rs
git commit -m "test: cover direct keyboard autosend scripts"
```

---

### Task 2: Add OpenWhip-Style Autosend Backend Path

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`

**Step 1: Write failing tests**

Add tests in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`:

```rust
#[test]
fn autosend_direct_script_does_not_click_coordinates() {
    let script = direct_type_and_submit_to_app_script("com.tencent.xinWeChat", "讨论方案");

    assert!(!script.contains("click at"));
    assert!(script.contains("keystroke \"讨论方案\""));
    assert!(script.contains("key code 36"));
}

#[test]
fn paste_and_submit_script_remains_available_as_fallback() {
    let script = paste_and_submit_to_app_script("com.tencent.xinWeChat");

    assert!(script.contains("keystroke \"v\" using command down"));
    assert!(script.contains("key code 36"));
}
```

Add a test in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs` under `last_input_target_tests`:

```rust
#[test]
fn autosend_accepts_wechat_without_click_point() {
    let state = LastInputTargetState::default();
    state.set(LastInputTarget {
        app: FrontmostApp {
            name: "WeChat".to_string(),
            bundle_id: "com.tencent.xinWeChat".to_string(),
        },
        observed_at_ms: 123,
        click_point: None,
    });

    assert_eq!(
        last_target_bundle_id(&state).unwrap(),
        "com.tencent.xinWeChat"
    );
}
```

**Step 2: Run tests to verify baseline**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib -- --nocapture
```

Expected: existing tests pass or only new direct-autosend references fail until implementation is complete.

**Step 3: Implement autosend function**

Add this public function in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`:

```rust
pub fn type_or_paste_prompt_and_submit_to_app(body: &str, bundle_id: &str) -> Result<(), String> {
    if should_direct_type(body) {
        let script = direct_type_and_submit_to_app_script(bundle_id, body);
        let output = Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            return Ok(());
        }
    }

    paste_prompt_and_submit_to_app(body, bundle_id)
}
```

**Step 4: Use the new backend path**

Change `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs` in `paste_prompt_and_submit_to_last_target_impl` from the current click-point-first logic to this first:

```rust
platform::macos::type_or_paste_prompt_and_submit_to_app(body, &target.app.bundle_id)
```

Keep the old coordinate function in the codebase for now, but stop using it as the default.

**Step 5: Run tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib -- --nocapture
```

Expected: PASS.

**Step 6: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src-tauri/src/platform/macos.rs src-tauri/src/lib.rs
git commit -m "fix: autosend prompts with direct keyboard input"
```

---

### Task 3: Ensure Popover Fully Hides Before Autosend

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`

**Step 1: Write failing test**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`, add or update a test that verifies prompt selection order:

```tsx
it("hides the prompt popover before autosending the selected prompt", async () => {
  const calls: string[] = [];
  platformApi.getAccessibilityStatus.mockResolvedValue({ trusted: true });
  platformApi.hidePromptPopover.mockImplementation(async () => {
    calls.push("hide");
  });
  platformApi.pastePromptAndSubmitToLastTarget.mockImplementation(async () => {
    calls.push("autosend");
  });

  render(<App />);
  await userEvent.click(screen.getByRole("button", { name: /讨论方案/i }));

  expect(calls).toEqual(["hide", "autosend"]);
});
```

**Step 2: Run test to verify current behavior**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- src/app/App.test.tsx --runInBand
```

Expected: PASS if current order is already correct. If the test is hard to express due existing mocks, adjust mocks only enough to cover ordering.

**Step 3: Add a short settle delay after hiding**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`, add:

```ts
const waitForWindowHide = () => new Promise((resolve) => window.setTimeout(resolve, 120));
```

Then change the selection flow to:

```ts
await hidePromptPopover();
await waitForWindowHide();
await pastePromptAndSubmitToLastTarget(prompt.body);
```

**Step 4: Run frontend tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --runInBand
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src/App.tsx src/app/App.test.tsx
git commit -m "fix: wait for prompt popover to hide before autosend"
```

---

### Task 4: Add Autosend Diagnostics

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/platform/platformApi.ts`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`

**Step 1: Write failing Rust tests**

Add a test in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`:

```rust
#[test]
fn autosend_error_includes_stderr_when_osascript_fails() {
    let err = format_autosend_error("direct-type", "System Events got an error");

    assert!(err.contains("direct-type"));
    assert!(err.contains("System Events got an error"));
}
```

**Step 2: Implement diagnostic error formatter**

Add:

```rust
fn format_autosend_error(method: &str, stderr: &str) -> String {
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        format!("Autosend failed while using {}.", method)
    } else {
        format!("Autosend failed while using {}: {}", method, trimmed)
    }
}
```

Use it when `osascript` exits unsuccessfully.

**Step 3: Add minimal frontend error surfacing**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`, keep the existing `alert`, but make the message more actionable:

```ts
alert(
  message ||
    "Autosend failed. Click into a target input field, confirm Accessibility permission, then try again."
);
```

**Step 4: Run tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib -- --nocapture
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --runInBand
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src-tauri/src/platform/macos.rs src-tauri/src/lib.rs src/platform/platformApi.ts src/App.tsx
git commit -m "fix: surface autosend failure diagnostics"
```

---

### Task 5: Remove WeChat/Codex Coordinate Dependency from Default Flow

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`

**Step 1: Write failing test**

Add a test in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`:

```rust
#[test]
fn autosend_does_not_require_click_point_for_codex() {
    let state = LastInputTargetState::default();
    state.set(LastInputTarget {
        app: FrontmostApp {
            name: "Codex".to_string(),
            bundle_id: "com.openai.codex".to_string(),
        },
        observed_at_ms: 123,
        click_point: None,
    });

    assert_eq!(last_target_bundle_id(&state).unwrap(), "com.openai.codex");
}
```

**Step 2: Ensure default flow ignores click point**

Make `paste_prompt_and_submit_to_last_target_impl` always call:

```rust
platform::macos::type_or_paste_prompt_and_submit_to_app(body, &target.app.bundle_id)
```

Do not branch on `target.click_point` in the default path.

**Step 3: Keep coordinate functions only as fallback utilities**

Leave these functions in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs` for future explicit fallback:

```rust
paste_prompt_and_submit_to_app_at_point
paste_and_submit_to_app_at_point_script
fallback_click_point_for_app
```

Do not delete them in this task.

**Step 4: Run tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src-tauri/src/lib.rs src-tauri/src/platform/macos.rs
git commit -m "fix: stop requiring input coordinates for autosend"
```

---

### Task 6: Verify Build, Package, and Runtime Preconditions

**Files:**
- Read: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/tauri.conf.json`
- Read: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/Info.plist`
- Generated: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app`
- Generated: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/dmg/Prompt Picker_1.0.0_aarch64.dmg`

**Step 1: Run full test suite**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --runInBand
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib -- --nocapture
```

Expected: all tests pass.

**Step 2: Build frontend**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm run build
```

Expected: build succeeds and writes `/Users/yang/Desktop/GitHub-pre/prompt-picker/dist`.

**Step 3: Package App**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
CARGO_BUILD_JOBS=1 npm run tauri -- build
```

Expected:

```text
/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app
/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/dmg/Prompt Picker_1.0.0_aarch64.dmg
```

**Step 4: Verify menu-bar app setting**

Run:

```bash
plutil -p "/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app/Contents/Info.plist" | rg "LSUIElement"
```

Expected:

```text
"LSUIElement" => true
```

**Step 5: Verify signature status and note permission requirement**

Run:

```bash
codesign -dv --verbose=4 "/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app"
```

Expected: current local build may still be adhoc signed. If adhoc, final user-facing notes must say Accessibility permission may need to be re-granted after replacing the app.

**Step 6: Commit generated package only if requested**

Do not commit `dist`, `target`, `.app`, or `.dmg` changes unless the user explicitly asks to commit generated build artifacts.

---

### Task 7: Final User-Facing Smoke Checklist

**Files:**
- No code changes.

**Step 1: Kill old running app**

Run:

```bash
pkill -f "/Prompt Picker.app/Contents/MacOS/prompt-picker" || true
```

Expected: old Prompt Picker process stops.

**Step 2: Open rebuilt app**

Run:

```bash
open "/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app"
```

Expected: app runs as menu-bar utility and Calico appears.

**Step 3: Confirm Accessibility permission**

Manual check:

```text
System Settings > Privacy & Security > Accessibility > Prompt Picker enabled
```

Expected: enabled.

**Step 4: Codex smoke test**

Manual steps:

```text
1. Click into Codex input box.
2. Click Calico.
3. Click "讨论方案".
4. Confirm prompt appears in Codex input and sends.
```

Expected: prompt is entered and Enter sends it.

**Step 5: WeChat smoke test**

Manual steps:

```text
1. Click into a WeChat chat input box.
2. Click Calico.
3. Click "讨论方案".
4. Confirm prompt appears in WeChat input and sends.
```

Expected: prompt is entered and Enter sends it.

**Step 6: Commit final source changes**

If all checks pass:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git status --short
git add src-tauri/src/platform/macos.rs src-tauri/src/lib.rs src/App.tsx src/app/App.test.tsx src/platform/platformApi.ts
git commit -m "fix: make prompt autosend use stable keyboard automation"
```

---

### Task 8: Push Only After User Approval

**Files:**
- No source changes.

**Step 1: Confirm branch**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git branch --show-current
git status --short
```

Expected: on the intended branch, with only expected generated files uncommitted.

**Step 2: Push**

Run only after user asks to push:

```bash
git push
```

Expected: source commits pushed to GitHub.

---

## Notes and Boundaries

- This plan intentionally does not remove coordinate-click code in the first pass. Removing it is a separate cleanup after the keyboard automation path proves stable.
- This plan does not promise success without macOS Accessibility permission. Cross-app keyboard automation requires that permission.
- This plan changes the default behavior from coordinate-first to keyboard-first, which is the main architectural improvement borrowed from OpenWhip.
- The React prompt list cannot be literally identical to OpenWhip because OpenWhip has no interactive list. This plan preserves our product UI while adopting OpenWhip's more stable automation principle.
