# Global Prompt Picker Flow Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the blue floating `Prompts` button reliably open a prompt list and insert the selected prompt into the user's last focused desktop input field.

**Architecture:** Fix the overlay entrypoint first so the floating button can call Tauri commands. Store the last valid input target in Rust/Tauri shared application state, not in a React `useRef`, because `main`, `prompt-button`, and `prompt-popover` are separate WebViews with separate JavaScript runtimes. Keep the first implementation focused on "user clicked into an input field first"; do not attempt full desktop-wide input-field discovery yet.

**Tech Stack:** Tauri v2, React 19, Vite, Vitest, macOS Accessibility/System Events, AppleScript, pbcopy/Cmd+V.

---

## Constraints And Product Rules

- Do not change unrelated UI or prompt management behavior.
- Do not attempt Level 2/3 global Accessibility-tree scanning in this pass.
- The supported interaction is:
  1. User focuses an input field.
  2. Prompt Picker records that focused target.
  3. User clicks the floating `Prompts` button.
  4. Popover opens without destroying the target context.
  5. User selects a prompt.
  6. App activates the recorded target app and pastes the prompt.
- If no input target has been recorded, show a clear message instead of silently failing.
- The last input target must live in backend shared state. Do not store it only in `src/App.tsx`; the popover window cannot read the main window's React refs.
- Keep tests small and add one behavior per task.

---

### Task 1: Prove The Overlay Button Needs Global Tauri Access

**Files:**
- Modify: `src/overlay/overlayHtml.test.ts`
- Test data: `src-tauri/tauri.conf.json`

**Step 1: Write the failing test**

Add an assertion that documents the dependency between the vanilla `overlay.html` and Tauri global injection:

```ts
import { readFileSync } from "fs";

it("enables global Tauri for the vanilla overlay html", () => {
  const config = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
  expect(config.app?.withGlobalTauri).toBe(true);
});
```

Also keep the existing assertions that `overlay.html` contains:

```ts
expect(html).toContain("window.__TAURI__");
expect(html).toContain("show_prompt_popover_from_button");
```

**Step 2: Run test to verify it fails**

Run:

```bash
./node_modules/.bin/vitest run src/overlay/overlayHtml.test.ts --reporter=verbose
```

Expected: FAIL because `app.withGlobalTauri` is currently missing.

**Step 3: Do not fix in this task**

Stop after confirming the failure. This task only captures the current root cause as a test.

**Step 4: Commit**

```bash
git add src/overlay/overlayHtml.test.ts
git commit -m "test: capture overlay global tauri requirement"
```

---

### Task 2: Enable Overlay Tauri API And Surface Overlay Errors

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `public/overlay.html`
- Modify: `src/overlay/overlayHtml.test.ts`

**Step 1: Enable global Tauri for vanilla overlay**

Update `src-tauri/tauri.conf.json`:

```json
{
  "app": {
    "withGlobalTauri": true,
    "windows": [
      {
        "title": "Prompt Picker",
        "width": 760,
        "height": 560,
        "minWidth": 640,
        "minHeight": 460,
        "resizable": true,
        "fullscreen": false
      }
    ],
    "security": {
      "csp": null
    }
  }
}
```

Preserve all existing sibling keys.

**Step 2: Add explicit overlay invoke error logging**

In `public/overlay.html`, add a helper:

```js
async function invoke(command, args) {
  if (!tauri?.core?.invoke) {
    console.error("Tauri invoke API is unavailable in overlay.html");
    return null;
  }
  try {
    return await tauri.core.invoke(command, args);
  } catch (error) {
    console.error(`Tauri command failed: ${command}`, error);
    return null;
  }
}
```

Then replace direct calls:

```js
await tauri.core.invoke('show_prompt_popover_from_button');
```

with:

```js
await invoke('show_prompt_popover_from_button');
```

Apply the same wrapper to:

- `show_prompt_button_controls_from_button`
- `move_prompt_button_to`
- `prompt_button_position_cmd`

**Step 3: Update static test**

In `src/overlay/overlayHtml.test.ts`, assert:

```ts
expect(html).toContain("Tauri invoke API is unavailable");
expect(html).toContain("Tauri command failed");
```

**Step 4: Run tests**

Run:

```bash
./node_modules/.bin/vitest run src/overlay/overlayHtml.test.ts src/app/tauriCapabilities.test.ts --reporter=verbose
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src-tauri/tauri.conf.json public/overlay.html src/overlay/overlayHtml.test.ts
git commit -m "fix: allow floating overlay to call tauri commands"
```

---

### Task 3: Add Backend Shared State For The Last Input Target

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/platform/macos.rs`

**Step 1: Write failing Rust tests**

Add cloneable/serializable state types if they do not already exist. Do not require `Deserialize` here; the state is written by backend code, not received as JSON from the frontend.

```rust
#[derive(Clone, Debug, serde::Serialize)]
pub struct LastInputTarget {
    pub app: FrontmostApp,
    pub observed_at_ms: u128,
}
```

Add tests in `src-tauri/src/lib.rs` or a small `last_input_target` module:

```rust
#[test]
fn stores_and_reads_last_input_target() {
    let state = LastInputTargetState::default();
    let target = LastInputTarget {
        app: FrontmostApp {
            name: "Notes".to_string(),
            bundle_id: "com.apple.Notes".to_string(),
        },
        observed_at_ms: 123,
    };

    state.set(target.clone());

    assert_eq!(state.get().unwrap().app.bundle_id, "com.apple.Notes");
}
```

**Step 2: Run tests to verify failure**

Run:

```bash
cd src-tauri
cargo test stores_and_reads_last_input_target
```

Expected: FAIL because `LastInputTargetState` does not exist yet.

**Step 3: Implement backend shared state**

Create a state wrapper:

```rust
#[derive(Default)]
pub struct LastInputTargetState(std::sync::Mutex<Option<LastInputTarget>>);

impl LastInputTargetState {
    pub fn set(&self, target: LastInputTarget) {
        *self.0.lock().expect("last input target lock poisoned") = Some(target);
    }

    pub fn get(&self) -> Option<LastInputTarget> {
        self.0
            .lock()
            .expect("last input target lock poisoned")
            .clone()
    }
}
```

Register it with Tauri:

```rust
.manage(LastInputTargetState::default())
```

Keep this state minimal: app name, bundle id, and observed timestamp are enough for this pass.

**Step 4: Run tests**

Run:

```bash
cd src-tauri
cargo test stores_and_reads_last_input_target
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/platform/macos.rs
git commit -m "feat: store last input target in tauri state"
```

---

### Task 4: Update Backend Target State During Input Polling

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/platform/macos.rs`
- Modify: `src/overlay/useInputTargetPolling.test.ts`

**Step 1: Write failing tests**

Add Rust tests for the command behavior:

```rust
#[test]
fn skips_prompt_picker_as_last_input_target() {
    let state = LastInputTargetState::default();
    let target = InputTarget {
        frame: CandidateInput { x: 0.0, y: 0.0, width: 10.0, height: 10.0 },
        window_frame: CandidateInput { x: 0.0, y: 0.0, width: 100.0, height: 100.0 },
        button_position: (10.0, 10.0),
        app: Some(FrontmostApp {
            name: "Prompt Picker".to_string(),
            bundle_id: "local.promptpicker.dev".to_string(),
        }),
    };

    record_last_input_target_if_valid(&state, &target);

    assert!(state.get().is_none());
}
```

Add a test that records a normal app:

```rust
#[test]
fn records_non_prompt_picker_input_target() {
    let state = LastInputTargetState::default();
    let target = InputTarget {
        frame: CandidateInput { x: 0.0, y: 0.0, width: 10.0, height: 10.0 },
        window_frame: CandidateInput { x: 0.0, y: 0.0, width: 100.0, height: 100.0 },
        button_position: (10.0, 10.0),
        app: Some(FrontmostApp {
            name: "Notes".to_string(),
            bundle_id: "com.apple.Notes".to_string(),
        }),
    };

    record_last_input_target_if_valid(&state, &target);

    assert_eq!(state.get().unwrap().app.bundle_id, "com.apple.Notes");
}
```

Add a frontend polling test only to verify `current_input_target` is still called and `showPromptButton` still receives the same position. Do not assert React owns the shared target.

**Step 2: Run test to verify failure**

Run:

```bash
cd src-tauri
cargo test last_input_target
```

Expected: FAIL because `record_last_input_target_if_valid` does not exist yet.

**Step 3: Update `current_input_target` command to record valid targets**

Change the Tauri command in `src-tauri/src/lib.rs` from:

```rust
fn current_input_target() -> Option<platform::InputTarget> {
    platform::macos::current_input_target()
}
```

to accept state:

```rust
fn current_input_target(
    state: tauri::State<LastInputTargetState>,
) -> Option<platform::InputTarget> {
    let target = platform::macos::current_input_target()?;
    record_last_input_target_if_valid(state.inner(), &target);
    Some(target)
}
```

Implement helper:

```rust
fn record_last_input_target_if_valid(
    state: &LastInputTargetState,
    target: &platform::InputTarget,
) {
    let Some(app) = target.app.clone() else {
        return;
    };
    if app.bundle_id == "local.promptpicker.dev" || app.name == "Prompt Picker" {
        return;
    }
    state.set(LastInputTarget {
        app,
        observed_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    });
}
```

**Step 4: Keep frontend polling simple**

Do not add `lastInputTargetRef` to `src/App.tsx`. The popover cannot safely read state from the main window's React runtime.

**Step 5: Run tests**

Run:

```bash
cd src-tauri
cargo test last_input_target
```

Expected: PASS.

**Step 6: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/platform/macos.rs src/overlay/useInputTargetPolling.test.ts
git commit -m "feat: record focused input target from backend"
```

---

### Task 5: Paste Prompt To Backend Last Target

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/platform/macos.rs`
- Modify: `src/platform/platformApi.ts`
- Modify: `src/App.tsx`
- Modify: `src/app/App.test.tsx`

**Step 1: Write failing Rust tests**

Add testable helpers in `src-tauri/src/platform/macos.rs`:

```rust
fn paste_to_app_script(bundle_id: &str) -> String {
    format!(
        r#"tell application id "{}" to activate
delay 0.1
tell application "System Events" to keystroke "v" using command down"#,
        bundle_id
    )
}
```

Test:

```rust
#[test]
fn paste_to_app_script_activates_target_bundle_before_paste() {
    let script = paste_to_app_script("com.apple.Notes");
    assert!(script.contains("tell application id \"com.apple.Notes\" to activate"));
    assert!(script.contains("keystroke \"v\" using command down"));
}
```

Add a backend command test for missing target if practical:

```rust
#[test]
fn missing_last_target_returns_clear_error() {
    let state = LastInputTargetState::default();
    let result = paste_prompt_to_last_target_impl("hello", &state);
    assert_eq!(result.unwrap_err(), "Click into a text field first, then choose a prompt.");
}
```

**Step 2: Run tests to verify failure**

```bash
cd src-tauri
cargo test paste_to_app_script_activates_target_bundle_before_paste missing_last_target_returns_clear_error
```

Expected: FAIL because helpers/command do not exist yet.

**Step 3: Add backend command**

In `src-tauri/src/lib.rs`, add:

```rust
#[tauri::command]
fn paste_prompt_to_last_target(
    body: String,
    state: tauri::State<LastInputTargetState>,
) -> Result<(), String> {
    paste_prompt_to_last_target_impl(&body, state.inner())
}
```

Implement:

```rust
fn paste_prompt_to_last_target_impl(
    body: &str,
    state: &LastInputTargetState,
) -> Result<(), String> {
    let Some(target) = state.get() else {
        return Err("Click into a text field first, then choose a prompt.".to_string());
    };
    platform::macos::paste_prompt_to_app(body, &target.app.bundle_id)
}
```

Register the command in `tauri::generate_handler!`.

**Step 4: Add frontend wrapper**

In `src/platform/platformApi.ts`:

```ts
export async function pastePromptToLastTarget(body: string): Promise<void> {
  return invoke("paste_prompt_to_last_target", { body });
}
```

Keep existing `pastePromptToApp(body, bundle_id)` unchanged. Do not rename `bundle_id` to `bundleId`; existing wrapper uses snake_case and should remain stable.

**Step 5: Update prompt selection**

In `src/App.tsx`, import `pastePromptToLastTarget` and update `handleSelect`:

```ts
await pastePromptToLastTarget(prompt.body);
await hidePromptPopover();
```

Remove direct fallback to `pastePrompt(prompt.body)` from the popover selection path. If the backend returns the no-target error, the existing catch block can show it:

```ts
const message = e instanceof Error ? e.message : String(e);
alert(message || "Failed to paste prompt. Please try again.");
```

**Step 6: Write/update frontend tests**

In `src/app/App.test.tsx`, assert prompt selection calls:

```ts
expect(vi.mocked(invoke)).toHaveBeenCalledWith("paste_prompt_to_last_target", {
  body: "Test body",
});
```

Add missing-target test by making invoke reject only for `paste_prompt_to_last_target`:

```ts
vi.mocked(invoke).mockImplementation(async (command) => {
  if (command === "paste_prompt_to_last_target") {
    throw new Error("Click into a text field first, then choose a prompt.");
  }
  return undefined;
});

window.alert = vi.fn();

fireEvent.click(await screen.findByRole("button", { name: /Test Prompt/i }));

await waitFor(() => {
  expect(window.alert).toHaveBeenCalledWith(
    "Click into a text field first, then choose a prompt."
  );
});
```

**Step 7: Run tests**

Run:

```bash
npm test
cd src-tauri && cargo test
```

Expected: PASS.

**Step 8: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/platform/macos.rs src/platform/platformApi.ts src/App.tsx src/app/App.test.tsx
git commit -m "feat: paste prompt to last focused target"
```

---

### Task 6: Preserve Existing Fallback As Explicit Copy-Only Recovery

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/platform/platformApi.ts`
- Modify: `src/app/App.test.tsx`

**Step 1: Write failing tests**

Add a test that no-target failure does not silently paste into the wrong app:

```ts
it("does not fall back to blind paste when no input target is recorded", async () => {
  const { invoke } = await import("@tauri-apps/api/core");
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "paste_prompt_to_last_target") {
      throw new Error("Click into a text field first, then choose a prompt.");
    }
    return undefined;
  });

  window.alert = vi.fn();

  fireEvent.click(await screen.findByRole("button", { name: /Test Prompt/i }));

  expect(vi.mocked(invoke)).not.toHaveBeenCalledWith("paste_prompt", expect.anything());
});
```

**Step 2: Decide explicit fallback UX**

Use this rule:

- Normal prompt click: paste only to backend last target.
- If no target: alert the user and leave the prompt list open.
- Existing manual fallback behavior can be exposed later as a separate "Copy" button. Do not add the Copy button in this plan.

**Step 3: Run tests**

Run:

```bash
npm test
```

Expected: PASS.

**Step 4: Commit**

```bash
git add src/App.tsx src/platform/platformApi.ts src/app/App.test.tsx
git commit -m "fix: avoid blind paste without target"
```

---

### Task 6.5: Optional Capability Tightening Review

**Files:**
- Review: `src-tauri/capabilities/default.json`
- Optional modify: `src-tauri/capabilities/default.json`
- Optional test: `src/app/tauriCapabilities.test.ts`

**Step 1: Review capability blast radius**

`withGlobalTauri: true` exposes `window.__TAURI__` to local webviews. Because this app loads local bundled resources only, this is acceptable for the fast fix, but the capability file currently grants the same permissions to `main`, `prompt-button`, and `prompt-popover`.

Review whether the minimal split should be:

- `main`: fs read/write, dialog, clipboard, shell open, core.
- `prompt-button`: core/event/window only.
- `prompt-popover`: fs app read, clipboard/paste command, core/event/window.

**Step 2: Do not split unless tests are added**

If splitting capabilities now, add a static test that verifies:

```ts
expect(promptButton.permissions).not.toContain("dialog:allow-open");
expect(promptButton.permissions).not.toContain("fs:allow-app-write-recursive");
```

**Step 3: Recommendation**

For this implementation pass, it is acceptable to leave the current broad capability unchanged after documenting the risk. Splitting can be a follow-up hardening task because it can create packaging/runtime regressions if rushed.

**Step 4: Commit only if changed**

```bash
git add src-tauri/capabilities/default.json src/app/tauriCapabilities.test.ts
git commit -m "chore: tighten overlay capabilities"
```

---

### Task 7: Manual Acceptance Test On Real macOS Apps

**Files:**
- Modify: `docs/qa/codex-app-acceptance.md`

**Step 1: Document acceptance checklist**

Append a section:

```md
## Prompt Picker Global Insert Acceptance

- [ ] Launch Prompt Picker.
- [ ] Confirm main window is not blank.
- [ ] Confirm blue `P Prompts` button is visible.
- [ ] Open Apple Notes or TextEdit.
- [ ] Click into a text field and confirm the caret is visible.
- [ ] Click the blue `P Prompts` button.
- [ ] Confirm a popover appears next to the button.
- [ ] Confirm the list contains `讨论方案`.
- [ ] Click `讨论方案`.
- [ ] Confirm the text is inserted into the original input field.
- [ ] Confirm the popover closes.
- [ ] Drag the blue button and confirm it stays where dropped.
- [ ] Right-click or Control-click the blue button and confirm controls open.
- [ ] Remove Accessibility permission and confirm the app shows a clear permission warning.
```

**Step 2: Commit**

```bash
git add docs/qa/codex-app-acceptance.md
git commit -m "docs: add global prompt insertion acceptance checks"
```

---

### Task 8: Full Build, Signing, And Bundle Verification

**Files:**
- Build output: `src-tauri/target/release/bundle/macos/Prompt Picker.app`
- Build output: `src-tauri/target/release/bundle/dmg/Prompt Picker_1.0.0_aarch64.dmg`

**Step 1: Run full test suite**

Run:

```bash
npm test
```

Expected:

```text
Test Files  13 passed
Tests       62+ passed
```

Exact test count may increase after new tests.

**Step 2: Build full Tauri app**

Run:

```bash
npm run tauri build
```

Expected:

```text
Finished 2 bundles at:
  src-tauri/target/release/bundle/macos/Prompt Picker.app
  src-tauri/target/release/bundle/dmg/Prompt Picker_1.0.0_aarch64.dmg
```

**Step 3: Re-sign local app if needed**

Run:

```bash
codesign --verify --deep --strict --verbose=2 \
  "src-tauri/target/release/bundle/macos/Prompt Picker.app"
```

If it reports `code has no resources but signature indicates they must be present`, run:

```bash
codesign --force --deep --sign - \
  "src-tauri/target/release/bundle/macos/Prompt Picker.app"
```

Then verify again.

**Step 4: Regenerate DMG after signing**

Run:

```bash
hdiutil create \
  -volname "Prompt Picker" \
  -srcfolder "src-tauri/target/release/bundle/macos/Prompt Picker.app" \
  -ov \
  -format UDZO \
  "src-tauri/target/release/bundle/dmg/Prompt Picker_1.0.0_aarch64.dmg"
```

**Step 5: Verify DMG**

Run:

```bash
hdiutil verify "src-tauri/target/release/bundle/dmg/Prompt Picker_1.0.0_aarch64.dmg"
```

Expected:

```text
hdiutil: verify: checksum of "...Prompt Picker_1.0.0_aarch64.dmg" is VALID
```

**Step 6: Launch the app**

Run:

```bash
pkill -f "Prompt Picker.app/Contents/MacOS/prompt-picker" || true
open -n "src-tauri/target/release/bundle/macos/Prompt Picker.app"
```

Expected:
- App process exists.
- Main window renders.
- Blue button renders.

**Step 7: Commit**

Usually do not commit build artifacts unless this repository intentionally tracks them. If build artifacts are tracked here, commit them separately:

```bash
git add dist src-tauri/target/release/bundle/macos src-tauri/target/release/bundle/dmg
git commit -m "build: package prompt picker global flow"
```

If build artifacts should not be tracked, do not commit them.

---

## Non-Goals For This Plan

- Do not scan all desktop windows for every possible text field.
- Do not implement OCR or visual detection of input boxes.
- Do not support password fields or protected secure inputs.
- Do not replace clipboard insertion with low-level synthetic text events in this pass.
- Do not redesign the prompt management UI.

---

## Risk Notes

- macOS Accessibility permission is required for reliable cross-app paste.
- Some apps may ignore `Cmd+V`, block automation, or use non-standard input surfaces.
- `withGlobalTauri: true` is the fastest fix for vanilla `overlay.html`; a later cleanup can replace the vanilla overlay with a Vite-built entrypoint.
- The last input target is intentionally stored in Rust/Tauri state because each webview has its own JavaScript runtime. Do not move this state back into React refs.
- The plan intentionally avoids blind fallback paste when no input target is recorded; this prevents inserting text into the wrong application.
- The floating popover must remain non-activating where possible, but paste should still explicitly reactivate the recorded target app.

---

## Final Manual Verification Script

Use this exact human flow before calling the work complete:

```text
1. Launch Prompt Picker.
2. Open Apple Notes or TextEdit.
3. Click into a text field and type "before ".
4. Click the blue Prompts button.
5. Confirm the popover opens.
6. Confirm "讨论方案" appears.
7. Click "讨论方案".
8. Confirm the prompt text appears after "before " in the original text field.
9. Confirm the popover closes.
10. Drag the blue button to a new position.
11. Wait 3 seconds.
12. Confirm the button does not snap back.
```
