# Restore Codex Activating Autosend Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restore stable autosend in Codex by making the backend send to the explicitly captured target app again: copy prompt, activate the captured app, click the recorded/fallback input point when available, paste, then submit according to the configured submit key. Codex must no longer fall back to "Copied. Paste manually." merely because the current focus-preserving frontmost check cannot prove focus safety.

**Architecture:** Keep the existing prompt picker UI and capture model, but change autosend execution from "focus-preserving guarded frontmost path" back to a target-activating sender for the user-selected prompt action. The app still refuses to send when there is no captured usable target, the target is unsafe, or accessibility permission is missing. The existing focus-preserving machinery can remain as a helper or test subject, but it must not be the primary path for `paste_prompt_and_submit_to_last_target` or prompt groups.

**Tech Stack:** Tauri, Rust backend, macOS Accessibility / Quartz keyboard events, React frontend, Vitest, Cargo tests.

---

## Context

The regression is not a status text problem. The user chooses a prompt in Codex and the app reports "Copied. Paste manually.", meaning the backend intentionally refuses to paste/send.

The stable behavior from roughly 26 hours earlier used a simple target-activating path:

1. Use the captured target bundle id.
2. Copy the prompt into the clipboard.
3. Activate the target app by bundle id.
4. Wait until that app is frontmost.
5. Click the recorded point if present.
6. Send `Cmd+V`.
7. Send Return.

The current command entry points still exist in `src-tauri/src/lib.rs`, but `paste_prompt_and_submit_to_last_target_impl()` and `paste_prompt_sequence_and_submit_to_last_target_impl()` call the stricter `focus_preserving_*` path. That path copies first, classifies the current frontmost app, and returns `copied_without_send` when the target cannot be proven frontmost. This is exactly the failure seen in Codex.

Codex is also a poor fit for the AX recovery fallback: local checks showed `System Events` cannot reliably read `AXFocusedUIElement` from the Codex window. Therefore a fix that depends on AX focus repair will remain unstable for Codex.

## Non-Goals

- Do not change the prompt list UI, popover wording, or toast copy.
- Do not add a Codex-only whitelist as the core fix.
- Do not send to an arbitrary visible app when there is no captured prompt target.
- Do not remove accessibility permission checks.
- Do not rewrite the never-key panel work in this plan; it can remain useful, but Codex stability must not depend on that path being perfect.

## Must-Hold Invariants

- A captured usable target is enough to attempt autosend. Do not reintroduce a pre-paste "current frontmost must already be target" gate in the user-facing command path.
- A missing click point is not a failure. The activating sender should still activate the captured app and send `Cmd+V`; it should only click when a point is available.
- Do not depend on `repair_focus_to_editable_element()` or `AXFocusedUIElement` for Codex success. AX repair may remain in older focus-preserving internals, but the restored primary path must not require it.
- Permission must be checked before changing the clipboard. If accessibility is missing, return `MissingAccessibilityPermission` and leave the user's clipboard untouched.
- No captured target means no sending. The app may copy the prompt, but it must not guess a target from the visible-app list.
- If the user switches to another app after clicking a prompt, the command still targets the app captured for that prompt-pick session. This is intentional: the product behavior is "send this prompt to the input I was using when I opened the picker", not "send only if the frontmost app still happens to match after Prompt Picker UI interaction".

---

## Task 1: Capture The Regression With Backend Tests

**Files:**

- `src-tauri/src/lib.rs`
- `src-tauri/src/platform/macos.rs`

**Step 1.1: Add tests proving autosend must use the captured target sender directly.**

In the existing `last_input_target_tests` module in `src-tauri/src/lib.rs`, add or update tests around `paste_prompt_and_submit_to_last_target_impl()` or a testable helper introduced in Task 2. These tests should assert:

- When a `PromptPickSessionTarget` exists for `com.openai.codex`, autosend invokes the app sender with `body`, `bundle_id`, `click_point`, and `submit_key`.
- The path must not call `frontmost_reader`, `recover_target`, `post_focus_preserving_paste`, or `post_focus_preserving_submit_key`.
- The outcome is `sent` when the app sender returns `AutosendOutcome::sent()`.

Expected test shape:

```rust
#[test]
fn autosend_uses_activating_sender_for_captured_codex_target() {
    let state = PromptPickSessionState::default();
    state.set(PromptPickSessionTarget {
        app: FrontmostApp {
            name: "Codex".to_string(),
            bundle_id: "com.openai.codex".to_string(),
        },
        pid: None,
        observed_at_ms: now_ms(),
        click_point: Some(TargetClickPoint { x: 640.0, y: 720.0 }),
    });

    let result = paste_prompt_and_submit_to_session_target_with_senders(
        "hello",
        &state,
        None,
        platform::macos::NativeSubmitKey::Enter,
        |body, bundle_id, click_point, submit_key| {
            assert_eq!(body, "hello");
            assert_eq!(bundle_id, "com.openai.codex");
            assert_eq!(click_point.unwrap().x, 640.0);
            assert_eq!(submit_key, platform::macos::NativeSubmitKey::Enter);
            AutosendOutcome::sent()
        },
        |_| panic!("copy-only fallback must not run when target exists"),
    );

    assert!(result.unwrap().sent);
}
```

The exact helper signature can differ, but the behavior must be enforced.

**Step 1.2: Add tests for the three minimal guardrails.**

Add tests that prove:

- No target: copy first body only and return `NoSafeTarget`.
- Unsafe target: copy only and return an autosend safety failure.
- Untrusted accessibility in the macOS activating sender: no clipboard write before permission, return `MissingAccessibilityPermission`.

**Step 1.3: Add tests proving Codex does not require AX or a click point.**

Add a test where the target app is `com.openai.codex`, `pid` is `None`, and `click_point` is `None`. The app sender must still be invoked:

```rust
#[test]
fn autosend_attempts_codex_target_without_click_point() {
    // target = Codex, click_point = None
    // expected: app_sender receives bundle_id = com.openai.codex and click_point = None
    // expected: no AX repair / frontmost guard / copy-only fallback runs
}
```

**Step 1.4: Add tests for submit key propagation.**

Cover all three `NativeSubmitKey` values:

- `Enter` sends Return after paste.
- `CommandEnter` sends Command+Return after paste.
- `None` pastes but does not submit.

These can be pure Rust tests against a new macOS helper in Task 3 that accepts injectable operations, so the tests do not emit real keyboard events.

**Step 1.5: Run expected failing tests.**

Command:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib autosend_uses_activating_sender_for_captured_codex_target
```

Expected result before implementation: the new test fails because the command path still routes through `focus_preserving_*` and can return copy-only when frontmost classification is not `Target`.

**Commit:**

```bash
git add src-tauri/src/lib.rs src-tauri/src/platform/macos.rs
git commit -m "test: lock codex activating autosend path"
```

---

## Task 2: Route Autosend Commands To The Captured Target Sender

**Files:**

- `src-tauri/src/lib.rs`

**Step 2.1: Change the helper signature for app sender.**

Update `paste_prompt_and_submit_to_session_target_with_senders()` so `app_sender` receives `NativeSubmitKey`:

```rust
A: FnOnce(&str, &str, Option<TargetClickPoint>, platform::macos::NativeSubmitKey) -> AutosendOutcome
```

Do the same for `paste_prompt_sequence_and_submit_to_session_target_with_senders()`:

```rust
A: FnMut(&str, &str, Option<TargetClickPoint>, platform::macos::NativeSubmitKey) -> AutosendOutcome
```

Keep the existing target selection behavior:

- Prefer `PromptPickSessionState::take()`.
- Fall back to recent input target when it is still recent and usable.
- Refuse unsafe or unsupported apps.
- Copy-only if no target exists.

**Step 2.2: Route single prompt autosend to this helper.**

Replace `paste_prompt_and_submit_to_last_target_impl()` internals so it calls `paste_prompt_and_submit_to_session_target_with_senders()` instead of `focus_preserving_prompt_to_last_target_impl()`.

The production sender should call the macOS target-activating function from Task 3:

```rust
platform::macos::paste_prompt_and_submit_to_app_clipboard_with_copier(
    body,
    bundle_id,
    click_point.map(|point| (point.x, point.y)),
    submit_key,
    |text| copy_text_to_clipboard(app, text),
)
```

**Step 2.3: Route prompt groups to this helper.**

Replace `paste_prompt_sequence_and_submit_to_last_target_impl()` internals so it calls `paste_prompt_sequence_and_submit_to_session_target_with_senders()` instead of `focus_preserving_prompt_sequence_to_last_target_impl()`.

Each body should use the same captured target and same submit key. On first failed body, return `AutosendSequenceOutcome::from_failure()` with the failed index. Keep interval clamping.

**Step 2.4: Keep focus-preserving functions but remove them from the primary command path.**

Do not delete `guarded_focus_preserving_autosend_with_senders()` in this task unless it becomes unused and clearly safe to remove. The important behavior is that user-facing autosend commands no longer depend on frontmost classification succeeding.

**Step 2.5: Do not add a replacement frontmost rejection gate.**

The only target identity check in the primary command path should be target selection:

```text
prompt session target or recent target exists
  -> app is usable
  -> app is not unsafe
  -> call activating sender
```

The activating sender may verify that the requested app becomes frontmost after explicit activation. It must not reject just because another app was frontmost before activation.

**Verification:**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib autosend_uses_activating_sender_for_captured_codex_target
cargo test --manifest-path src-tauri/Cargo.toml --lib autosend_sequence_uses_one_session_target_for_all_bodies
```

**Commit:**

```bash
git add src-tauri/src/lib.rs
git commit -m "fix: route autosend through captured target sender"
```

---

## Task 3: Upgrade The macOS Target-Activating Sender

**Files:**

- `src-tauri/src/platform/macos.rs`
- `src-tauri/src/platform/unsupported.rs`

**Step 3.1: Make `paste_prompt_and_submit_to_app_clipboard_with_copier()` first-class again.**

Remove `#[allow(dead_code)]` only if the function is now used by production. Extend its signature:

```rust
pub fn paste_prompt_and_submit_to_app_clipboard_with_copier<C>(
    body: &str,
    bundle_id: &str,
    click_point: Option<(f64, f64)>,
    submit_key: NativeSubmitKey,
    copy_sender: C,
) -> AutosendOutcome
where
    C: FnOnce(&str) -> Result<(), String>,
```

**Step 3.2: Preserve the working order from the stable version.**

The function must do this exact sequence:

1. Check accessibility permission before copying.
2. Copy prompt to clipboard.
3. `recover_target_app_for_autosend(bundle_id, click_point)`.
4. `post_paste_shortcut()`.
5. Sleep for a short settle delay, around 220 ms.
6. If `submit_key == NativeSubmitKey::None`, return `AutosendOutcome::sent()`.
7. Otherwise call `post_focus_preserving_submit_key(submit_key)`.
8. Return `AutosendOutcome::sent()`.

Use the existing `post_focus_preserving_submit_key()` because it already supports `None`, `Enter`, and `CommandEnter`.

Important details:

- `recover_target_app_for_autosend()` already treats `click_point: None` as "activate and wait only"; keep that behavior.
- Do not call `repair_focus_to_editable_element()` from this sender. Codex success must not depend on AX.
- Do not run `frontmost_app_with_pid()` before paste as a safety gate. The only frontmost wait should be the post-activation wait inside `recover_target_app_for_autosend()`.

**Step 3.3: Keep error mapping precise.**

- Clipboard failure -> `AutosendOutcome::copy_failed`.
- Activation/click failure -> `AutosendOutcome::target_focus_failed`.
- Paste event failure -> `AutosendOutcome::paste_event_failed`.
- Submit event failure -> `AutosendOutcome::return_event_failed`.

**Step 3.4: Update unsupported platform stub.**

Make `src-tauri/src/platform/unsupported.rs` match the new function signature and keep its current copy-only behavior.

**Step 3.5: Add pure tests around source/order or injected helper.**

Prefer extracting an internal helper that accepts operation closures:

```rust
fn paste_prompt_and_submit_to_app_clipboard_with_ops<C, R, P, S, W>(...)
```

Then test the operation log:

```text
permission -> copy -> recover -> paste -> sleep -> submit
```

Add tests:

- `activating_clipboard_sender_pastes_and_presses_return`
- `activating_clipboard_sender_respects_command_enter`
- `activating_clipboard_sender_respects_submit_key_none`
- `activating_clipboard_sender_does_not_copy_without_accessibility_permission`
- `activating_clipboard_sender_clicks_recorded_point_before_paste`
- `activating_clipboard_sender_pastes_without_click_point`
- `activating_clipboard_sender_does_not_call_ax_repair`

**Verification:**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib activating_clipboard_sender
```

**Commit:**

```bash
git add src-tauri/src/platform/macos.rs src-tauri/src/platform/unsupported.rs
git commit -m "fix: restore target activating mac autosend sender"
```

---

## Task 4: Remove The Arbitrary Visible-App Autosend Fallback

**Files:**

- `src-tauri/src/lib.rs`

**Why:** Restoring aggressive target activation fixes Codex, but it makes target selection more important. If Prompt Picker is frontmost and there is no current/recent target, choosing the first visible non-Prompt-Picker app is too broad. It can send to the wrong app.

**Step 4.1: Tighten `prompt_pick_session_target()`.**

When frontmost is Prompt Picker:

- Use a recent target only if it is recent and usable.
- Do not pick `visible_apps.into_iter().find(...)` as an autosend target.

The function may still receive `visible_apps` if other code needs the signature, but autosend should not use an arbitrary visible app.

**Step 4.2: Add/update tests.**

Add:

```rust
#[test]
fn prompt_pick_session_target_does_not_use_arbitrary_visible_app_without_recent_target()
```

Expected:

- frontmost = Prompt Picker
- visible apps include Codex/Claude/WeChat
- recent target = None
- result = None

Also keep/add:

```rust
#[test]
fn prompt_pick_session_target_uses_recent_target_when_picker_is_frontmost()
```

**Verification:**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib prompt_pick_session_target
```

**Commit:**

```bash
git add src-tauri/src/lib.rs
git commit -m "fix: avoid arbitrary visible app autosend target"
```

---

## Task 5: Update Or Remove Tests That Now Encode The Broken Path

**Files:**

- `src-tauri/src/lib.rs`
- `src-tauri/src/platform/macos.rs`

**Step 5.1: Keep tests for focus-preserving internals only if the internals remain.**

Any tests that assert user-facing autosend uses `guarded_focus_preserving_autosend_with_senders()` must be rewritten. The new expected user-facing behavior is:

```text
captured target exists -> activating sender runs
captured target missing/unsafe/untrusted -> copy-only or permission outcome
```

**Step 5.2: Replace anti-legacy assertions.**

If `src-tauri/src/platform/macos.rs` still has tests like:

```rust
legacy_activating_paste_script_is_not_present
```

replace them with tests that assert the activating sender exists, supports submit key, and does not interpolate prompt body into AppleScript.

**Step 5.3: Preserve true safety tests.**

Do not delete tests that guard against:

- Copying before accessibility permission.
- Sending with no target.
- Sending to unsafe targets.
- Broken clipboard write.
- Broken keyboard event.

**Verification:**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

**Commit:**

```bash
git add src-tauri/src/lib.rs src-tauri/src/platform/macos.rs
git commit -m "test: align autosend tests with activating target path"
```

---

## Task 6: Frontend Smoke Check For Submit Key Contract

**Files:**

- `src/App.tsx`
- `src/platform/platformApi.ts`
- Existing frontend tests if present

**Step 6.1: Confirm no UI behavior change is needed.**

The frontend should keep calling:

```ts
pastePromptAndSubmitToLastTarget(body, submitKey)
```

The backend now honors that submit key in the target-activating sender.

**Step 6.2: Add/adjust a small test only if current tests do not cover submit key forwarding.**

Expected:

- "填入并发送" passes `enter` or configured submit key.
- "只填入输入框" passes `none`.
- Command+Enter mode passes `command_enter`.

Do not redesign the settings UI in this task.

**Verification:**

```bash
npm test -- --run
```

**Commit:**

```bash
git add src/App.tsx src/platform/platformApi.ts src/**/*.test.ts src/**/*.test.tsx
git commit -m "test: cover autosend submit key forwarding"
```

Only include files actually changed.

---

## Task 7: Full Verification Before Completion

**Automated checks:**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --lib
npm test -- --run
npm run build
```

Expected:

- Rust formatting completes without diffs, or diffs are intentionally included.
- Cargo lib tests pass.
- Frontend tests pass.
- Frontend build passes.

**Manual QA, after building a Mac app:**

1. Quit every running Prompt Picker process.
2. Open the latest local app bundle from `src-tauri/target/release/bundle/macos/Prompt Picker.app`, not `/Applications/Prompt Picker.app` unless it has been replaced.
3. In Codex:
   - Click into the input box.
   - Click the cat.
   - Choose a single prompt in "填入并发送" mode.
   - Expected: prompt is pasted into Codex and submitted.
4. In Codex paste-only mode:
   - Expected: prompt is pasted but not submitted.
5. In Claude and WeChat:
   - Repeat the same smoke test.
   - Expected: target app activates, recorded/fallback point is clicked when available, prompt is pasted, submit key is sent.
6. With no captured target:
   - Open Prompt Picker without first focusing an input.
   - Choose a prompt.
   - Expected: copy-only outcome, no arbitrary app receives text.
7. With accessibility permission disabled:
   - Expected: missing-permission outcome before clipboard replacement.

**Commit if verification caused formatting/test fixture changes:**

```bash
git status --short
git add <changed files>
git commit -m "chore: verify codex autosend restore"
```

---

## Task 8: Push

**Before push:**

```bash
git status --short
git log --oneline -5
```

Confirm only intended commits for this task are ahead of `origin/main`.

**Push:**

```bash
git push origin main
```

---

## Expected User-Facing Result

From the user's point of view:

```text
Codex input is focused
        ↓
User clicks the cat
        ↓
Prompt panel opens
        ↓
User chooses a prompt
        ↓
Prompt Picker activates Codex again
        ↓
Prompt is pasted
        ↓
Configured submit key is sent
        ↓
Codex receives the message reliably
```

If the app cannot identify a real previous target, it must not guess. It copies the prompt and reports copy-only. But when Codex was the captured target, the app should act like the older stable implementation and send to Codex reliably.
