# Native Autosend Event Injection Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make prompt selection reliably paste the selected prompt into the current Codex/WeChat input field and press Enter, without asking the user to manually Cmd+V.

**Architecture:** Replace the fragile `osascript -> System Events -> Cmd+V + Enter` autosend path with a native macOS CoreGraphics keyboard event path owned by Prompt Picker itself. Keep clipboard-based paste for Chinese, long, and multiline prompts, but post `Cmd+V` and `Return` through `CGEvent`, add focused timing, and expose clear structured outcomes for success versus permission/focus failure.

**Tech Stack:** Tauri 2, Rust 2021, macOS ApplicationServices/CoreGraphics, React, TypeScript, Vitest, Cargo tests, macOS Info.plist/TCC permissions.

---

### Task 1: Add Native macOS Keyboard Event Helpers

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`

**Step 1: Write failing Rust tests for the native script-free path**

Add tests inside `#[cfg(test)] mod tests`:

```rust
#[test]
fn native_autosend_sequence_uses_paste_then_return() {
    let sequence = native_autosend_event_sequence();

    assert_eq!(
        sequence,
        vec![
            "cmd-down",
            "v-down",
            "v-up",
            "cmd-up",
            "return-down",
            "return-up",
        ]
    );
}

#[test]
fn native_autosend_does_not_depend_on_osascript() {
    assert!(!native_autosend_uses_osascript());
}
```

**Step 2: Run tests to verify failure**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib native_autosend -- --nocapture
```

Expected: FAIL because `native_autosend_event_sequence` and `native_autosend_uses_osascript` do not exist.

**Step 3: Implement the test-visible event sequence helpers**

Add near the paste helpers:

```rust
#[cfg(test)]
fn native_autosend_event_sequence() -> Vec<&'static str> {
    vec![
        "cmd-down",
        "v-down",
        "v-up",
        "cmd-up",
        "return-down",
        "return-up",
    ]
}

#[cfg(test)]
fn native_autosend_uses_osascript() -> bool {
    false
}
```

**Step 4: Implement native key posting helpers**

Add macOS CoreGraphics FFI in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`:

```rust
type CGEventSourceRef = *mut std::ffi::c_void;
type CGEventRef = *mut std::ffi::c_void;
type CGEventFlags = u64;
type CGKeyCode = u16;

const CG_EVENT_FLAG_MASK_COMMAND: CGEventFlags = 1 << 20;
const KEY_CODE_V: CGKeyCode = 9;
const KEY_CODE_RETURN: CGKeyCode = 36;
const KEY_CODE_COMMAND: CGKeyCode = 55;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGEventCreateKeyboardEvent(
        source: CGEventSourceRef,
        virtualKey: CGKeyCode,
        keyDown: bool,
    ) -> CGEventRef;
    fn CGEventSetFlags(event: CGEventRef, flags: CGEventFlags);
    fn CGEventPost(tap: u32, event: CGEventRef);
    fn CFRelease(cf: *const std::ffi::c_void);
}

const CG_HID_EVENT_TAP: u32 = 0;
```

Add helpers:

```rust
fn post_key_event(key_code: CGKeyCode, key_down: bool, flags: CGEventFlags) -> Result<(), String> {
    unsafe {
        let event = CGEventCreateKeyboardEvent(std::ptr::null_mut(), key_code, key_down);
        if event.is_null() {
            return Err("CGEventCreateKeyboardEvent returned null".to_string());
        }
        CGEventSetFlags(event, flags);
        CGEventPost(CG_HID_EVENT_TAP, event);
        CFRelease(event.cast_const());
    }
    Ok(())
}

fn post_key_tap(key_code: CGKeyCode, flags: CGEventFlags) -> Result<(), String> {
    post_key_event(key_code, true, flags)?;
    post_key_event(key_code, false, flags)
}

fn post_paste_shortcut() -> Result<(), String> {
    post_key_event(KEY_CODE_COMMAND, true, CG_EVENT_FLAG_MASK_COMMAND)?;
    post_key_tap(KEY_CODE_V, CG_EVENT_FLAG_MASK_COMMAND)?;
    post_key_event(KEY_CODE_COMMAND, false, 0)
}

fn post_return_key() -> Result<(), String> {
    post_key_tap(KEY_CODE_RETURN, 0)
}
```

**Step 5: Run focused Rust tests**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib native_autosend -- --nocapture
```

Expected: PASS.

**Step 6: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src-tauri/src/platform/macos.rs
git commit -m "feat: add native macos key events"
```

---

### Task 2: Replace Foreground Autosend with Native Event Injection

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`

**Step 1: Write failing tests for structured native failure**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`, add:

```rust
#[test]
fn autosend_outcome_reports_native_keyboard_failure_after_copy() {
    let outcome = AutosendOutcome::keyboard_failed("native key event failed".to_string());

    assert!(outcome.copied);
    assert!(!outcome.sent);
    assert_eq!(outcome.error.as_deref(), Some("native key event failed"));
}
```

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`, update the existing `autosend_returns_foreground_outcome_without_last_target` test if needed so it still asserts a structured `AutosendOutcome`.

**Step 2: Run focused tests**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib autosend_outcome autosend_returns_foreground_outcome_without_last_target -- --nocapture
```

Expected: If Cargo rejects multiple filters, run these two commands separately:

```bash
cargo test --lib autosend_outcome -- --nocapture
cargo test --lib autosend_returns_foreground_outcome_without_last_target -- --nocapture
```

**Step 3: Implement native autosend body**

Change `paste_prompt_and_submit_to_foreground` to:

```rust
pub fn paste_prompt_and_submit_to_foreground(body: &str) -> Result<AutosendOutcome, String> {
    if let Err(error) = copy_to_clipboard(body) {
        return Ok(AutosendOutcome::copy_failed(error));
    }

    refocus_previous_app_if_prompt_picker_frontmost();
    std::thread::sleep(std::time::Duration::from_millis(280));

    if let Err(error) = post_paste_shortcut() {
        return Ok(AutosendOutcome::keyboard_failed(format!(
            "Native paste event failed: {}",
            error
        )));
    }

    std::thread::sleep(std::time::Duration::from_millis(320));

    if let Err(error) = post_return_key() {
        return Ok(AutosendOutcome::keyboard_failed(format!(
            "Native return event failed: {}",
            error
        )));
    }

    Ok(AutosendOutcome::sent())
}
```

Keep `foreground_paste_and_submit_script` only if older tests still need it, but stop using it in the active autosend path.

**Step 4: Run focused Rust tests**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib autosend_outcome -- --nocapture
cargo test --lib foreground_paste_and_submit_script_matches_openwhip_focus_model -- --nocapture
cargo test --lib autosend_returns_foreground_outcome_without_last_target -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src-tauri/src/platform/macos.rs src-tauri/src/lib.rs
git commit -m "fix: use native events for autosend"
```

---

### Task 3: Stabilize Focus Timing Around Prompt Selection

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`

**Step 1: Write failing frontend timing test**

Add to `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`:

```tsx
it("waits long enough for the popover to hide before autosend", async () => {
  vi.useFakeTimers();
  const { invoke } = await import("@tauri-apps/api/core");
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    if (command === "paste_prompt_and_submit_to_last_target") {
      return { copied: true, sent: true, error: null };
    }
    return undefined;
  });
  const { readTextFile } = await import("@tauri-apps/plugin-fs");
  (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
    JSON.stringify({ version: 1, prompts: mockPrompts })
  );

  await act(async () => {
    render(<App />);
  });

  fireEvent.click(await screen.findByText("Test Prompt"));

  await waitFor(() => {
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("hide_prompt_popover");
  });
  expect(vi.mocked(invoke)).not.toHaveBeenCalledWith(
    "paste_prompt_and_submit_to_last_target",
    expect.anything()
  );

  await act(async () => {
    vi.advanceTimersByTime(260);
  });

  await waitFor(() => {
    expect(vi.mocked(invoke)).toHaveBeenCalledWith(
      "paste_prompt_and_submit_to_last_target",
      { body: "Test body" }
    );
  });

  vi.useRealTimers();
});
```

**Step 2: Run test to verify failure**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "waits long enough"
```

Expected: FAIL because the current `waitForWindowHide` is only 120ms.

**Step 3: Increase hide wait**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`, change:

```tsx
const waitForWindowHide = () => new Promise((resolve) => window.setTimeout(resolve, 260));
```

**Step 4: Run frontend timing tests**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "waits long enough|hides the prompt popover before autosending"
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src/App.tsx src/app/App.test.tsx
git commit -m "fix: wait for focus before autosend"
```

---

### Task 4: Replace Manual Paste Messaging with Permission-Centered Failure

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/public/overlay.html`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/overlayHtml.test.ts`

**Step 1: Write failing frontend status tests**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`, add:

```tsx
it("does not tell the user to manually paste when autosend keyboard automation fails", async () => {
  const { invoke } = await import("@tauri-apps/api/core");
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    if (command === "paste_prompt_and_submit_to_last_target") {
      return { copied: true, sent: false, error: "Native paste event failed" };
    }
    return undefined;
  });
  const { readTextFile } = await import("@tauri-apps/plugin-fs");
  (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
    JSON.stringify({ version: 1, prompts: mockPrompts })
  );

  await act(async () => {
    render(<App />);
  });

  fireEvent.click(await screen.findByText("Test Prompt"));

  await waitFor(() => {
    expect(emitMock).toHaveBeenCalledWith("prompt-autosend-status", {
      kind: "failed",
      message: "未能自动发送，请检查权限",
    });
  });
  expect(emitMock).not.toHaveBeenCalledWith(
    "prompt-autosend-status",
    expect.objectContaining({ message: "已复制，可手动 Cmd+V" })
  );
});
```

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/overlayHtml.test.ts`, add:

```ts
it("does not use manual paste as the default failure copy", () => {
  const html = readFileSync("public/overlay.html", "utf8");

  expect(html).not.toContain("可手动 Cmd+V");
});
```

**Step 2: Run tests to verify failure**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "manual paste|default failure copy"
```

Expected: FAIL because the current copied fallback still says `已复制，可手动 Cmd+V`.

**Step 3: Update frontend status mapping**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`, change copied-but-unsent handling:

```tsx
} else if (outcome.copied) {
  await emitAutosendStatus("failed", "未能自动发送，请检查权限");
}
```

Keep copy failure as:

```tsx
await emitAutosendStatus("failed", "未能复制，请重试");
```

**Step 4: Update overlay default message**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/public/overlay.html`, change:

```js
const message = payload.message || '未能自动发送';
```

**Step 5: Run tests**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "manual paste|default failure copy|emits a copied status"
```

Expected: Update old test name/assertions from “copied status” to “failed status after copied keyboard failure”, then PASS.

**Step 6: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src/App.tsx src/app/App.test.tsx public/overlay.html src/overlay/overlayHtml.test.ts
git commit -m "fix: report autosend failure clearly"
```

---

### Task 5: Add macOS Permission Metadata and Package Identity Checks

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/Info.plist`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`

**Step 1: Write failing Rust package metadata test**

Add to `menu_bar_app_tests` in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`:

```rust
#[test]
fn macos_info_plist_declares_apple_events_usage() {
    let info_plist = include_str!("../Info.plist");

    assert!(info_plist.contains("<key>NSAppleEventsUsageDescription</key>"));
    assert!(info_plist.contains("send keyboard events"));
}
```

**Step 2: Run test to verify failure**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib macos_info_plist_declares_apple_events_usage -- --nocapture
```

Expected: FAIL because `NSAppleEventsUsageDescription` is missing.

**Step 3: Add Info.plist usage description**

Change `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/Info.plist` to include:

```xml
<key>NSAppleEventsUsageDescription</key>
<string>Prompt Picker needs to send keyboard events to paste and submit your selected prompt in the active input field.</string>
```

Keep existing `LSUIElement` unchanged.

**Step 4: Run test**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib macos_info_plist_declares_apple_events_usage -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src-tauri/Info.plist src-tauri/src/lib.rs
git commit -m "fix: declare macos automation usage"
```

---

### Task 6: Verification, Packaging, and Push

**Files:**
- No source changes expected unless verification finds a defect.

**Step 1: Run full frontend tests**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test
```

Expected: PASS, all Vitest tests pass.

**Step 2: Run Rust format and tests**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo fmt --check
cargo test --lib -- --nocapture
```

Expected: PASS.

**Step 3: Build frontend**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm run build
```

Expected: PASS.

**Step 4: Build Tauri release app**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
CARGO_BUILD_JOBS=1 npm run tauri -- build
```

Expected: PASS and produce:

```text
/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app
/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/dmg/Prompt Picker_1.0.0_aarch64.dmg
```

**Step 5: Static package checks**

```bash
plutil -p "/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app/Contents/Info.plist" | rg "LSUIElement|CFBundleIdentifier|NSAppleEventsUsageDescription"
codesign -dv --verbose=4 "/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app" 2>&1 | rg "Identifier|Signature|TeamIdentifier|Info.plist"
strings "/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app/Contents/MacOS/prompt-picker" | rg "Native paste event failed|Native return event failed|Privacy_Accessibility"
```

Expected:
- `LSUIElement` is true.
- `CFBundleIdentifier` is `local.promptpicker.dev`.
- `NSAppleEventsUsageDescription` exists.
- Binary contains native autosend failure strings.

Note: If `codesign` still reports `Signature=adhoc`, document that the app still requires manual Accessibility authorization on the exact built app. Do not claim permission bypass.

**Step 6: Commit final fixes if verification required them**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add <changed source files>
git commit -m "fix: stabilize native autosend"
```

Skip if no source fixes were needed.

**Step 7: Push**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git status --short
git push origin main
```

Expected:
- Source files are committed.
- Generated `dist`, `node_modules`, `src-tauri/target`, `.app`, and `.dmg` changes may remain uncommitted and should not be staged.
- Push reports `main -> main`.

**Step 8: Final user-facing summary**

Report:

```text
Click prompt
→ list closes
→ app waits for focus to settle
→ native CGEvent posts Cmd+V
→ native CGEvent posts Return
→ if successful: Calico shows "已发送"
→ if macOS blocks automation: Calico shows "未能自动发送，请检查权限"
```

Include:
- Commit hash.
- Verification commands and pass counts.
- App and DMG paths.

