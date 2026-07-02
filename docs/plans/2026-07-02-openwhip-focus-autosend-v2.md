# OpenWhip Focus Autosend V2 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make prompt selection from the Calico popover visibly respond, restore the previous target app focus, insert the chosen prompt, and press Enter in Codex, WeChat, and other macOS apps.

**Architecture:** Move autosend permission handling to the backend and make the frontend hide the prompt popover before any blocking check. Use an OpenWhip-inspired focus restore layer, but do not blindly Cmd+Tab because this app uses a non-activating prompt popover; run Cmd+Tab only when Prompt Picker actually became frontmost, otherwise preserve/activate the recorded target app. Keep direct keyboard input as the default for short single-line prompts, paste fallback for longer or multiline prompts, and show visible dialog errors when automation cannot run.

**Tech Stack:** Tauri 2, React, TypeScript, Rust, macOS AppleScript/System Events, `@tauri-apps/plugin-dialog`, `pbcopy`, existing Vitest and Cargo tests.

---

### Task 1: Make Prompt Selection Always Hide the Popover First

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`

**Step 1: Write the failing tests**

Add these tests to `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx` inside the existing `describe("app", () => { ... })` block:

```tsx
it("does not run a frontend accessibility preflight before autosend", async () => {
  const { invoke } = await import("@tauri-apps/api/core");
  vi.mocked(invoke).mockClear();
  vi.mocked(invoke).mockResolvedValue(undefined);
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
    expect(vi.mocked(invoke)).toHaveBeenCalledWith(
      "paste_prompt_and_submit_to_last_target",
      { body: "Test body" }
    );
  });

  expect(vi.mocked(invoke)).not.toHaveBeenCalledWith("accessibility_status_cmd");
});

it("hides the prompt popover before surfacing an autosend failure", async () => {
  const { invoke } = await import("@tauri-apps/api/core");
  const { message } = await import("@tauri-apps/plugin-dialog");
  const calls: string[] = [];
  vi.mocked(invoke).mockClear();
  vi.mocked(message).mockClear();
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    if (command === "hide_prompt_popover") {
      calls.push("hide");
      return undefined;
    }
    if (command === "paste_prompt_and_submit_to_last_target") {
      calls.push("autosend");
      throw new Error("Accessibility permission required for autosend.");
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
    expect(calls).toEqual(["hide", "autosend"]);
    expect(vi.mocked(message)).toHaveBeenCalledWith(
      "Accessibility permission required for autosend.",
      { title: "Prompt Picker", kind: "error" }
    );
  });
});
```

Update the existing dialog mock in the same test file from:

```tsx
vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(),
  open: vi.fn(),
}));
```

to:

```tsx
vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(),
  open: vi.fn(),
  message: vi.fn().mockResolvedValue("Ok"),
}));
```

**Step 2: Run tests to verify failure**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "frontend accessibility preflight|autosend failure"
```

Expected: FAIL because current `handleSelect` calls `getAccessibilityStatus()` before `hidePromptPopover()` and still uses `alert` instead of `message()`.

**Step 3: Implement the frontend flow**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`, remove `getAccessibilityStatus` from the `./platform/platformApi` import and import `message` from the dialog plugin:

```tsx
import { save, open, message } from "@tauri-apps/plugin-dialog";
```

Change `handleSelect` to this order:

```tsx
const handleSelect = async (prompt: PromptItem) => {
  if (submittingPromptId) return;
  setSubmittingPromptId(prompt.id);
  try {
    await hidePromptPopover();
    await waitForWindowHide();
    await pastePromptAndSubmitToLastTarget(prompt.body);
  } catch (e) {
    console.error("Failed to paste prompt:", e);
    const messageText = e instanceof Error ? e.message : String(e);
    await message(
      messageText ||
        "Autosend failed. Click into a target input field, confirm Accessibility permission, then try again.",
      { title: "Prompt Picker", kind: "error" }
    );
  } finally {
    setSubmittingPromptId(null);
  }
};
```

**Step 4: Run tests to verify pass**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "frontend accessibility preflight|autosend failure|hides the prompt popover before autosending"
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src/App.tsx src/app/App.test.tsx
git commit -m "fix: hide prompt popover before autosend checks"
```

---

### Task 2: Allow Visible Error Dialogs from Prompt Windows

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/capabilities/default.json`

**Step 1: Write the failing capability check**

Add a test to `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs` under `menu_bar_app_tests`:

```rust
#[test]
fn tauri_capabilities_allow_message_dialogs() {
    let capabilities = include_str!("../capabilities/default.json");

    assert!(capabilities.contains("\"dialog:allow-message\""));
}
```

**Step 2: Run test to verify failure**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib menu_bar_app_tests::tauri_capabilities_allow_message_dialogs -- --nocapture
```

Expected: FAIL because current capabilities allow only `dialog:allow-open` and `dialog:allow-save`.

**Step 3: Add the permission**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/capabilities/default.json`, add:

```json
"dialog:allow-message",
```

near the existing dialog permissions:

```json
"dialog:allow-open",
"dialog:allow-save",
"dialog:allow-message",
```

**Step 4: Run test to verify pass**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib menu_bar_app_tests::tauri_capabilities_allow_message_dialogs -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src-tauri/capabilities/default.json src-tauri/src/lib.rs
git commit -m "fix: allow autosend error dialogs"
```

---

### Task 3: Move Accessibility Permission Guard into Backend Autosend

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`

**Step 1: Write failing tests**

Add these tests in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs` inside the existing `#[cfg(test)] mod tests` block:

```rust
#[test]
fn autosend_preflight_rejects_missing_accessibility_permission() {
    let err = ensure_accessibility_trusted_for_autosend(false).unwrap_err();

    assert!(err.contains("Accessibility permission required"));
    assert!(err.contains("System Settings"));
}

#[test]
fn autosend_preflight_allows_trusted_accessibility_permission() {
    assert!(ensure_accessibility_trusted_for_autosend(true).is_ok());
}
```

**Step 2: Run tests to verify failure**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib platform::macos::tests::autosend_preflight -- --nocapture
```

Expected: FAIL because `ensure_accessibility_trusted_for_autosend` does not exist.

**Step 3: Implement the guard**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`, add:

```rust
fn ensure_accessibility_trusted_for_autosend(trusted: bool) -> Result<(), String> {
    if trusted {
        Ok(())
    } else {
        Err(
            "Accessibility permission required for autosend. Enable Prompt Picker in System Settings > Privacy & Security > Accessibility, then try again."
                .to_string(),
        )
    }
}
```

At the top of `type_or_paste_prompt_and_submit_to_app`, add:

```rust
ensure_accessibility_trusted_for_autosend(is_accessibility_trusted())?;
```

**Step 4: Run tests to verify pass**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib platform::macos::tests::autosend_preflight -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src-tauri/src/platform/macos.rs
git commit -m "fix: guard autosend with backend accessibility check"
```

---

### Task 4: Add OpenWhip-Style Conditional Focus Restore

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`

**Step 1: Write failing tests**

Add these tests in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`:

```rust
#[test]
fn cmd_tab_refocus_script_matches_openwhip_pattern() {
    let script = cmd_tab_refocus_previous_app_script();

    assert!(script.contains("tell application \"System Events\""));
    assert!(script.contains("key down command"));
    assert!(script.contains("key code 48"));
    assert!(script.contains("key up command"));
}

#[test]
fn should_cmd_tab_refocus_only_when_prompt_picker_is_frontmost() {
    assert!(should_cmd_tab_refocus_before_autosend(Some(&FrontmostApp {
        name: "Prompt Picker".to_string(),
        bundle_id: "local.promptpicker.dev".to_string(),
    })));

    assert!(!should_cmd_tab_refocus_before_autosend(Some(&FrontmostApp {
        name: "Codex".to_string(),
        bundle_id: "com.openai.codex".to_string(),
    })));

    assert!(!should_cmd_tab_refocus_before_autosend(None));
}
```

**Step 2: Run tests to verify failure**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib platform::macos::tests::cmd_tab_refocus -- --nocapture
```

Expected: FAIL because the helper functions do not exist.

**Step 3: Implement helper functions**

Add these helpers near the existing script builders in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`:

```rust
fn should_cmd_tab_refocus_before_autosend(frontmost: Option<&FrontmostApp>) -> bool {
    frontmost
        .map(|app| app.bundle_id == "local.promptpicker.dev" || app.name == "Prompt Picker")
        .unwrap_or(false)
}

fn cmd_tab_refocus_previous_app_script() -> &'static str {
    r#"tell application "System Events"
    key down command
    key code 48
    key up command
end tell"#
}
```

Add this runtime helper:

```rust
fn restore_focus_before_autosend(bundle_id: &str) -> Result<(), String> {
    let frontmost = frontmost_app();
    if should_cmd_tab_refocus_before_autosend(frontmost.as_ref()) {
        let output = Command::new("osascript")
            .arg("-e")
            .arg(cmd_tab_refocus_previous_app_script())
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(format_autosend_error(
                "cmd-tab-refocus",
                String::from_utf8_lossy(&output.stderr).as_ref(),
            ));
        }
    }

    let script = format!(r#"tell application id "{}" to activate"#, bundle_id);
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format_autosend_error(
            "activate-target",
            String::from_utf8_lossy(&output.stderr).as_ref(),
        ));
    }

    std::thread::sleep(std::time::Duration::from_millis(120));
    Ok(())
}
```

Call it inside `type_or_paste_prompt_and_submit_to_app` after the backend accessibility guard and before direct-type/paste:

```rust
restore_focus_before_autosend(bundle_id)?;
```

**Important:** Do not run Cmd+Tab unconditionally. The popover is configured as a non-activating panel in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/macos_panels.rs`; blindly Cmd+Tab can move away from the real target app when the target app is still frontmost.

**Step 4: Run tests to verify pass**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib platform::macos::tests::cmd_tab_refocus -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src-tauri/src/platform/macos.rs
git commit -m "fix: restore target focus before autosend"
```

---

### Task 5: Keep Keyboard Autosend Fallbacks and Avoid Coordinate Dependency

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`

**Step 1: Verify existing tests still express the default path**

Confirm these tests exist in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`:

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

Confirm this test exists in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`:

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

**Step 2: Add a regression test for the app-level autosend branch**

Add a pure test in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs` if it does not already exist:

```rust
#[test]
fn last_target_for_wechat_does_not_need_click_point() {
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

**Step 3: Ensure default branch remains coordinate-free**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`, keep `paste_prompt_and_submit_to_last_target_impl` as:

```rust
fn paste_prompt_and_submit_to_last_target_impl(
    body: &str,
    state: &LastInputTargetState,
) -> Result<(), String> {
    let Some(target) = state.get() else {
        return Err("Click into a text field first, then choose a prompt.".to_string());
    };
    platform::macos::type_or_paste_prompt_and_submit_to_app(body, &target.app.bundle_id)
}
```

Do not branch on `target.click_point` in the default autosend path.

**Step 4: Run tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib last_input_target_tests::autosend_does_not_require_click_point_for_codex last_input_target_tests::last_target_for_wechat_does_not_need_click_point platform::macos::tests::autosend_direct_script_does_not_click_coordinates platform::macos::tests::paste_and_submit_script_remains_available_as_fallback -- --nocapture
```

If Cargo rejects multiple test-name filters, run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib autosend -- --nocapture
cargo test --lib fallback -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src-tauri/src/lib.rs src-tauri/src/platform/macos.rs
git commit -m "test: keep autosend coordinate-free by default"
```

---

### Task 6: Add an Explicit User-Facing Permission Recovery Path

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`

**Step 1: Write failing test**

Add this test in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`:

```tsx
it("shows an actionable permission message when backend autosend reports accessibility failure", async () => {
  const { invoke } = await import("@tauri-apps/api/core");
  const { message } = await import("@tauri-apps/plugin-dialog");
  vi.mocked(invoke).mockClear();
  vi.mocked(message).mockClear();
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    if (command === "paste_prompt_and_submit_to_last_target") {
      throw new Error(
        "Accessibility permission required for autosend. Enable Prompt Picker in System Settings > Privacy & Security > Accessibility, then try again."
      );
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
    expect(vi.mocked(message)).toHaveBeenCalledWith(
      expect.stringContaining("System Settings > Privacy & Security > Accessibility"),
      { title: "Prompt Picker", kind: "error" }
    );
  });
});
```

**Step 2: Run test to verify failure or coverage**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "permission message"
```

Expected: FAIL before Task 1 implementation, PASS after Task 1 if the error dialog path is correctly implemented.

**Step 3: Keep implementation minimal**

If Task 1 already implemented `message(...)` in the catch block, do not add extra UI. Do not add a new settings screen or custom modal unless `message(...)` cannot be enabled with `dialog:allow-message`.

**Step 4: Run test to verify pass**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "permission message"
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src/App.tsx src/app/App.test.tsx
git commit -m "test: show actionable autosend permission errors"
```

---

### Task 7: Full Verification and Packaging

**Files:**
- Read: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/tauri.conf.json`
- Read: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/Info.plist`
- Generated: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app`
- Generated: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/dmg/Prompt Picker_1.0.0_aarch64.dmg`

**Step 1: Run format checks**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo fmt --check
```

Expected: PASS.

**Step 2: Run Rust tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib -- --nocapture
```

Expected: PASS.

**Step 3: Run frontend tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test
```

Expected: PASS.

**Step 4: Build frontend**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm run build
```

Expected: PASS.

**Step 5: Package App**

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

**Step 6: Verify menu-bar app setting**

Run:

```bash
plutil -p "/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app/Contents/Info.plist" | rg "LSUIElement|CFBundleIdentifier"
```

Expected:

```text
"CFBundleIdentifier" => "local.promptpicker.dev"
"LSUIElement" => true
```

**Step 7: Verify build contains new code**

Run:

```bash
strings "/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app/Contents/MacOS/prompt-picker" | rg "cmd-tab-refocus|activate-target|Accessibility permission required for autosend|direct-type"
```

Expected: all key strings appear.

**Step 8: Restart rebuilt app**

Run:

```bash
pkill -f "/Prompt Picker.app/Contents/MacOS/prompt-picker" || true
open "/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app"
sleep 2
pgrep -afil "/Prompt Picker.app/Contents/MacOS/prompt-picker|prompt-picker"
```

Expected: exactly one current Prompt Picker app process is running from the rebuilt bundle path.

**Step 9: Note signing limitation**

Run:

```bash
codesign -dv --verbose=4 "/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app"
```

Expected: local build may still be adhoc signed. Final user-facing notes must say macOS Accessibility permission may need to be re-granted after replacing the app.

**Step 10: Do not commit generated artifacts unless explicitly requested**

Do not add `dist`, `node_modules`, `src-tauri/target`, `.app`, or `.dmg` changes unless the user explicitly asks for generated artifacts to be committed.

---

### Task 8: Manual Smoke Checklist

**Files:**
- No code changes.

**Step 1: Permission state**

Manual check:

```text
System Settings > Privacy & Security > Accessibility > Prompt Picker enabled
```

Expected: enabled. If it was already enabled but behavior still fails after replacing the app, remove Prompt Picker from the list, add the rebuilt app again, then enable it.

**Step 2: Codex normal path**

Manual steps:

```text
1. Click into the Codex input box so the cursor is visible.
2. Click Calico.
3. Click "讨论方案".
```

Expected:

```text
Popover disappears immediately.
Codex receives the prompt text.
Prompt is submitted with Enter.
```

**Step 3: WeChat normal path**

Manual steps:

```text
1. Click into a WeChat chat input box so the cursor is visible.
2. Click Calico.
3. Click "讨论方案".
```

Expected:

```text
Popover disappears immediately.
WeChat receives the prompt text.
Message is sent with Enter.
```

**Step 4: Permission failure path**

Manual steps:

```text
1. Disable Prompt Picker in Accessibility.
2. Click a target input box.
3. Click Calico.
4. Click "讨论方案".
```

Expected:

```text
Popover disappears immediately.
No text is sent.
Visible error dialog explains Accessibility permission is required.
```

**Step 5: No target path**

Manual steps:

```text
1. Start the app fresh.
2. Do not click any target input.
3. Click Calico.
4. Click "讨论方案".
```

Expected:

```text
Popover disappears immediately.
Visible error explains that the user should click into a text field first.
```

---

### Task 9: Commit and Push

**Files:**
- Source files changed in previous tasks only.

**Step 1: Inspect status**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git status --short
git diff --cached --stat
git diff --check
```

Expected: source files and tests only; no generated build artifacts staged.

**Step 2: Commit final source changes if any task left staged work**

Run:

```bash
git add src/App.tsx src/app/App.test.tsx src-tauri/src/platform/macos.rs src-tauri/src/lib.rs src-tauri/capabilities/default.json
git commit -m "fix: make autosend focus restore visible and reliable"
```

Expected: commit succeeds. If all task commits already happened, this step should be skipped.

**Step 3: Push only after user requests execution**

Run only after explicit execution request:

```bash
git push origin main
```

Expected: `main` on GitHub receives the new commits.

---

## Risk Controls

- Do not blindly Cmd+Tab. The current prompt popover is a non-activating panel, so unconditional Cmd+Tab can move focus away from the correct target app.
- Do not put the Accessibility check back in the frontend before `hidePromptPopover`; that recreates the "click did nothing" symptom.
- Keep paste fallback for multiline and long prompts.
- Keep coordinate-click functions available but not in the default autosend path.
- Keep Prompt Picker menu-bar behavior unchanged: `LSUIElement` must remain true.
- Treat adhoc signing as a runtime permission risk and document the need to re-grant Accessibility permission after replacing the app.

## Success Criteria

- Clicking any prompt always closes the prompt popover first.
- If autosend cannot run because of Accessibility permission, the user sees a native error dialog.
- Codex and WeChat autosend do not depend on input coordinates.
- OpenWhip-style Cmd+Tab restore exists but only runs when Prompt Picker actually became frontmost.
- Short single-line prompts use direct keyboard input and Enter.
- Long or multiline prompts use paste fallback and Enter.
- Existing prompt management, settings, menu-bar app behavior, and Calico visibility still pass tests.
