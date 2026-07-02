# Autosend Feedback Status Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** After a prompt is selected, give the user a clear non-blocking result signal: either "sent" or "copied for manual paste", while keeping the OpenWhip-style foreground autosend flow.

**Architecture:** Keep the current foreground `Cmd+V + Enter` autosend path, but change the backend command to return a structured outcome so the frontend knows whether the prompt was copied and whether keyboard automation succeeded. The prompt popover remains non-blocking; the floating Calico button displays a short status bubble via a Tauri event, and the main window shows Accessibility readiness as a persistent status.

**Tech Stack:** Tauri 2, React, TypeScript, Rust, macOS `osascript`/`System Events`, Vitest, Cargo tests.

---

### Task 1: Return a Structured Autosend Outcome

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/platform/platformApi.ts`

**Step 1: Write the failing Rust tests**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`, add tests inside `#[cfg(test)] mod tests`:

```rust
#[test]
fn autosend_outcome_reports_copy_failure() {
    let outcome = AutosendOutcome::copy_failed("pbcopy failed".to_string());

    assert!(!outcome.copied);
    assert!(!outcome.sent);
    assert_eq!(outcome.error.as_deref(), Some("pbcopy failed"));
}

#[test]
fn autosend_outcome_reports_keyboard_failure_after_copy() {
    let outcome = AutosendOutcome::keyboard_failed("System Events denied".to_string());

    assert!(outcome.copied);
    assert!(!outcome.sent);
    assert_eq!(outcome.error.as_deref(), Some("System Events denied"));
}

#[test]
fn autosend_outcome_reports_sent_after_keyboard_success() {
    let outcome = AutosendOutcome::sent();

    assert!(outcome.copied);
    assert!(outcome.sent);
    assert!(outcome.error.is_none());
}
```

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`, update or add a test under `mod last_input_target_tests`:

```rust
#[test]
fn autosend_returns_foreground_outcome_without_last_target() {
    let state = LastInputTargetState::default();
    let result = paste_prompt_and_submit_to_last_target_with_sender("hello", &state, |body| {
        assert_eq!(body, "hello");
        Ok(platform::macos::AutosendOutcome::sent())
    });

    let outcome = result.unwrap();
    assert!(outcome.copied);
    assert!(outcome.sent);
}
```

**Step 2: Run tests to verify failure**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib autosend_outcome autosend_returns_foreground_outcome -- --nocapture
```

Expected: FAIL because `AutosendOutcome` does not exist and the command still returns `Result<(), String>`.

**Step 3: Implement `AutosendOutcome`**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`, add near `AccessibilityStatus`:

```rust
#[derive(Clone, Debug, Serialize)]
pub struct AutosendOutcome {
    pub copied: bool,
    pub sent: bool,
    pub error: Option<String>,
}

impl AutosendOutcome {
    fn sent() -> Self {
        Self {
            copied: true,
            sent: true,
            error: None,
        }
    }

    fn copy_failed(error: String) -> Self {
        Self {
            copied: false,
            sent: false,
            error: Some(error),
        }
    }

    fn keyboard_failed(error: String) -> Self {
        Self {
            copied: true,
            sent: false,
            error: Some(error),
        }
    }
}
```

Change `paste_prompt_and_submit_to_foreground` to:

```rust
pub fn paste_prompt_and_submit_to_foreground(body: &str) -> Result<AutosendOutcome, String> {
    if let Err(error) = copy_to_clipboard(body) {
        return Ok(AutosendOutcome::copy_failed(error));
    }
    refocus_previous_app_if_prompt_picker_frontmost();

    let script = foreground_paste_and_submit_script();
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Ok(AutosendOutcome::keyboard_failed(format_autosend_error(
            "foreground-paste-and-submit",
            String::from_utf8_lossy(&output.stderr).as_ref(),
        )));
    }
    Ok(AutosendOutcome::sent())
}
```

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`, import/export the type from `platform`, then change:

```rust
fn paste_prompt_and_submit_to_last_target(
    body: String,
    state: tauri::State<LastInputTargetState>,
) -> Result<AutosendOutcome, String>
```

and:

```rust
fn paste_prompt_and_submit_to_last_target_with_sender<F>(
    body: &str,
    _state: &LastInputTargetState,
    sender: F,
) -> Result<AutosendOutcome, String>
where
    F: FnOnce(&str) -> Result<AutosendOutcome, String>,
{
    sender(body)
}
```

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/platform/platformApi.ts`, add:

```ts
export interface AutosendOutcome {
  copied: boolean;
  sent: boolean;
  error: string | null;
}
```

and change:

```ts
export async function pastePromptAndSubmitToLastTarget(
  body: string
): Promise<AutosendOutcome> {
  return invoke<AutosendOutcome>("paste_prompt_and_submit_to_last_target", { body });
}
```

**Step 4: Run tests to verify pass**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib autosend_outcome autosend_returns_foreground_outcome -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src-tauri/src/platform/macos.rs src-tauri/src/lib.rs src/platform/platformApi.ts
git commit -m "feat: return autosend outcome"
```

---

### Task 2: Emit Non-Blocking Autosend Status from the Prompt Popover

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`

**Step 1: Write failing frontend tests**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`, add an event mock near existing Tauri mocks:

```tsx
const emitMock = vi.hoisted(() => vi.fn().mockResolvedValue(undefined));

vi.mock("@tauri-apps/api/event", () => ({
  emit: emitMock,
}));
```

In `beforeEach`, add:

```tsx
emitMock.mockClear();
```

Add tests in `describe("app", () => { ... })`:

```tsx
it("emits a sent status when autosend reports keyboard success", async () => {
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
    expect(emitMock).toHaveBeenCalledWith("prompt-autosend-status", {
      kind: "sent",
      message: "已发送",
    });
  });
});

it("emits a copied status when autosend copies but keyboard automation fails", async () => {
  const { invoke } = await import("@tauri-apps/api/core");
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    if (command === "paste_prompt_and_submit_to_last_target") {
      return { copied: true, sent: false, error: "System Events denied" };
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
      kind: "copied",
      message: "已复制，可手动 Cmd+V",
    });
  });
});
```

**Step 2: Run tests to verify failure**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "emits a sent status|emits a copied status"
```

Expected: FAIL because `App.tsx` does not emit the event.

**Step 3: Implement status emission**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`, import:

```tsx
import { emit } from "@tauri-apps/api/event";
```

Add helper near `waitForWindowHide`:

```tsx
type AutosendStatusKind = "sent" | "copied" | "failed";

async function emitAutosendStatus(kind: AutosendStatusKind, message: string) {
  try {
    await emit("prompt-autosend-status", { kind, message });
  } catch (error) {
    console.warn("Failed to emit autosend status:", error);
  }
}
```

Change `handleSelect`:

```tsx
const outcome = await pastePromptAndSubmitToLastTarget(prompt.body);
if (outcome.sent) {
  await emitAutosendStatus("sent", "已发送");
} else if (outcome.copied) {
  await emitAutosendStatus("copied", "已复制，可手动 Cmd+V");
} else {
  await emitAutosendStatus("failed", "未能复制，请重试");
}
```

Change `catch` to:

```tsx
console.warn("Prompt autosend failed without blocking the picker:", e);
await emitAutosendStatus("failed", "未能发送，请重试");
```

**Step 4: Run tests to verify pass**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "emits a sent status|emits a copied status|blocking dialog"
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src/App.tsx src/app/App.test.tsx
git commit -m "feat: emit autosend status"
```

---

### Task 3: Show a Short Status Bubble on Calico

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/public/overlay.html`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/overlayHtml.test.ts`

**Step 1: Write failing overlay tests**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/overlayHtml.test.ts`, add:

```ts
it("listens for prompt autosend status and renders a Calico status bubble", () => {
  const html = readFileSync("public/overlay.html", "utf8");

  expect(html).toContain("prompt-autosend-status");
  expect(html).toContain("calico-status-bubble");
  expect(html).toContain("showStatusBubble");
  expect(html).toContain("hideStatusBubble");
  expect(html).toContain("statusBubble.textContent");
});
```

**Step 2: Run test to verify failure**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "Calico status bubble"
```

Expected: FAIL because the overlay has no status bubble.

**Step 3: Add the bubble markup and CSS**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/public/overlay.html`, add inside `<body>` before the button:

```html
<div id="statusBubble" class="calico-status-bubble" aria-live="polite"></div>
```

Add CSS before `</style>`:

```css
.calico-status-bubble {
  position: absolute;
  top: 4px;
  left: 50%;
  z-index: 2;
  max-width: 124px;
  padding: 5px 8px;
  color: #1f2937;
  background: rgba(255, 255, 255, 0.94);
  border: 1px solid rgba(203, 213, 225, 0.9);
  border-radius: 999px;
  box-shadow: 0 8px 18px rgba(15, 23, 42, 0.14);
  font: 700 11px/1.2 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  opacity: 0;
  pointer-events: none;
  text-align: center;
  transform: translateX(-50%) translateY(-4px);
  transition:
    opacity 120ms ease,
    transform 120ms ease;
  white-space: nowrap;
}

.calico-status-bubble.is-visible {
  opacity: 1;
  transform: translateX(-50%) translateY(0);
}

.calico-status-bubble.is-sent {
  color: #166534;
  border-color: rgba(134, 239, 172, 0.9);
}

.calico-status-bubble.is-copied,
.calico-status-bubble.is-failed {
  color: #92400e;
  border-color: rgba(253, 186, 116, 0.9);
}
```

**Step 4: Add the event listener**

In the `<script type="module">` section, after `const sprite = ...`, add:

```js
const statusBubble = document.getElementById('statusBubble');
let statusTimer = 0;
```

Add helpers:

```js
function hideStatusBubble() {
  if (!statusBubble) return;
  statusBubble.classList.remove('is-visible');
}

function showStatusBubble(payload) {
  if (!statusBubble || !payload) return;
  const kind = payload.kind || 'copied';
  const message = payload.message || '已复制';
  window.clearTimeout(statusTimer);
  statusBubble.textContent = message;
  statusBubble.className = `calico-status-bubble is-visible is-${kind}`;
  statusTimer = window.setTimeout(hideStatusBubble, 1800);
}

function listenForAutosendStatus() {
  if (!tauri?.event?.listen) return;
  tauri.event.listen('prompt-autosend-status', (event) => {
    showStatusBubble(event.payload);
  }).catch((error) => {
    console.error('Tauri event listen failed: prompt-autosend-status', error);
  });
}
```

Call `listenForAutosendStatus();` before event listener setup ends.

**Step 5: Run test to verify pass**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "Calico status bubble|animated Calico"
```

Expected: PASS.

**Step 6: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add public/overlay.html src/overlay/overlayHtml.test.ts
git commit -m "feat: show autosend status on calico"
```

---

### Task 4: Add Main Window Accessibility Readiness Status

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`

**Step 1: Write failing frontend tests**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`, add:

```tsx
it("shows autosend accessibility readiness in the main window", async () => {
  currentWindowLabel = "main";
  const { invoke } = await import("@tauri-apps/api/core");
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    if (command === "accessibility_status_cmd") return { trusted: false };
    return undefined;
  });
  const { readTextFile } = await import("@tauri-apps/plugin-fs");
  (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValue(
    JSON.stringify({ version: 1, prompts: mockPrompts })
  );

  await act(async () => {
    render(<App />);
  });

  expect(await screen.findByText("Autosend: Needs Accessibility")).toBeTruthy();
});

it("opens Accessibility settings from the main window status control", async () => {
  currentWindowLabel = "main";
  const { invoke } = await import("@tauri-apps/api/core");
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    if (command === "accessibility_status_cmd") return { trusted: false };
    return undefined;
  });
  const { readTextFile } = await import("@tauri-apps/plugin-fs");
  (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValue(
    JSON.stringify({ version: 1, prompts: mockPrompts })
  );

  await act(async () => {
    render(<App />);
  });

  fireEvent.click(await screen.findByRole("button", { name: "Open Accessibility Settings" }));

  expect(vi.mocked(invoke)).toHaveBeenCalledWith("open_accessibility_settings");
});
```

**Step 2: Write failing Rust test**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`, add:

```rust
#[test]
fn accessibility_settings_url_targets_privacy_accessibility() {
    assert!(accessibility_settings_url().contains("Privacy_Accessibility"));
}
```

**Step 3: Run tests to verify failure**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "accessibility readiness|Accessibility settings"
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib accessibility_settings_url -- --nocapture
```

Expected: FAIL because UI status and backend command do not exist.

**Step 4: Implement backend command**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`, add:

```rust
pub fn accessibility_settings_url() -> &'static str {
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
}

pub fn open_accessibility_settings() -> Result<(), String> {
    let output = Command::new("open")
        .arg(accessibility_settings_url())
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
```

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`, add:

```rust
#[tauri::command]
fn open_accessibility_settings() -> Result<(), String> {
    platform::macos::open_accessibility_settings()
}
```

and include it in `tauri::generate_handler![ ... ]`.

**Step 5: Implement main window status**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`, import `getAccessibilityStatus` from `platformApi`.

Add state:

```tsx
const [accessibilityTrusted, setAccessibilityTrusted] = useState<boolean | null>(null);
```

In `useEffect`, after setting settings:

```tsx
getAccessibilityStatus()
  .then((status) => setAccessibilityTrusted(status.trusted))
  .catch(() => setAccessibilityTrusted(null));
```

Pass `accessibilityTrusted` and `onOpenAccessibilitySettings` into `MainWindow`:

```tsx
accessibilityTrusted={accessibilityTrusted}
onOpenAccessibilitySettings={() => invoke("open_accessibility_settings")}
```

In `MainWindow`, render beside the existing floating status:

```tsx
<div className="status-row">
  <span className={floatingButtonVisible ? "status-pill is-on" : "status-pill"}>
    Status: {floatingButtonVisible ? "Visible" : "Hidden"}
  </span>
  {accessibilityTrusted === false ? (
    <button className="status-pill status-action" onClick={onOpenAccessibilitySettings}>
      Autosend: Needs Accessibility
      <span className="sr-only">Open Accessibility Settings</span>
    </button>
  ) : (
    <span className="status-pill is-on">
      Autosend: {accessibilityTrusted ? "Ready" : "Checking"}
    </span>
  )}
</div>
```

Add CSS:

```css
.status-row {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
}

.status-action {
  cursor: pointer;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
}
```

**Step 6: Run tests to verify pass**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "accessibility readiness|Accessibility settings|floating button status"
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib accessibility_settings_url -- --nocapture
```

Expected: PASS.

**Step 7: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src/App.tsx src/app/App.test.tsx src/styles.css src-tauri/src/lib.rs src-tauri/src/platform/macos.rs
git commit -m "feat: show autosend permission status"
```

---

### Task 5: Final Verification and Packaging

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

Expected: PASS, no formatting diff, all Rust tests pass.

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
plutil -p "/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app/Contents/Info.plist" | rg "LSUIElement|CFBundleIdentifier"
strings "/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app/Contents/MacOS/prompt-picker" | rg "foreground-paste-and-submit|Privacy_Accessibility"
```

Expected:
- `LSUIElement` is `true`.
- `CFBundleIdentifier` is `local.promptpicker.dev`.
- Binary contains `foreground-paste-and-submit`.
- Binary contains `Privacy_Accessibility`.

**Step 6: Restart local app**

```bash
pids=$(pgrep -x prompt-picker || true)
if [ -n "$pids" ]; then kill $pids; sleep 1; fi
open -n "/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app"
sleep 2
pgrep -x -a prompt-picker || true
```

Expected: one running `prompt-picker` process.

**Step 7: Commit final fixes if any**

If verification required source fixes:

```bash
git add <changed source files>
git commit -m "fix: stabilize autosend feedback"
```

If no source fixes are needed, skip this step.

---

### Task 6: Push to GitHub Main

**Files:**
- Git only.

**Step 1: Check worktree**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git status --short
```

Expected: source files are committed. Generated `dist`, `node_modules`, `src-tauri/target`, `.app`, and `.dmg` changes may remain uncommitted and should not be staged.

**Step 2: Push**

```bash
git push origin main
```

Expected: `main -> main`.

**Step 3: Final user-facing summary**

Report:
- Commit hash.
- Verification commands and pass counts.
- App and DMG paths.
- Expected user experience:

```text
Click prompt
→ list closes
→ if autosend succeeded: Calico briefly shows "已发送"
→ if keyboard automation failed after copy: Calico briefly shows "已复制，可手动 Cmd+V"
→ main window shows whether Autosend is Ready or Needs Accessibility
```
