# Codex Prompt Autosend Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Selecting a prompt from the Calico floating picker pastes it into the Codex input box and presses Return to send it.

**Architecture:** Keep the existing Tauri/React three-window model: `main` records input targets, `prompt-button` is the non-activating Calico entry, and `prompt-popover` is the quick prompt list. Reuse the existing last-input-target state, add a macOS paste-and-submit backend path, and make the UI call that path when a prompt is selected. Guard the first implementation to Codex (`com.openai.codex`) so the automatic Return key cannot accidentally send text into chat apps or documents.

**Tech Stack:** Tauri 2, Rust, macOS AppleScript via `osascript`, `pbcopy`, React, TypeScript, Vitest, Cargo tests.

---

## Constraints

- Do not modify generated build output under `/Users/yang/Desktop/GitHub-pre/prompt-picker/dist`.
- Do not modify generated build output under `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target`.
- Do not commit `/Users/yang/Desktop/GitHub-pre/prompt-picker/node_modules/.package-lock.json`.
- Do not use OpenWhip's Ctrl+C interrupt behavior. This tool should paste and submit, not interrupt.
- Keep the existing generic paste command available for future use; add a new autosend command instead of changing `paste_prompt_to_last_target`.

## Target User Flow

```text
User clicks into the Codex input box
  -> User clicks Calico
  -> Prompt Picker records the current input target
  -> Prompt list opens above Calico
  -> User clicks a prompt
  -> Prompt list closes
  -> Prompt Picker activates Codex
  -> Prompt Picker pastes via Cmd+V
  -> Prompt Picker presses Return
  -> Codex sends the prompt
```

## Task 1: Add macOS Paste-And-Submit Backend

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`

**Step 1: Write the failing tests**

Add these tests inside the existing `#[cfg(test)] mod tests` in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`:

```rust
#[test]
fn paste_and_submit_to_app_script_activates_target_pastes_and_presses_return() {
    let script = paste_and_submit_to_app_script("com.openai.codex");

    assert!(script.contains("tell application id \"com.openai.codex\" to activate"));
    assert!(script.contains("keystroke \"v\" using command down"));
    assert!(script.contains("key code 36"));
}

#[test]
fn paste_and_submit_to_app_script_uses_clipboard_not_literal_prompt_text() {
    let script = paste_and_submit_to_app_script("com.openai.codex");

    assert!(!script.contains("keystroke \"{body}\""));
    assert!(!script.contains("Test body"));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test paste_and_submit_to_app_script --lib
```

Expected: FAIL because `paste_and_submit_to_app_script` does not exist.

**Step 3: Write minimal implementation**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`, add this next to `paste_prompt_to_app`:

```rust
pub fn paste_prompt_and_submit_to_app(body: &str, bundle_id: &str) -> Result<(), String> {
    copy_to_clipboard(body)?;
    let script = paste_and_submit_to_app_script(bundle_id);
    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(())
}

fn paste_and_submit_to_app_script(bundle_id: &str) -> String {
    format!(
        r#"tell application id "{}" to activate
delay 0.15
tell application "System Events"
    keystroke "v" using command down
    delay 0.1
    key code 36
end tell"#,
        bundle_id
    )
}
```

**Step 4: Run test to verify it passes**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test paste_and_submit_to_app_script --lib
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src-tauri/src/platform/macos.rs
git commit -m "feat: add macos paste and submit macro"
```

## Task 2: Add Codex-Guarded Tauri Command

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`

**Step 1: Write the failing tests**

Inside `#[cfg(test)] mod last_input_target_tests` in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`, add:

```rust
#[test]
fn missing_codex_last_target_returns_clear_autosend_error() {
    let state = LastInputTargetState::default();
    let result = codex_last_target_bundle_id(&state);

    assert_eq!(
        result.unwrap_err(),
        "Click into the Codex input box first, then choose a prompt."
    );
}

#[test]
fn rejects_non_codex_target_for_autosend() {
    let state = LastInputTargetState::default();
    state.set(LastInputTarget {
        app: FrontmostApp {
            name: "WeChat".to_string(),
            bundle_id: "com.tencent.xinWeChat".to_string(),
        },
        observed_at_ms: 123,
    });

    let result = codex_last_target_bundle_id(&state);

    assert_eq!(
        result.unwrap_err(),
        "Autosend is only enabled for Codex. Click into the Codex input box first."
    );
}

#[test]
fn accepts_codex_target_for_autosend() {
    let state = LastInputTargetState::default();
    state.set(LastInputTarget {
        app: FrontmostApp {
            name: "Codex".to_string(),
            bundle_id: "com.openai.codex".to_string(),
        },
        observed_at_ms: 123,
    });

    assert_eq!(
        codex_last_target_bundle_id(&state).unwrap(),
        "com.openai.codex"
    );
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test codex_last_target --lib
```

Expected: FAIL because `codex_last_target_bundle_id` does not exist.

**Step 3: Write minimal implementation**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`, add this near the existing paste command:

```rust
const CODEX_BUNDLE_ID: &str = "com.openai.codex";

#[tauri::command]
fn paste_prompt_and_submit_to_last_target(
    body: String,
    state: tauri::State<LastInputTargetState>,
) -> Result<(), String> {
    paste_prompt_and_submit_to_last_target_impl(&body, state.inner())
}

fn paste_prompt_and_submit_to_last_target_impl(
    body: &str,
    state: &LastInputTargetState,
) -> Result<(), String> {
    let bundle_id = codex_last_target_bundle_id(state)?;
    platform::macos::paste_prompt_and_submit_to_app(body, &bundle_id)
}

fn codex_last_target_bundle_id(state: &LastInputTargetState) -> Result<String, String> {
    let Some(target) = state.get() else {
        return Err("Click into the Codex input box first, then choose a prompt.".to_string());
    };
    if target.app.bundle_id != CODEX_BUNDLE_ID {
        return Err(
            "Autosend is only enabled for Codex. Click into the Codex input box first.".to_string(),
        );
    }
    Ok(target.app.bundle_id)
}
```

Then register the command in the existing `tauri::generate_handler!` list:

```rust
paste_prompt_and_submit_to_last_target,
```

**Step 4: Run test to verify it passes**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test codex_last_target --lib
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src-tauri/src/lib.rs
git commit -m "feat: add codex autosend command"
```

## Task 3: Make Prompt Selection Call Autosend

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/platform/platformApi.ts`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`

**Step 1: Write the failing frontend tests**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`, change the test name:

```ts
it("autosends selected prompt into the backend last Codex target", async () => {
```

Change its expectation from:

```ts
expect(vi.mocked(invoke)).toHaveBeenCalledWith(
  "paste_prompt_to_last_target",
  { body: "Test body" }
);
```

to:

```ts
expect(vi.mocked(invoke)).toHaveBeenCalledWith(
  "paste_prompt_and_submit_to_last_target",
  { body: "Test body" }
);
```

Also update `"does not move the floating button when selecting a prompt from the popover"` to expect the new command:

```ts
expect(vi.mocked(invoke)).toHaveBeenCalledWith(
  "paste_prompt_and_submit_to_last_target",
  { body: "Test body" }
);
```

Finally update `"does not fall back to blind paste when no input target is recorded"` so the mock throws on the new command:

```ts
if (command === "paste_prompt_and_submit_to_last_target") {
  throw new Error("Click into the Codex input box first, then choose a prompt.");
}
```

and expect the Codex-specific alert:

```ts
expect(window.alert).toHaveBeenCalledWith(
  "Click into the Codex input box first, then choose a prompt."
);
```

**Step 2: Run test to verify it fails**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/app/App.test.tsx
```

Expected: FAIL because `App.tsx` still calls `paste_prompt_to_last_target`.

**Step 3: Add the frontend API wrapper**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/platform/platformApi.ts`, add:

```ts
export async function pastePromptAndSubmitToLastTarget(body: string): Promise<void> {
  return invoke("paste_prompt_and_submit_to_last_target", { body });
}
```

**Step 4: Change prompt selection**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`, replace the import:

```ts
pastePromptToLastTarget,
```

with:

```ts
pastePromptAndSubmitToLastTarget,
```

Then change `handleSelect`:

```ts
await pastePromptAndSubmitToLastTarget(prompt.body);
await hidePromptPopover();
```

**Step 5: Run test to verify it passes**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/app/App.test.tsx
```

Expected: PASS.

**Step 6: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src/platform/platformApi.ts src/App.tsx src/app/App.test.tsx
git commit -m "feat: autosend selected prompts to codex"
```

## Task 4: Record Current Input Target Before Opening The Popover

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/public/overlay.html`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/overlayHtml.test.ts`

**Step 1: Write the failing test**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/overlayHtml.test.ts`, add:

```ts
it("records the current input target before opening the prompt list", () => {
  const html = readFileSync("public/overlay.html", "utf8");

  expect(html).toContain("current_input_target");
  expect(html.indexOf("current_input_target")).toBeLessThan(
    html.indexOf("show_prompt_popover_from_button")
  );
});
```

**Step 2: Run test to verify it fails**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/overlay/overlayHtml.test.ts
```

Expected: FAIL because `overlay.html` opens the prompt popover without first invoking `current_input_target`.

**Step 3: Update the click path**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/public/overlay.html`, change the non-drag pointerup branch from:

```js
setSprite('thinking', 900);
await invoke('show_prompt_popover_from_button');
```

to:

```js
setSprite('thinking', 900);
await invoke('current_input_target');
await invoke('show_prompt_popover_from_button');
```

Reason: the Calico panel is configured as non-activating, so immediately before opening the popover the frontmost app should still be Codex when the user clicked from Codex. This improves freshness of `LastInputTargetState`.

**Step 4: Run test to verify it passes**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/overlay/overlayHtml.test.ts
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add public/overlay.html src/overlay/overlayHtml.test.ts
git commit -m "fix: record input target before opening prompt list"
```

## Task 5: Add Double-Click Protection In The Prompt List

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/ui/PromptQuickList.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/ui/PromptQuickList.test.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`

**Step 1: Write the failing component test**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/ui/PromptQuickList.test.tsx`, add a test that verifies a submitting prompt is disabled:

```ts
it("disables the prompt currently being submitted", () => {
  render(
    <PromptQuickList
      prompts={mockPrompts}
      onSelect={vi.fn()}
      submittingPromptId="1"
    />
  );

  expect(screen.getByRole("button", { name: /Test Prompt/i })).toBeDisabled();
});
```

**Step 2: Run test to verify it fails**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/ui/PromptQuickList.test.tsx
```

Expected: FAIL because `PromptQuickList` does not accept `submittingPromptId`.

**Step 3: Update PromptQuickList props**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/ui/PromptQuickList.tsx`, change the props:

```ts
interface PromptQuickListProps {
  prompts: PromptItem[];
  onSelect: (prompt: PromptItem) => void;
  submittingPromptId?: string | null;
}
```

Change the component signature:

```ts
export function PromptQuickList({
  prompts,
  onSelect,
  submittingPromptId = null,
}: PromptQuickListProps) {
```

Add `disabled` to each prompt button:

```tsx
disabled={submittingPromptId === prompt.id}
```

**Step 4: Add App state**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`, add:

```ts
const [submittingPromptId, setSubmittingPromptId] = useState<string | null>(null);
```

At the start of `handleSelect`:

```ts
if (submittingPromptId) return;
setSubmittingPromptId(prompt.id);
```

At the end of `handleSelect`, add a `finally` block:

```ts
} finally {
  setSubmittingPromptId(null);
}
```

Pass it into the quick list:

```tsx
<PromptQuickList
  prompts={prompts}
  onSelect={handleSelect}
  submittingPromptId={submittingPromptId}
/>
```

**Step 5: Run tests to verify they pass**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/ui/PromptQuickList.test.tsx src/app/App.test.tsx
```

Expected: PASS.

**Step 6: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src/ui/PromptQuickList.tsx src/App.tsx src/ui/PromptQuickList.test.tsx src/app/App.test.tsx
git commit -m "fix: prevent duplicate prompt autosend"
```

## Task 6: Full Verification

**Files:**
- No source files expected.

**Step 1: Format Rust**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo fmt
```

Expected: no errors.

**Step 2: Run Rust tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib
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

**Step 5: Build Tauri bundle**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
CARGO_BUILD_JOBS=1 npm run tauri -- build
```

Expected: PASS.

**Step 6: Runtime smoke check**

Manual check:

```text
1. Launch /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app
2. Click into the Codex input panel.
3. Click Calico.
4. Click one prompt.
5. Verify the prompt appears in Codex and is sent automatically.
6. Click into a non-Codex input field.
7. Click Calico and choose a prompt.
8. Verify the app refuses to autosend and shows the Codex-specific error.
```

Expected: Codex path sends; non-Codex path does not send.

**Step 7: Final commit if formatting changed files**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git status --short
git add <only source/test files changed by this task>
git commit -m "test: verify codex prompt autosend"
```

Only commit if Task 6 produced legitimate source/test/doc changes. Do not commit generated `dist`, `target`, or `node_modules` files.

## Acceptance Criteria

- Clicking Calico opens the prompt list above the character as before.
- Clicking a prompt calls the new autosend command, not the old paste-only command.
- The prompt body is copied via `pbcopy`, pasted via Cmd+V, and submitted via Return (`key code 36`).
- The autosend path only works when the last recorded target is Codex (`com.openai.codex`).
- Non-Codex targets produce a clear error and do not paste or press Return.
- Existing generic paste commands remain available.
- Existing drag behavior and popover placement remain unchanged.
- Rust tests, frontend tests, frontend build, and Tauri build pass.
