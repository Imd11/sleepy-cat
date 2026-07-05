# Async Autosend Responsiveness Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make single-prompt and group autosend run without making Prompt Picker feel frozen or showing a persistent macOS spinning cursor over Calico.

**Architecture:** Keep the existing macOS automation behavior intact, but move blocking native work out of the synchronous Tauri IPC command path with `tauri::async_runtime::spawn_blocking`. Add a lightweight autosend activity signal so the hidden main window pauses input-target polling while a send is in flight, reducing contention with `osascript` and `lsappinfo`.

**Tech Stack:** React 19, TypeScript, Vitest, Tauri 2, Rust 2021, macOS Accessibility/System Events automation.

---

## Context And Constraints

Current confirmed root causes:

- Single-prompt selection calls `paste_prompt_and_submit_to_last_target` from `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`.
- That command is currently a synchronous `#[tauri::command]` in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`.
- Paste-only mode calls `paste_prompt_to_last_target`, which is also currently a synchronous `#[tauri::command]` and must be covered by this plan.
- The macOS implementation in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs` performs blocking work: `osascript`, `lsappinfo`, `wait_for_frontmost_bundle_id`, and several `std::thread::sleep` calls.
- Clicking Calico starts `begin_prompt_pick_session`, which also performs blocking target detection.
- The hidden main window continues input-target polling through `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/useInputTargetPolling.ts`, and that polling also calls native commands that use `osascript` and `lsappinfo`.

Do not change in this plan:

- Do not redesign Calico visuals, prompt list UI, settings UI, tray icon, app logo, or prompt manager layout.
- Do not change prompt text storage, categories, group schema, import/export, or group interval semantics.
- Do not remove the existing macOS automation sleeps in the first pass. Moving them off the blocking IPC path is the first fix. Timing tuning can come later with measurements.
- Do not remove accessibility permission checks.
- Do not use `git reset --hard`, `git checkout --`, or stage unrelated generated build artifacts.

Expected user-visible result:

```text
Click Calico
  -> prompt list appears as before
  -> Calico remains responsive

Click one prompt
  -> prompt list closes immediately
  -> Calico does not show a persistent macOS spinning cursor
  -> target app is activated
  -> prompt is pasted and Return is sent
  -> Calico receives success or failure motion

During autosend
  -> hidden main-window input polling pauses
  -> native target detection does not compete with the send path
```

---

### Task 1: Add Tests For Shared Native State And Blocking-Command Guardrails

**Files:**

- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`

**Step 1: Add tests that document state clone sharing**

Add tests in the existing `#[cfg(test)] mod tests` block in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`.

These tests should be added before implementation so they fail until the state wrappers are made cloneable/shared:

```rust
#[test]
fn cloned_last_input_target_state_shares_target() {
    let state = LastInputTargetState::default();
    let cloned = state.clone();

    state.set(LastInputTarget {
        app: FrontmostApp {
            name: "Codex".to_string(),
            bundle_id: "com.openai.codex".to_string(),
        },
        observed_at_ms: now_ms(),
        click_point: Some(TargetClickPoint { x: 12.0, y: 34.0 }),
    });

    assert_eq!(
        cloned.get().unwrap().app.bundle_id,
        "com.openai.codex"
    );
}

#[test]
fn cloned_prompt_pick_session_state_shares_target() {
    let state = PromptPickSessionState::default();
    let cloned = state.clone();

    state.begin(7);
    assert!(cloned.set_if_current(
        7,
        PromptPickSessionTarget {
            app: FrontmostApp {
                name: "Codex".to_string(),
                bundle_id: "com.openai.codex".to_string(),
            },
            observed_at_ms: now_ms(),
            click_point: None,
        }
    ));

    assert_eq!(
        state.take().unwrap().app.bundle_id,
        "com.openai.codex"
    );
}
```

**Step 2: Add a source-level guardrail test for async command shape**

Add this test in the same Rust test module:

```rust
#[test]
fn autosend_and_prompt_capture_commands_use_spawn_blocking() {
    let source = include_str!("lib.rs");

    assert!(source.contains("async fn begin_prompt_pick_session"));
    assert!(source.contains("async fn paste_prompt_to_last_target"));
    assert!(source.contains("async fn paste_prompt_and_submit_to_last_target"));
    assert!(source.contains("async fn paste_prompt_sequence_and_submit_to_last_target"));
    assert!(source.matches("tauri::async_runtime::spawn_blocking(move ||").count() >= 4);
}
```

This test is intentionally white-box. The current bug is caused by command execution shape, so the plan needs a regression guard that blocks silently reverting these commands back to synchronous IPC handlers.

**Step 3: Run Rust tests and verify failure**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test cloned_last_input_target_state_shares_target
cargo test cloned_prompt_pick_session_state_shares_target
cargo test autosend_and_prompt_capture_commands_use_spawn_blocking
```

Expected: FAIL because `LastInputTargetState` and `PromptPickSessionState` are not cloneable shared wrappers yet, and the commands are not async/spawned yet.

**Step 4: Commit after this task only if executing**

```bash
git add /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs
git commit -m "test: capture autosend responsiveness guardrails"
```

---

### Task 2: Make Native State Cloneable For Background Work

**Files:**

- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`

**Step 1: Convert state wrappers to shared `Arc<Mutex<...>>` wrappers**

Change `LastInputTargetState` from:

```rust
#[derive(Default)]
pub struct LastInputTargetState(std::sync::Mutex<Option<LastInputTarget>>);
```

to:

```rust
#[derive(Clone, Default)]
pub struct LastInputTargetState(std::sync::Arc<std::sync::Mutex<Option<LastInputTarget>>>);
```

Change `PromptPickSessionState` from:

```rust
#[derive(Default)]
pub struct PromptPickSessionState(std::sync::Mutex<PromptPickSessionInner>);
```

to:

```rust
#[derive(Clone, Default)]
pub struct PromptPickSessionState(std::sync::Arc<std::sync::Mutex<PromptPickSessionInner>>);
```

Do not change the public methods except as required by formatting. The existing `set`, `get`, `take`, `begin`, and `set_if_current` methods should keep the same behavior.

**Step 2: Run focused Rust tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test cloned_last_input_target_state_shares_target
cargo test cloned_prompt_pick_session_state_shares_target
```

Expected: PASS.

**Step 3: Run existing state/autosend tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test prompt_pick_session
cargo test paste_prompt_and_submit
cargo test autosend
```

Expected: PASS. If the filter does not match all intended tests, run full `cargo test` before committing.

**Step 4: Commit**

```bash
git add /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs
git commit -m "refactor: share prompt target state across native tasks"
```

---

### Task 3: Move Prompt Insert And Autosend Commands To Background Blocking Workers

**Files:**

- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`
- Do not modify unless a compiler error forces it: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`

**Step 1: Convert paste-only last-target command to async + `spawn_blocking`**

Paste-only mode uses this command through `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/platform/platformApi.ts` and `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`. It must be moved with the send commands; otherwise the responsiveness fix only works in `paste_and_submit` mode.

Replace the existing command:

```rust
#[tauri::command]
fn paste_prompt_to_last_target(
    body: String,
    state: tauri::State<LastInputTargetState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    paste_prompt_to_last_target_impl(&body, state.inner(), |text| {
        copy_text_to_clipboard(&app, text)
    })
}
```

with:

```rust
#[tauri::command]
async fn paste_prompt_to_last_target(
    body: String,
    state: tauri::State<'_, LastInputTargetState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let state = state.inner().clone();
    let app = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        paste_prompt_to_last_target_impl(&body, &state, |text| {
            copy_text_to_clipboard(&app, text)
        })
    })
    .await
    .map_err(|error| format!("Paste task failed: {}", error))?
}
```

If Rust complains that a Tauri state parameter is held across the `await`, introduce a small private helper that clones the managed state before the first `.await`, then call the helper. Do not capture `tauri::State<'_, T>` directly inside the blocking closure.

**Step 2: Convert single autosend command to async + `spawn_blocking`**

Replace the existing command:

```rust
#[tauri::command]
fn paste_prompt_and_submit_to_last_target(
    body: String,
    session_state: tauri::State<PromptPickSessionState>,
    recent_state: tauri::State<LastInputTargetState>,
    app: tauri::AppHandle,
) -> Result<AutosendOutcome, String> {
    paste_prompt_and_submit_to_last_target_impl(
        &body,
        session_state.inner(),
        recent_state.inner(),
        &app,
    )
}
```

with:

```rust
#[tauri::command]
async fn paste_prompt_and_submit_to_last_target(
    body: String,
    session_state: tauri::State<'_, PromptPickSessionState>,
    recent_state: tauri::State<'_, LastInputTargetState>,
    app: tauri::AppHandle,
) -> Result<AutosendOutcome, String> {
    let session_state = session_state.inner().clone();
    let recent_state = recent_state.inner().clone();
    let app = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        paste_prompt_and_submit_to_last_target_impl(
            &body,
            &session_state,
            &recent_state,
            &app,
        )
    })
    .await
    .map_err(|error| format!("Autosend task failed: {}", error))?
}
```

If Rust complains that a Tauri state parameter is held across the `await`, introduce a small private helper that clones the managed state before the first `.await`, then call the helper. Do not capture `tauri::State<'_, T>` directly inside the blocking closure.

**Step 3: Convert group autosend command to async + `spawn_blocking`**

Replace the existing command:

```rust
#[tauri::command]
fn paste_prompt_sequence_and_submit_to_last_target(
    bodies: Vec<String>,
    interval_ms: u64,
    session_state: tauri::State<PromptPickSessionState>,
    recent_state: tauri::State<LastInputTargetState>,
    app: tauri::AppHandle,
) -> Result<AutosendSequenceOutcome, String> {
    paste_prompt_sequence_and_submit_to_last_target_impl(
        &bodies,
        interval_ms,
        session_state.inner(),
        recent_state.inner(),
        &app,
    )
}
```

with:

```rust
#[tauri::command]
async fn paste_prompt_sequence_and_submit_to_last_target(
    bodies: Vec<String>,
    interval_ms: u64,
    session_state: tauri::State<'_, PromptPickSessionState>,
    recent_state: tauri::State<'_, LastInputTargetState>,
    app: tauri::AppHandle,
) -> Result<AutosendSequenceOutcome, String> {
    let session_state = session_state.inner().clone();
    let recent_state = recent_state.inner().clone();
    let app = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        paste_prompt_sequence_and_submit_to_last_target_impl(
            &bodies,
            interval_ms,
            &session_state,
            &recent_state,
            &app,
        )
    })
    .await
    .map_err(|error| format!("Autosend sequence task failed: {}", error))?
}
```

**Step 4: Keep the sync implementation helpers unchanged**

Do not rewrite these helpers unless the compiler requires a type signature adjustment:

- `paste_prompt_to_last_target_impl`
- `paste_prompt_and_submit_to_last_target_impl`
- `paste_prompt_sequence_and_submit_to_last_target_impl`
- `paste_prompt_and_submit_to_session_target_with_senders`
- `paste_prompt_sequence_and_submit_to_session_target_with_senders`

The plan is to preserve autosend behavior while changing where the blocking work runs.

**Step 5: Run Rust command-shape tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test autosend_and_prompt_capture_commands_use_spawn_blocking
```

Expected: still FAIL until `begin_prompt_pick_session` is also converted in Task 4. It should now only fail on the missing async prompt-capture command, not on `paste_prompt_to_last_target`, `paste_prompt_and_submit_to_last_target`, or sequence autosend.

**Step 6: Run existing paste and autosend tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test paste_prompt_to_last_target
cargo test paste_prompt_and_submit
cargo test autosend_sequence
cargo test autosend
```

Expected: PASS.

**Step 7: Commit**

```bash
git add /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs
git commit -m "fix: run prompt insertion automation off the ipc handler"
```

---

### Task 4: Move Prompt Pick Session Capture To A Background Blocking Worker

**Files:**

- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`
- Test reference: `/Users/yang/Desktop/GitHub-pre/prompt-picker/public/overlay.html`
- Test reference: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/overlayHtml.test.ts`

**Step 1: Convert `begin_prompt_pick_session` to async + `spawn_blocking`**

Replace:

```rust
#[tauri::command]
fn begin_prompt_pick_session(
    session_id: u64,
    session_state: tauri::State<PromptPickSessionState>,
    recent_state: tauri::State<LastInputTargetState>,
) -> Option<FrontmostApp> {
    if let Some(input_target) = platform::macos::current_input_target() {
        record_last_input_target_if_valid(recent_state.inner(), &input_target);
    }

    let Some(target) = prompt_pick_session_target(
        frontmost_app(),
        platform::macos::visible_apps(),
        recent_state.inner().get(),
    ) else {
        session_state.inner().clear_if_current(session_id);
        return None;
    };
    record_prompt_pick_session_target_if_valid(session_state.inner(), target, session_id)
}
```

with:

```rust
#[tauri::command]
async fn begin_prompt_pick_session(
    session_id: u64,
    session_state: tauri::State<'_, PromptPickSessionState>,
    recent_state: tauri::State<'_, LastInputTargetState>,
) -> Option<FrontmostApp> {
    let session_state = session_state.inner().clone();
    let recent_state = recent_state.inner().clone();

    tauri::async_runtime::spawn_blocking(move || {
        if let Some(input_target) = platform::macos::current_input_target() {
            record_last_input_target_if_valid(&recent_state, &input_target);
        }

        let Some(target) = prompt_pick_session_target(
            frontmost_app(),
            platform::macos::visible_apps(),
            recent_state.get(),
        ) else {
            session_state.clear_if_current(session_id);
            return None;
        };
        record_prompt_pick_session_target_if_valid(&session_state, target, session_id)
    })
    .await
    .ok()
    .flatten()
}
```

**Step 2: Keep the front-end fire-and-forget behavior**

Confirm `/Users/yang/Desktop/GitHub-pre/prompt-picker/public/overlay.html` still contains:

```js
const sessionPromise = invoke('begin_prompt_pick_session', { sessionId });
void sessionPromise.catch(() => null);
```

Do not change it to `await`. The list must open immediately and target capture must remain non-blocking from the overlay user interaction perspective.

**Step 3: Run Rust guardrail test**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test autosend_and_prompt_capture_commands_use_spawn_blocking
```

Expected: PASS.

**Step 4: Run overlay HTML tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npx vitest run src/overlay/overlayHtml.test.ts
```

Expected: PASS, including the assertion that `begin_prompt_pick_session` is not awaited.

**Step 5: Commit**

```bash
git add /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs
git commit -m "fix: capture prompt target without blocking calico"
```

---

### Task 5: Emit Autosend Activity And Make Prompt Selection UI Lightweight

**Files:**

- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`
- Test: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`

**Step 1: Add failing tests for autosend activity events**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`, add or update tests around single prompt selection to verify:

- `prompt-autosend-activity` is emitted with `{ active: true }` before the autosend invoke.
- `prompt-autosend-activity` is emitted with `{ active: false }` after the autosend invoke settles.
- The false event is emitted in both success and failure paths.
- The same activity events are emitted in paste-only mode before and after `paste_prompt_to_last_target`.

Representative test structure:

```ts
it("emits autosend activity around single prompt sending", async () => {
  const calls: string[] = [];
  invokeMock.mockImplementation(async (command: string) => {
    calls.push(`invoke:${command}`);
    if (command === "hide_prompt_popover") return undefined;
    if (command === "paste_prompt_and_submit_to_last_target") {
      return { copied: true, sent: true, error: null, reason: null };
    }
    return defaultInvokeResponse(command);
  });
  emitMock.mockImplementation(async (event: string, payload?: unknown) => {
    calls.push(`emit:${event}:${JSON.stringify(payload)}`);
  });

  renderPromptPopoverWithSinglePrompt();
  await userEvent.click(screen.getByRole("option", { name: /prompt title/i }));

  await waitFor(() => {
    expect(calls).toContain("invoke:paste_prompt_and_submit_to_last_target");
  });

  expect(calls).toContain('emit:prompt-autosend-activity:{"active":true}');
  expect(calls).toContain('emit:prompt-autosend-activity:{"active":false}');
  expect(calls.indexOf('emit:prompt-autosend-activity:{"active":true}')).toBeLessThan(
    calls.indexOf("invoke:paste_prompt_and_submit_to_last_target")
  );
});
```

Use the existing test helpers in `App.test.tsx`; do not introduce a new testing framework.

**Step 2: Add a helper in `App.tsx`**

Add near `emitAutosendStatus`:

```ts
async function emitAutosendActivity(active: boolean) {
  try {
    await emit("prompt-autosend-activity", { active });
  } catch (error) {
    console.warn("Failed to emit autosend activity:", error);
  }
}
```

**Step 3: Add an in-flight ref separate from visual submitting state**

In `App`, add:

```ts
const autosendInFlightRef = useRef(false);
```

Change the start of `handleSelect` from:

```ts
if (submittingPromptId || promptListRefreshingRef.current) return;
setSubmittingPromptId(prompt.id);
```

to:

```ts
if (autosendInFlightRef.current || promptListRefreshingRef.current) return;
autosendInFlightRef.current = true;
setSubmittingPromptId(prompt.id);
```

**Step 4: Clear list-level submitting state after the popover is hidden**

Inside `handleSelect`, after:

```ts
await hidePromptPopover();
await waitForWindowHide();
```

add:

```ts
setSubmittingPromptId(null);
await emitAutosendActivity(true);
```

This keeps the prompt row protected during the click/close transition, then removes list-level waiting UI once the list is hidden. Calico motion remains the user-facing sending feedback.

**Step 5: Always stop autosend activity in `finally`**

Change the `finally` block from:

```ts
finally {
  setSubmittingPromptId(null);
}
```

to:

```ts
finally {
  autosendInFlightRef.current = false;
  setSubmittingPromptId(null);
  await emitAutosendActivity(false);
}
```

If `emitAutosendActivity(true)` is not reached because `hidePromptPopover` throws, emitting false is still harmless and keeps listeners safe.

**Step 6: Run focused frontend tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npx vitest run src/app/App.test.tsx
```

Expected: PASS.

**Step 7: Commit**

```bash
git add /Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx /Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx
git commit -m "fix: signal autosend activity from prompt selection"
```

---

### Task 6: Pause Input Target Polling During Autosend

**Files:**

- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/useInputTargetPolling.ts`
- Test: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/useInputTargetPolling.test.ts`

**Step 1: Add failing polling pause tests**

Add tests in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/useInputTargetPolling.test.ts` that verify:

- Receiving `prompt-autosend-activity` with `{ active: true }` pauses future polling.
- Receiving `{ active: false }` resumes polling after a short delay.
- If a poll result returns while paused, it does not call `showPromptButton` again.

Representative test shape:

```ts
it("pauses input target polling while autosend is active", async () => {
  const listeners = new Map<string, (event: { payload: unknown }) => void>();
  listenMock.mockImplementation(async (event, callback) => {
    listeners.set(event, callback as (event: { payload: unknown }) => void);
    return vi.fn();
  });

  renderHook(() => useInputTargetPolling([], { buttonOffset: null, buttonPosition: null }));
  await waitFor(() => expect(getFrontmostApp).toHaveBeenCalled());
  vi.clearAllMocks();

  listeners.get("prompt-autosend-activity")?.({ payload: { active: true } });
  vi.advanceTimersByTime(2500);

  expect(getFrontmostApp).not.toHaveBeenCalled();
  expect(getCurrentInputTarget).not.toHaveBeenCalled();
});
```

Use existing mocks and timer patterns already present in this test file.

**Step 2: Listen for autosend activity in the hook**

Add a ref near the existing refs:

```ts
const autosendPausedRef = useRef(false);
```

Inside the first `useEffect` that already registers drag listeners, register another listener:

```ts
let unlistenAutosendActivity: (() => void) | undefined;

listen<{ active?: boolean }>("prompt-autosend-activity", (event) => {
  autosendPausedRef.current = event.payload?.active === true;
  if (!autosendPausedRef.current) {
    lastTargetAtRef.current = Date.now();
  }
})
  .then((unlisten) => {
    if (active) {
      unlistenAutosendActivity = unlisten;
    } else {
      unlisten();
    }
  })
  .catch(() => {});
```

Dispose it in the cleanup:

```ts
unlistenAutosendActivity?.();
```

**Step 3: Gate the polling loop**

At the top of `poll`, after the `floatingButtonVisible` check and before target detection, add:

```ts
if (autosendPausedRef.current) {
  schedulePoll(500);
  return;
}
```

After every awaited native call inside `poll`, check pause state before continuing expensive follow-up work:

```ts
const app = await getFrontmostApp();
if (!isCurrent() || autosendPausedRef.current) return;

const inputTarget = (await getCurrentInputTarget()) as InputTarget | null;
if (!isCurrent() || autosendPausedRef.current) return;
```

This prevents a poll that was already in flight from showing or moving Calico after autosend starts.

**Step 4: Resume with a short delay**

When the listener receives `{ active: false }`, do not call `poll()` directly from inside the event listener. Let the next scheduled poll occur. If the tests show a long resume delay, schedule a 700ms resume by adding a small `resumeAfterAutosendRef` timestamp and honoring it in `poll`. Keep the first implementation simple unless tests prove it is too slow.

**Step 5: Run focused polling tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npx vitest run src/overlay/useInputTargetPolling.test.ts
```

Expected: PASS.

**Step 6: Commit**

```bash
git add /Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/useInputTargetPolling.ts /Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/useInputTargetPolling.test.ts
git commit -m "fix: pause input polling during autosend"
```

---

### Task 7: Verify End-To-End Behavior And Build

**Files:**

- No planned source edits.
- Verification may produce generated files under `dist/`, `src-tauri/target/`, or release folders. Do not stage generated artifacts unless explicitly requested later.

**Step 1: Run full frontend tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test
```

Expected: PASS.

**Step 2: Run TypeScript and Vite build**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm run build
```

Expected: PASS.

**Step 3: Run full Rust tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test
```

Expected: PASS.

**Step 4: Run Rust build**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo build
```

Expected: PASS.

**Step 5: Manual macOS acceptance check**

Use the local app build or dev app and verify:

```text
1. Click into Codex or another safe text input.
2. Click Calico.
3. Click a single prompt container.
4. Observe:
   - prompt list closes immediately
   - Calico does not show a persistent spinning cursor
   - target app receives pasted prompt and Return
   - success motion appears after autosend
5. Repeat with mouse resting over Calico during autosend.
6. Switch Settings -> prompt click behavior to paste-only, then repeat with a single prompt.
7. Repeat with a group prompt to confirm sequencing still works.
```

Expected: single prompt feels responsive from the Prompt Picker side, even though the target app activation and paste still take normal macOS automation time.

**Step 6: Inspect changed files before final commit**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git status --short
git diff -- src/App.tsx src/app/App.test.tsx src/overlay/useInputTargetPolling.ts src/overlay/useInputTargetPolling.test.ts src-tauri/src/lib.rs
```

Expected:

- Only planned source/test files are modified.
- No unrelated `dist/`, `node_modules/`, `src-tauri/target/`, release, or DMG artifacts are staged.

**Step 7: Final commit**

If prior task commits were skipped, commit the complete implementation:

```bash
git add \
  /Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx \
  /Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx \
  /Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/useInputTargetPolling.ts \
  /Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/useInputTargetPolling.test.ts \
  /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs
git commit -m "fix: keep prompt autosend responsive"
```

Do not push unless the user explicitly asks during execution.

---

## Risk Review Before Execution

Main risks:

1. `AppHandle` or plugin clipboard access may behave differently inside `spawn_blocking`.
   - Mitigation: keep full Rust tests and run manual macOS autosend checks.
   - If clipboard writing fails in the blocking worker, resolve target and compute plan in the worker but perform clipboard write through a small async command boundary only if evidence proves it is required.

2. Async Tauri commands with `tauri::State<'_, T>` may fail to compile if state borrows are held across `.await`.
   - Mitigation: clone Arc-backed state wrappers before the first `.await`, and do not capture `tauri::State` in the blocking closure.

3. Pausing polling could delay Calico repositioning immediately after autosend.
   - Mitigation: pause only while autosend is active and let normal polling resume after completion. Do not disable polling globally.

4. Clearing `submittingPromptId` after the popover hides could allow repeated selection if the window remains visible due to a hide failure.
   - Mitigation: guard with `autosendInFlightRef`, not only state. The ref remains true until autosend finishes.

5. Group prompts still intentionally take time to send.
   - Mitigation: this plan does not change group sequencing. It only prevents the app UI from feeling frozen while the sequence runs.

6. Paste-only mode uses a separate command from paste-and-submit mode.
   - Mitigation: include `paste_prompt_to_last_target` in the same `spawn_blocking` treatment and activity-event flow. Do not consider the responsiveness work complete until paste-only manual verification passes.

---

## Acceptance Criteria

- Single prompt selection no longer makes Calico or the app feel frozen during macOS automation.
- Paste-only prompt insertion no longer makes Calico or the app feel frozen during macOS automation.
- Mouse over Calico does not show a persistent spinning cursor caused by Prompt Picker blocking its own command path.
- Prompt text still lands in the target input and Return is still sent.
- In paste-only mode, prompt text still lands in the target input without Return.
- Group prompt sending still preserves order and interval behavior.
- Input target polling pauses during autosend and resumes afterward.
- Existing prompt management, settings, Calico idle motion, menu bar, and window behavior tests still pass.

---

## Final Verification Commands

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test
npm run build

cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test
cargo build
```
