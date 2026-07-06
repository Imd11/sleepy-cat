# Autosend Target Recovery Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restore the old Codex experience where an open Codex window can receive prompts without the user first focusing the input box, while keeping copy-only safety when the target is genuinely unsafe.

**Architecture:** Capture the target before opening Prompt Picker UI, keep the focus-preserving path when the target remains frontmost, and add a controlled target-recovery fallback only when Prompt Picker itself stole frontmost status. If the user or system switched to another non-target app, keep the current safe copy-only behavior.

**Tech Stack:** Tauri v2, Rust macOS native event injection, macOS Accessibility/System Events, React, Vite, Vitest, Cargo tests.

**Critical State-Machine Constraint:** `begin_prompt_pick_session` must own session startup before it captures a target, and opening the popover must not clear an already-captured target for the same `sessionId`. Otherwise the plan can capture a target and immediately erase it.

---

### Task 1: Make Prompt Session Capture Own The Session Lifecycle

**Files:**
- Modify: `src-tauri/src/lib.rs:107-132`
- Modify: `src-tauri/src/lib.rs:777-798`
- Modify: `src-tauri/src/windows.rs:537-579`
- Modify: `public/overlay.html:510-521`
- Modify: `src/overlay/overlayHtml.test.ts:179-200`

**Step 1: Write the failing test**

Add a Rust state test near the existing `cloned_prompt_pick_session_state_shares_target` test:

```rust
#[test]
fn begin_if_new_preserves_target_for_current_session() {
    let state = PromptPickSessionState::default();
    state.begin(7);
    state.set(PromptPickSessionTarget {
        app: FrontmostApp {
            name: "Codex".to_string(),
            bundle_id: "com.openai.codex".to_string(),
        },
        pid: None,
        observed_at_ms: now_ms(),
        click_point: Some(TargetClickPoint { x: 640.0, y: 720.0 }),
    });

    state.begin_if_new(7);
    assert_eq!(state.get().unwrap().app.bundle_id, "com.openai.codex");

    state.begin_if_new(8);
    assert!(state.get().is_none());
}
```

Add a source-level test near `autosend_and_prompt_capture_commands_use_spawn_blocking`:

```rust
#[test]
fn prompt_capture_command_begins_session_before_recording_target() {
    let source = include_str!("lib.rs");
    let start = source
        .find("async fn begin_prompt_pick_session")
        .expect("begin_prompt_pick_session should exist");
    let end = source[start..]
        .find("#[tauri::command]\nfn paste_prompt")
        .expect("next command should follow begin_prompt_pick_session");
    let command_source = &source[start..start + end];

    assert!(command_source.contains("session_state.begin(session_id);"));
    assert!(command_source.contains("record_prompt_pick_session_target_if_valid"));
}
```

Add a source-level test in `src-tauri/src/windows.rs` next to the existing prompt popover tests:

```rust
#[test]
fn prompt_popover_open_preserves_existing_session_capture() {
    let source = include_str!("windows.rs");
    let start = source
        .find("pub fn toggle_prompt_popover_from_button")
        .expect("toggle command should exist");
    let end = source[start..]
        .find("pub fn show_prompt_button_controls_from_button")
        .expect("next command should follow toggle");
    let command_source = &source[start..start + end];

    assert!(command_source.contains("session_state.begin_if_new(session_id);"));
    assert!(command_source.contains("session_state.begin(session_id);"));
}
```

Replace the current frontend test named `opens the prompt list without awaiting target session capture` with a test that requires target capture before the popover opens:

```ts
it("captures the prompt target before opening the prompt list", () => {
  const html = readOverlayHtml();

  expect(html).toContain("let promptPickSessionId = 0;");
  expect(html).toContain("const sessionId = ++promptPickSessionId;");
  expect(html).toContain("const permission = await invoke('prompt_interaction_permission_status');");
  expect(html).toContain("if (permission?.required && !permission.trusted)");
  expect(html).toContain("await handleMissingPromptInteractionPermission(permission);");
  expect(html).toContain("await invoke('begin_prompt_pick_session', { sessionId });");
  expect(html).toContain("const toggleResult = await invoke('toggle_prompt_popover_from_button', { sessionId });");
  expect(html).not.toContain("const sessionPromise = invoke('begin_prompt_pick_session'");
  expect(html).not.toContain("void sessionPromise.catch");
  expect(html.indexOf("begin_prompt_pick_session")).toBeLessThan(
    html.indexOf("toggle_prompt_popover_from_button")
  );
});
```

**Step 2: Run the failing test**

Run:

```bash
npm test -- src/overlay/overlayHtml.test.ts
cd src-tauri
cargo test --lib begin_if_new_preserves_target_for_current_session
cargo test --lib prompt_capture_command_begins_session_before_recording_target
cargo test --lib prompt_popover_open_preserves_existing_session_capture
```

Expected: FAIL because the frontend currently opens the popover first, `begin_prompt_pick_session` does not start the session, and the popover open path currently clears any existing target.

**Step 3: Implement minimal code**

Add this method to `PromptPickSessionState`:

```rust
pub fn begin_if_new(&self, session_id: u64) {
    let mut state = self.0.lock().expect("prompt pick session lock poisoned");
    if state.active_session_id == session_id {
        return;
    }
    state.active_session_id = session_id;
    state.target = None;
}
```

Update `begin_prompt_pick_session` so it starts the session before the blocking capture:

```rust
let session_state = session_state.inner().clone();
let recent_state = recent_state.inner().clone();
session_state.begin(session_id);
```

Keep the existing `set_if_current` guard. It is still needed to reject stale async capture results if a newer session begins.

Update the popover open paths:

```rust
// Opening should preserve an already-captured target for the same session.
session_state.begin_if_new(session_id);
```

Use `begin_if_new` in `show_prompt_popover_from_button` and in the opening branch of `toggle_prompt_popover_from_button`. Keep `session_state.begin(session_id)` in the close branch, because closing should clear the prompt-pick target.

Change the non-drag click block in `public/overlay.html` so the target session capture completes before the list opens:

```js
const sessionId = ++promptPickSessionId;
await invoke('begin_prompt_pick_session', { sessionId }).catch(() => null);
const toggleResult = await invoke('toggle_prompt_popover_from_button', { sessionId });
if (!toggleResult?.opened) {
  resetCalicoMotion();
}
```

Keep the permission check before this block. Do not change drag behavior.

**Step 4: Run test to verify it passes**

Run:

```bash
npm test -- src/overlay/overlayHtml.test.ts
cd src-tauri
cargo test --lib begin_if_new_preserves_target_for_current_session
cargo test --lib prompt_capture_command_begins_session_before_recording_target
cargo test --lib prompt_popover_open_preserves_existing_session_capture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add public/overlay.html src/overlay/overlayHtml.test.ts src-tauri/src/lib.rs src-tauri/src/windows.rs
git commit -m "fix: capture prompt target before opening picker"
```

---

### Task 2: Add Explicit Frontmost Classification

**Files:**
- Modify: `src-tauri/src/lib.rs:961-979`
- Test: `src-tauri/src/lib.rs` in `mod last_input_target_tests`

**Step 1: Write failing tests**

Add tests next to the existing `captured_target_matches_frontmost` tests:

```rust
#[test]
fn classifies_frontmost_target_status() {
    let target = PromptPickSessionTarget {
        app: FrontmostApp {
            name: "Codex".to_string(),
            bundle_id: "com.openai.codex".to_string(),
        },
        pid: Some(123),
        observed_at_ms: now_ms(),
        click_point: None,
    };

    assert_eq!(
        classify_target_frontmost(
            &target,
            Some(&frontmost_target("Codex", "com.openai.codex", Some(123)))
        ),
        TargetFrontmostStatus::Target
    );
    assert_eq!(
        classify_target_frontmost(
            &target,
            Some(&frontmost_target("Prompt Picker", "local.promptpicker.dev", Some(1)))
        ),
        TargetFrontmostStatus::PromptPicker
    );
    assert_eq!(
        classify_target_frontmost(
            &target,
            Some(&frontmost_target("Notes", "com.apple.Notes", Some(456)))
        ),
        TargetFrontmostStatus::OtherOrUnknown
    );
    assert_eq!(
        classify_target_frontmost(&target, None),
        TargetFrontmostStatus::OtherOrUnknown
    );
}
```

**Step 2: Run the failing Rust test**

Run:

```bash
cd src-tauri
cargo test --lib last_input_target_tests::classifies_frontmost_target_status
```

Expected: FAIL because `TargetFrontmostStatus` and `classify_target_frontmost` do not exist yet.

**Step 3: Implement minimal code**

Add this enum near `captured_target_matches_frontmost`:

```rust
#[derive(Debug, PartialEq, Eq)]
enum TargetFrontmostStatus {
    Target,
    PromptPicker,
    OtherOrUnknown,
}
```

Add this helper below `captured_target_matches_frontmost`:

```rust
fn classify_target_frontmost(
    target: &PromptPickSessionTarget,
    frontmost: Option<&FrontmostAppWithPid>,
) -> TargetFrontmostStatus {
    let Some(frontmost) = frontmost else {
        return TargetFrontmostStatus::OtherOrUnknown;
    };
    if captured_target_matches_frontmost(target, Some(frontmost)) {
        return TargetFrontmostStatus::Target;
    }
    if is_prompt_picker_app(&frontmost.app) {
        return TargetFrontmostStatus::PromptPicker;
    }
    TargetFrontmostStatus::OtherOrUnknown
}
```

**Step 4: Run the test**

Run:

```bash
cd src-tauri
cargo test --lib last_input_target_tests::classifies_frontmost_target_status
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "test: classify autosend frontmost target state"
```

---

### Task 3: Expose A Safe Target-Recovery Primitive On macOS

**Files:**
- Modify: `src-tauri/src/platform/macos.rs:871-951`
- Test: `src-tauri/src/platform/macos.rs` tests near `paste_and_submit_to_app_script_activates_target_pastes_and_presses_return`

**Step 1: Write failing tests**

Add a source-level test that requires a public recovery function to exist and to use the same existing primitives as the old successful Codex path:

```rust
#[test]
fn target_recovery_function_activates_waits_and_clicks_optional_point() {
    let source = include_str!("macos.rs");
    let start = source
        .find("pub fn recover_target_app_for_autosend")
        .expect("target recovery function should exist");
    let end = source[start..]
        .find("fn paste_and_submit_to_app_script")
        .expect("next helper should exist");
    let recovery_source = &source[start..start + end];

    assert!(recovery_source.contains("activate_app_by_bundle_id"));
    assert!(recovery_source.contains("wait_for_frontmost_bundle_id"));
    assert!(recovery_source.contains("click_target_point"));
    assert!(recovery_source.contains("Duration::from_millis(1_500)"));
}
```

**Step 2: Run the failing test**

Run:

```bash
cd src-tauri
cargo test --lib platform::macos::tests::target_recovery_function_activates_waits_and_clicks_optional_point
```

Expected: FAIL because the public recovery primitive does not exist.

**Step 3: Implement minimal code**

Add this public helper before `paste_and_submit_to_app_script`:

```rust
pub fn recover_target_app_for_autosend(
    bundle_id: &str,
    click_point: Option<(f64, f64)>,
) -> Result<(), String> {
    if let Err(error) = activate_app_by_bundle_id(bundle_id) {
        return Err(format_autosend_error("activate-target", &error));
    }
    if !wait_for_frontmost_bundle_id(bundle_id, Duration::from_millis(1_500)) {
        return Err(format!(
            "Target app did not become frontmost: {}",
            bundle_id
        ));
    }
    std::thread::sleep(Duration::from_millis(160));
    if let Some((x, y)) = click_point {
        if let Err(error) = click_target_point(x, y) {
            return Err(format_autosend_error("click-input-target", &error));
        }
        std::thread::sleep(Duration::from_millis(120));
    }
    Ok(())
}
```

This intentionally uses existing primitives. Do not add app-specific branches.

**Step 4: Run the test**

Run:

```bash
cd src-tauri
cargo test --lib platform::macos::tests::target_recovery_function_activates_waits_and_clicks_optional_point
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/platform/macos.rs
git commit -m "feat: add safe autosend target recovery primitive"
```

---

### Task 4: Wire Target Recovery Into Single-Prompt Autosend

**Files:**
- Modify: `src-tauri/src/lib.rs:248-380`
- Modify: `src-tauri/src/lib.rs:501-567`
- Test: `src-tauri/src/lib.rs` in `mod last_input_target_tests`

**Step 1: Write failing tests**

Add tests for the three important paths:

```rust
#[test]
fn autosend_recovers_app_only_target_with_recorded_click_point_when_picker_is_frontmost() {
    let target = PromptPickSessionTarget {
        app: FrontmostApp {
            name: "Codex".to_string(),
            bundle_id: "com.openai.codex".to_string(),
        },
        pid: None,
        observed_at_ms: now_ms(),
        click_point: Some(TargetClickPoint { x: 640.0, y: 720.0 }),
    };
    let mut frontmost = vec![
        Some(frontmost_target("Prompt Picker", "local.promptpicker.dev", Some(1))),
        Some(frontmost_target("Codex", "com.openai.codex", Some(42))),
        Some(frontmost_target("Codex", "com.openai.codex", Some(42))),
    ]
    .into_iter();
    let mut recovered = false;
    let mut pasted = false;
    let mut submitted = false;

    let outcome = guarded_focus_preserving_autosend_with_senders(
        "hello",
        &target,
        platform::macos::NativeSubmitKey::Enter,
        |_| Ok(()),
        || frontmost.next().flatten(),
        |recover_target| {
            recovered = true;
            assert_eq!(recover_target.app.bundle_id, "com.openai.codex");
            assert_eq!(recover_target.click_point.unwrap().x, 640.0);
            Ok(())
        },
        || {
            pasted = true;
            Ok(())
        },
        |_| {
            submitted = true;
            Ok(())
        },
        |_| {},
    );

    assert!(outcome.sent);
    assert!(recovered);
    assert!(pasted);
    assert!(submitted);
}

#[test]
fn autosend_refuses_when_another_app_is_frontmost_before_paste() {
    let target = PromptPickSessionTarget {
        app: FrontmostApp {
            name: "Codex".to_string(),
            bundle_id: "com.openai.codex".to_string(),
        },
        pid: Some(42),
        observed_at_ms: now_ms(),
        click_point: None,
    };
    let outcome = guarded_focus_preserving_autosend_with_senders(
        "hello",
        &target,
        platform::macos::NativeSubmitKey::Enter,
        |_| Ok(()),
        || Some(frontmost_target("Notes", "com.apple.Notes", Some(9))),
        |_| panic!("must not recover when a third-party app is frontmost"),
        || panic!("must not paste into the wrong app"),
        |_| panic!("must not submit into the wrong app"),
        |_| {},
    );

    assert_eq!(outcome.reason, Some(platform::macos::AutosendFailureReason::NoSafeTarget));
    assert!(outcome.copied);
    assert!(!outcome.sent);
}

#[test]
fn autosend_refuses_when_prompt_picker_recovery_does_not_restore_target() {
    let target = PromptPickSessionTarget {
        app: FrontmostApp {
            name: "Codex".to_string(),
            bundle_id: "com.openai.codex".to_string(),
        },
        pid: Some(42),
        observed_at_ms: now_ms(),
        click_point: None,
    };
    let mut frontmost = vec![
        Some(frontmost_target("Prompt Picker", "local.promptpicker.dev", Some(1))),
        Some(frontmost_target("Prompt Picker", "local.promptpicker.dev", Some(1))),
    ]
    .into_iter();
    let outcome = guarded_focus_preserving_autosend_with_senders(
        "hello",
        &target,
        platform::macos::NativeSubmitKey::Enter,
        |_| Ok(()),
        || frontmost.next().flatten(),
        |_| Ok(()),
        || panic!("must not paste before target is restored"),
        |_| panic!("must not submit before target is restored"),
        |_| {},
    );

    assert_eq!(outcome.reason, Some(platform::macos::AutosendFailureReason::NoSafeTarget));
    assert!(outcome.copied);
    assert!(!outcome.sent);
}
```

**Step 2: Run the failing tests**

Run:

```bash
cd src-tauri
cargo test --lib autosend_recovers_app_only_target_with_recorded_click_point_when_picker_is_frontmost
cargo test --lib autosend_refuses_when_another_app_is_frontmost_before_paste
cargo test --lib autosend_refuses_when_prompt_picker_recovery_does_not_restore_target
```

Expected: FAIL because the existing guard treats Prompt Picker the same as every other mismatch.

**Step 3: Implement minimal code**

Rename the `focus_repair` generic in `guarded_focus_preserving_autosend_with_senders` to a target recovery callback:

```rust
R: FnMut(&PromptPickSessionTarget) -> Result<(), String>,
```

Use `classify_target_frontmost` before paste:

```rust
let before_paste = frontmost_reader();
match classify_target_frontmost(target, before_paste.as_ref()) {
    TargetFrontmostStatus::Target => {}
    TargetFrontmostStatus::PromptPicker => {
        if recover_target(target).is_err() {
            return AutosendOutcome::copied_without_send(
                "Target app changed before paste; prompt was copied instead.".to_string(),
            );
        }
        let after_recovery = frontmost_reader();
        if !captured_target_matches_frontmost(target, after_recovery.as_ref()) {
            return AutosendOutcome::copied_without_send(
                "Target app changed before paste; prompt was copied instead.".to_string(),
            );
        }
    }
    TargetFrontmostStatus::OtherOrUnknown => {
        return AutosendOutcome::copied_without_send(
            "Target app changed before paste; prompt was copied instead.".to_string(),
        );
    }
}
```

Update `paste_prompt_and_submit_to_last_target_impl` to pass a real recovery function:

```rust
recover_target_for_autosend,
```

Add this helper near `repair_target_focus`:

```rust
fn recover_target_for_autosend(target: &PromptPickSessionTarget) -> Result<(), String> {
    platform::macos::recover_target_app_for_autosend(
        &target.app.bundle_id,
        target.click_point.map(|point| (point.x, point.y)),
    )?;
    if target.click_point.is_none() {
        repair_target_focus(target)?;
    }
    Ok(())
}
```

This restores the old Codex behavior when a recorded click point exists, even when the captured target has no pid. If no click point is available, the path remains conservative and requires AX repair; if AX repair cannot run, the caller falls back to copy-only.

**Step 4: Run tests**

Run:

```bash
cd src-tauri
cargo test --lib autosend_recovers_app_only_target_with_recorded_click_point_when_picker_is_frontmost
cargo test --lib autosend_refuses_when_another_app_is_frontmost_before_paste
cargo test --lib autosend_refuses_when_prompt_picker_recovery_does_not_restore_target
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "fix: recover autosend target when picker stole focus"
```

---

### Task 5: Apply The Same Recovery Rule To Prompt Groups

**Files:**
- Modify: `src-tauri/src/lib.rs:383-499`
- Test: `src-tauri/src/lib.rs` in `mod last_input_target_tests`

**Step 1: Write failing sequence test**

Add a group test that starts with Prompt Picker frontmost, recovers once, then sends all group items:

```rust
#[test]
fn sequence_autosend_recovers_target_when_prompt_picker_is_frontmost() {
    let target = PromptPickSessionTarget {
        app: FrontmostApp {
            name: "Codex".to_string(),
            bundle_id: "com.openai.codex".to_string(),
        },
        pid: Some(42),
        observed_at_ms: now_ms(),
        click_point: Some(TargetClickPoint { x: 640.0, y: 720.0 }),
    };
    let bodies = vec!["one".to_string(), "two".to_string()];
    let mut frontmost = vec![
        Some(frontmost_target("Prompt Picker", "local.promptpicker.dev", Some(1))),
        Some(frontmost_target("Codex", "com.openai.codex", Some(42))),
        Some(frontmost_target("Codex", "com.openai.codex", Some(42))),
        Some(frontmost_target("Codex", "com.openai.codex", Some(42))),
    ]
    .into_iter();
    let mut recovered_count = 0;
    let mut pasted_count = 0;
    let mut submitted_count = 0;

    let outcome = focus_preserving_prompt_sequence_for_target_with_senders(
        &bodies,
        700,
        &target,
        platform::macos::NativeSubmitKey::Enter,
        |_| Ok(()),
        || frontmost.next().flatten(),
        |_| {
            recovered_count += 1;
            Ok(())
        },
        || {
            pasted_count += 1;
            Ok(())
        },
        |_| {
            submitted_count += 1;
            Ok(())
        },
        |_| {},
    );

    assert!(outcome.sent);
    assert_eq!(outcome.sent_count, 2);
    assert_eq!(recovered_count, 1);
    assert_eq!(pasted_count, 2);
    assert_eq!(submitted_count, 2);
}
```

**Step 2: Run the failing test**

Run:

```bash
cd src-tauri
cargo test --lib last_input_target_tests::sequence_autosend_recovers_target_when_prompt_picker_is_frontmost
```

Expected: FAIL until the sequence path passes the recovery callback through consistently.

**Step 3: Implement minimal code**

Update `focus_preserving_prompt_sequence_to_last_target_impl` and `focus_preserving_prompt_sequence_for_target_with_senders` so the same recovery callback used by single prompt autosend is passed into every `guarded_focus_preserving_autosend_with_senders` call.

Do not create a separate group-only recovery path.

**Step 4: Run test**

Run:

```bash
cd src-tauri
cargo test --lib last_input_target_tests::sequence_autosend_recovers_target_when_prompt_picker_is_frontmost
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "fix: recover autosend target for prompt groups"
```

---

### Task 6: Update Tests That Intentionally Forbid Activation In The Main Path

**Files:**
- Modify: `src-tauri/src/platform/macos.rs:1349-1365`

**Step 1: Write the updated expectation**

The old test `focus_preserving_autosend_main_path_does_not_activate_or_click` is now too broad. Replace it with a narrower test:

```rust
#[test]
fn pure_focus_preserving_event_sender_does_not_activate_or_click() {
    let source = include_str!("macos.rs");
    let start = source
        .find("pub fn focus_preserving_paste_and_submit")
        .expect("focus-preserving autosend function should exist");
    let end = source[start..]
        .find("pub fn recover_target_app_for_autosend")
        .expect("target recovery function should follow pure sender");
    let pure_sender_source = &source[start..start + end];

    assert!(!pure_sender_source.contains("activate_app_by_bundle_id"));
    assert!(!pure_sender_source.contains("click_target_point"));
    assert!(pure_sender_source.contains("post_focus_preserving_paste"));
    assert!(pure_sender_source.contains("post_focus_preserving_submit_key"));
}
```

**Step 2: Run the test**

Run:

```bash
cd src-tauri
cargo test --lib platform::macos::tests::pure_focus_preserving_event_sender_does_not_activate_or_click
```

Expected: PASS after the new recovery helper is added and the pure sender remains unchanged.

**Step 3: Commit**

```bash
git add src-tauri/src/platform/macos.rs
git commit -m "test: scope focus-preserving autosend activation guard"
```

---

### Task 7: End-To-End Verification

**Files:**
- No code changes unless verification exposes a real bug.

**Step 1: Run frontend tests**

```bash
npm test
```

Expected: PASS.

**Step 2: Run Rust tests**

```bash
cd src-tauri
cargo test
```

Expected: PASS.

**Step 3: Run production build**

```bash
npm run build
```

Expected: TypeScript and Vite build succeed.

**Step 4: Build the Mac app for manual verification**

```bash
npm run tauri -- build
```

Expected: App bundle is produced under `src-tauri/target/release/bundle/macos/Prompt Picker.app`.

**Step 5: Manual verification checklist**

Manual scenarios:

```text
Codex:
1. Open Codex so the window is visible.
2. Do not click the input box.
3. Click Calico.
4. Pick a single prompt configured with Enter.
5. Expected: Prompt text is pasted into Codex and submitted.

Claude:
1. Focus Claude input once.
2. Click Calico.
3. Pick a single prompt.
4. Expected: If Claude retained or can recover editable focus, prompt is pasted and submitted.
5. If recovery fails, expected safe fallback is copied-only, not wrong-target submit.

WeChat:
1. Focus a chat input.
2. Click Calico.
3. Pick a prompt.
4. Expected: Prompt is pasted and submitted only when the original chat input can be restored.

Third-app safety:
1. Start from Codex.
2. Click Calico.
3. Before selecting a prompt, switch to another app that is not Prompt Picker.
4. Select a prompt.
5. Expected: No paste/submit into the third app; prompt is copied only.
```

**Step 6: Commit verification docs if needed**

If manual verification notes are recorded, append them to the relevant QA doc and commit:

```bash
git add docs/plans/2026-07-07-autosend-target-recovery.md
git commit -m "docs: record autosend target recovery verification"
```

Do not commit build artifacts.

---

### Task 8: Push To Main

**Files:**
- Git history only.

**Step 1: Confirm source diff**

```bash
git status --short
git log --oneline --decorate -8
```

Expected: only intentional source/test/docs changes are staged or committed; build artifacts are not staged.

**Step 2: Push**

```bash
git push origin main
```

Expected: push succeeds.

---

## User-Visible Result

After this plan is implemented, the user should see:

```text
Codex window open, input box not manually focused
    -> click Calico
    -> choose a prompt
    -> Prompt Picker restores Codex if its own popover stole focus
    -> prompt is pasted and submitted

Prompt Picker stolen focus only
    -> recover target and send

User switched to a different third app
    -> do not send
    -> copy only and show manual paste message
```

This restores the old Codex behavior without making unsafe guesses for unrelated apps.
