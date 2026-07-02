# Actionable Autosend Permission UX Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make autosend failures understandable and actionable, especially when macOS Accessibility permission blocks automatic paste/send.

**Architecture:** Keep the existing autosend flow (`pbcopy` -> native `CGEvent` Cmd+V -> native `CGEvent` Return), but return structured failure reasons from Rust so React can show precise status copy. Keep the UI non-blocking: Calico shows a short status bubble, permission failures provide a direct Accessibility Settings action, and the menu bar/main window expose the same recovery path.

**Tech Stack:** Tauri 2, Rust 2021, macOS ApplicationServices Accessibility/CoreGraphics, React, TypeScript, Vitest, Cargo tests, macOS menu bar/tray.

---

### Task 1: Add Structured Autosend Failure Reasons

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/platform/platformApi.ts`

**Step 1: Write failing Rust tests for reason-specific outcomes**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`, add tests inside `#[cfg(test)] mod tests`:

```rust
#[test]
fn autosend_outcome_reports_missing_accessibility_permission() {
    let outcome = AutosendOutcome::missing_accessibility_permission();

    assert!(outcome.copied);
    assert!(!outcome.sent);
    assert_eq!(
        outcome.reason,
        Some(AutosendFailureReason::MissingAccessibilityPermission)
    );
}

#[test]
fn autosend_outcome_reports_return_key_failure_after_copy() {
    let outcome = AutosendOutcome::return_event_failed("return failed".to_string());

    assert!(outcome.copied);
    assert!(!outcome.sent);
    assert_eq!(
        outcome.reason,
        Some(AutosendFailureReason::ReturnEventFailed)
    );
}
```

Also update existing autosend outcome tests to assert:

```rust
assert_eq!(outcome.reason, Some(AutosendFailureReason::CopyFailed));
assert_eq!(outcome.reason, Some(AutosendFailureReason::PasteEventFailed));
assert!(outcome.reason.is_none());
```

**Step 2: Run focused Rust tests to verify failure**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib autosend_outcome -- --nocapture
```

Expected: FAIL because `AutosendFailureReason`, `reason`, `missing_accessibility_permission`, and `return_event_failed` do not exist yet.

**Step 3: Implement structured failure reasons**

Add near `AccessibilityStatus` in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/platform/macos.rs`:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutosendFailureReason {
    CopyFailed,
    MissingAccessibilityPermission,
    PasteEventFailed,
    ReturnEventFailed,
    TargetFocusFailed,
}
```

Update `AutosendOutcome`:

```rust
#[derive(Clone, Debug, Serialize)]
pub struct AutosendOutcome {
    pub copied: bool,
    pub sent: bool,
    pub error: Option<String>,
    pub reason: Option<AutosendFailureReason>,
}
```

Update constructors:

```rust
pub fn sent() -> Self {
    Self {
        copied: true,
        sent: true,
        error: None,
        reason: None,
    }
}

pub fn copy_failed(error: String) -> Self {
    Self {
        copied: false,
        sent: false,
        error: Some(error),
        reason: Some(AutosendFailureReason::CopyFailed),
    }
}

pub fn keyboard_failed(error: String) -> Self {
    Self::paste_event_failed(error)
}

pub fn missing_accessibility_permission() -> Self {
    Self {
        copied: true,
        sent: false,
        error: Some("Accessibility permission required for autosend.".to_string()),
        reason: Some(AutosendFailureReason::MissingAccessibilityPermission),
    }
}

pub fn paste_event_failed(error: String) -> Self {
    Self {
        copied: true,
        sent: false,
        error: Some(error),
        reason: Some(AutosendFailureReason::PasteEventFailed),
    }
}

pub fn return_event_failed(error: String) -> Self {
    Self {
        copied: true,
        sent: false,
        error: Some(error),
        reason: Some(AutosendFailureReason::ReturnEventFailed),
    }
}
```

**Step 4: Route actual autosend failures to the specific reasons**

In `paste_prompt_and_submit_to_foreground`:

```rust
if !is_accessibility_trusted() {
    return Ok(AutosendOutcome::missing_accessibility_permission());
}
```

For paste failure:

```rust
return Ok(AutosendOutcome::paste_event_failed(format_autosend_error(
    "Native paste event failed",
    &error,
)));
```

For return failure:

```rust
return Ok(AutosendOutcome::return_event_failed(format_autosend_error(
    "Native return event failed",
    &error,
)));
```

**Step 5: Update TypeScript API type**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/platform/platformApi.ts`, add:

```ts
export type AutosendFailureReason =
  | "copy_failed"
  | "missing_accessibility_permission"
  | "paste_event_failed"
  | "return_event_failed"
  | "target_focus_failed";

export interface AutosendOutcome {
  copied: boolean;
  sent: boolean;
  error: string | null;
  reason: AutosendFailureReason | null;
}
```

**Step 6: Run focused Rust tests**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib autosend_outcome -- --nocapture
```

Expected: PASS.

**Step 7: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src-tauri/src/platform/macos.rs src/platform/platformApi.ts
git commit -m "feat: add autosend failure reasons"
```

---

### Task 2: Map Failure Reasons to User-Facing Calico Status

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`

**Step 1: Write failing frontend tests**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`, replace the generic failure test with:

```tsx
it("emits an actionable permission status when autosend lacks accessibility permission", async () => {
  const { invoke } = await import("@tauri-apps/api/core");
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    if (command === "paste_prompt_and_submit_to_last_target") {
      return {
        copied: true,
        sent: false,
        error: "Accessibility permission required for autosend.",
        reason: "missing_accessibility_permission",
      };
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
      message: "开启辅助功能",
      action: "open_accessibility_settings",
    });
  });
});
```

Add a return-key failure test:

```tsx
it("emits a distinct status when autosend pastes but cannot press return", async () => {
  const { invoke } = await import("@tauri-apps/api/core");
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    if (command === "paste_prompt_and_submit_to_last_target") {
      return {
        copied: true,
        sent: false,
        error: "Native return event failed",
        reason: "return_event_failed",
      };
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
      message: "已粘贴，未发送",
    });
  });
});
```

**Step 2: Run tests to verify failure**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "permission status|cannot press return"
```

Expected: FAIL because the frontend still maps copied-but-unsent to the generic permission message.

**Step 3: Add a centralized status mapper**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`, import `AutosendOutcome`:

```tsx
import type { AutosendOutcome } from "./platform/platformApi";
```

Replace `AutosendStatusKind` and add an optional action:

```tsx
type AutosendStatusKind = "sent" | "failed";
type AutosendStatusAction = "open_accessibility_settings";
```

Update the emitter:

```tsx
async function emitAutosendStatus(
  kind: AutosendStatusKind,
  message: string,
  action?: AutosendStatusAction
) {
  try {
    const payload = action ? { kind, message, action } : { kind, message };
    await emit("prompt-autosend-status", payload);
  } catch (error) {
    console.warn("Failed to emit autosend status:", error);
  }
}
```

Add:

```tsx
function statusForAutosendOutcome(outcome: AutosendOutcome): {
  kind: AutosendStatusKind;
  message: string;
  action?: AutosendStatusAction;
} {
  if (outcome.sent) {
    return { kind: "sent", message: "已发送" };
  }

  switch (outcome.reason) {
    case "missing_accessibility_permission":
      return {
        kind: "failed",
        message: "开启辅助功能",
        action: "open_accessibility_settings",
      };
    case "copy_failed":
      return { kind: "failed", message: "未能复制" };
    case "paste_event_failed":
      return { kind: "failed", message: "未能粘贴" };
    case "return_event_failed":
      return { kind: "failed", message: "已粘贴，未发送" };
    case "target_focus_failed":
      return { kind: "failed", message: "未找到输入框" };
    default:
      return {
        kind: "failed",
        message: outcome.copied ? "未能自动发送" : "未能复制",
      };
  }
}
```

In `handleSelect`, replace the copied/sent branching with:

```tsx
const status = statusForAutosendOutcome(outcome);
await emitAutosendStatus(status.kind, status.message, status.action);
```

**Step 4: Run focused tests**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "permission status|cannot press return"
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src/App.tsx src/app/App.test.tsx
git commit -m "fix: show specific autosend status"
```

---

### Task 3: Make Calico Permission Status Clickable

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/public/overlay.html`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/overlayHtml.test.ts`

**Step 1: Write failing overlay test**

Add to `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/overlayHtml.test.ts`:

```ts
it("opens Accessibility settings from actionable autosend status bubbles", () => {
  const html = readFileSync("public/overlay.html", "utf8");

  expect(html).toContain("open_accessibility_settings");
  expect(html).toContain("statusBubble.dataset.action");
  expect(html).toContain("is-action");
  expect(html).toContain("Open Accessibility Settings");
  expect(html).not.toContain("payload.kind || 'copied'");
});
```

**Step 2: Run test to verify failure**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "autosend status bubbles"
```

Expected: FAIL because the bubble does not open Accessibility Settings yet.

**Step 3: Add action styling and click handler**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/public/overlay.html`, add CSS:

```css
.calico-status-bubble.is-action {
  cursor: pointer;
  pointer-events: auto;
}
```

Update `showStatusBubble`:

```js
function showStatusBubble(payload) {
  if (!statusBubble || !payload) return;
  const kind = payload.kind || 'failed';
  const message = payload.message || '未能自动发送';
  const action = payload.action || '';
  window.clearTimeout(statusTimer);
  statusBubble.textContent = message;
  statusBubble.dataset.action = action;
  statusBubble.title = action === 'open_accessibility_settings' ? 'Open Accessibility Settings' : '';
  statusBubble.className = `calico-status-bubble is-visible is-${kind}${action ? ' is-action' : ''}`;
  statusTimer = window.setTimeout(hideStatusBubble, action ? 3200 : 1800);
}
```

Add after `listenForAutosendStatus();`:

```js
statusBubble?.addEventListener('click', async () => {
  if (statusBubble.dataset.action === 'open_accessibility_settings') {
    await invoke('open_accessibility_settings');
  }
});
```

**Step 4: Run overlay tests**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "autosend status bubbles|manual paste"
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add public/overlay.html src/overlay/overlayHtml.test.ts
git commit -m "fix: open accessibility settings from calico"
```

---

### Task 4: Add Main Window Recheck and Menu Bar Accessibility Entry

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`

**Step 1: Write failing frontend test for recheck**

Add to `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`:

```tsx
it("rechecks Accessibility status from the main window", async () => {
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

  fireEvent.click(
    await screen.findByRole("button", { name: "Recheck Accessibility" })
  );

  await waitFor(() => {
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("accessibility_status_cmd");
    expect(
      vi.mocked(invoke).mock.calls.filter(
        ([command]) => command === "accessibility_status_cmd"
      ).length
    ).toBeGreaterThanOrEqual(2);
  });
});
```

**Step 2: Run test to verify failure**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "rechecks Accessibility"
```

Expected: FAIL because the main window does not expose a recheck button yet.

**Step 3: Refactor accessibility status refresh into a callback**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`, add inside `App`:

```tsx
const refreshAccessibilityStatus = useCallback(async () => {
  setAccessibilityTrusted(null);
  try {
    const status = await getAccessibilityStatus();
    setAccessibilityTrusted(status.trusted);
  } catch {
    setAccessibilityTrusted(null);
  }
}, []);
```

Use it in `useEffect`:

```tsx
if (label === "main") {
  refreshAccessibilityStatus();
}
```

Pass it to `MainWindow`:

```tsx
onRefreshAccessibilityStatus={refreshAccessibilityStatus}
```

Update `MainWindow` props:

```tsx
onRefreshAccessibilityStatus: () => void;
```

Render beside the existing Accessibility button:

```tsx
<button
  className="status-pill status-action"
  aria-label="Recheck Accessibility"
  onClick={onRefreshAccessibilityStatus}
>
  Recheck
</button>
```

**Step 4: Add menu bar Accessibility entry test**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`, update `maps_tray_menu_item_ids_to_actions` to assert:

```rust
assert_eq!(
    tray_menu_action(TRAY_OPEN_ACCESSIBILITY_ID),
    TrayMenuAction::OpenAccessibilitySettings
);
```

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib menu_bar_app_tests::maps_tray_menu_item_ids_to_actions -- --nocapture
```

Expected: FAIL because the menu id/action does not exist yet.

**Step 5: Implement menu bar Accessibility entry**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`, add:

```rust
const TRAY_OPEN_ACCESSIBILITY_ID: &str = "open-accessibility-settings";
```

Add enum variant:

```rust
OpenAccessibilitySettings,
```

Map it in `tray_menu_action`:

```rust
TRAY_OPEN_ACCESSIBILITY_ID => TrayMenuAction::OpenAccessibilitySettings,
```

Create menu item in `setup_menu_bar_app`:

```rust
let open_accessibility = MenuItem::with_id(
    app_handle,
    TRAY_OPEN_ACCESSIBILITY_ID,
    "Open Accessibility Settings",
    true,
    None::<&str>,
)
.map_err(|e| e.to_string())?;
```

Insert it before the separator:

```rust
&[
    &open_main,
    &show_button,
    &hide_button,
    &open_accessibility,
    &separator,
    &quit,
]
```

Handle it in `.on_menu_event`:

```rust
TrayMenuAction::OpenAccessibilitySettings => {
    let _ = platform::macos::open_accessibility_settings();
}
```

**Step 6: Run focused tests**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- -t "rechecks Accessibility|Accessibility settings"

cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test --lib menu_bar_app_tests::maps_tray_menu_item_ids_to_actions -- --nocapture
```

Expected: PASS.

**Step 7: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src/App.tsx src/app/App.test.tsx src-tauri/src/lib.rs
git commit -m "fix: expose accessibility recovery actions"
```

---

### Task 5: Full Verification, Packaging, and Push

**Files:**
- No source changes expected unless verification finds a defect.

**Step 1: Run full frontend tests**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test
```

Expected: PASS. All Vitest tests pass.

**Step 2: Run Rust format and library tests**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo fmt --check
cargo test --lib -- --nocapture
```

Expected: PASS. Formatting is clean and all Rust library tests pass.

**Step 3: Build frontend**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm run build
```

Expected: PASS. TypeScript and Vite production build complete.

**Step 4: Build Tauri release bundles**

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
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
plutil -p "src-tauri/target/release/bundle/macos/Prompt Picker.app/Contents/Info.plist" | rg "LSUIElement|CFBundleIdentifier|NSAppleEventsUsageDescription"
strings "src-tauri/target/release/bundle/macos/Prompt Picker.app/Contents/MacOS/prompt-picker" | rg "open-accessibility-settings|Accessibility permission required|Native paste event failed|Native return event failed"
codesign -dv --verbose=4 "src-tauri/target/release/bundle/macos/Prompt Picker.app" 2>&1 | rg "Identifier|Signature|TeamIdentifier|Info.plist"
```

Expected:
- `LSUIElement` is true.
- `CFBundleIdentifier` is `local.promptpicker.dev`.
- `NSAppleEventsUsageDescription` exists.
- Binary contains `open-accessibility-settings`.
- If `Signature=adhoc`, final notes must explicitly say macOS may require re-granting Accessibility permission for the exact built app.

**Step 6: Inspect staged changes**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git status --short
git diff --staged --check
```

Expected:
- No whitespace errors.
- Only source/test files from this plan are staged.
- Generated `dist`, `node_modules`, `src-tauri/target`, `.app`, and `.dmg` changes are not included in the source commit unless the repo policy explicitly tracks them.

**Step 7: Push**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git push origin main
git rev-parse HEAD
git rev-parse @{u}
```

Expected: local `HEAD` and upstream hash match.

**Step 8: Final user-facing report**

Report:

```text
点击小猫
→ 选择提示词
→ 成功：小猫显示“已发送”
→ 缺权限：小猫显示“开启辅助功能”，点击可打开系统设置
→ 粘贴失败：小猫显示“未能粘贴”
→ 粘贴成功但回车失败：小猫显示“已粘贴，未发送”
```

Include:
- Commit hash.
- Verification commands and pass counts.
- App and DMG paths.
- The macOS Accessibility permission boundary: code cannot bypass it; ad-hoc builds may need reauthorization after replacing the app.

