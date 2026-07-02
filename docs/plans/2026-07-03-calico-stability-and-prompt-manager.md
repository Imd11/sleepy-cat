# Calico Stability And Prompt Manager Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Stabilize the Calico floating button position, make the main app open directly to prompt management, and move floating-button controls into menu bar/right-click control surfaces.

**Architecture:** Treat the saved floating button position as the single source of truth, and make the React polling loop cancellable so only one active loop can move the Calico window. Keep the main Tauri window focused on prompt library management, while menu bar and right-click Calico controls handle lower-frequency app controls.

**Tech Stack:** Tauri 2, Rust, React, TypeScript, Vitest, Cargo tests, macOS non-activating panel windows.

---

### Task 1: Add Regression Tests For The Floating Button Jump

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/useInputTargetPolling.test.ts`

**Step 1: Write a failing test for stale polling cleanup**

Add a test that proves an old polling loop cannot move Calico after settings change:

```ts
it("cancels the previous polling loop when saved position changes", async () => {
  getFrontmostApp.mockResolvedValue({ name: "Prompt Picker", bundle_id: "local.promptpicker.dev" });
  getCurrentInputTarget.mockResolvedValue(null);

  const { rerender } = renderHook(
    ({ position }) =>
      useInputTargetPolling(
        [],
        { buttonOffset: null, buttonPosition: position },
        {},
        true
      ),
    { initialProps: { position: null as { x: number; y: number } | null } }
  );

  await act(async () => {
    vi.advanceTimersByTime(1500);
  });
  expect(showPromptButton).toHaveBeenLastCalledWith(960, 700);

  vi.clearAllMocks();
  rerender({ position: { x: 1765, y: 419 } });

  await act(async () => {
    vi.advanceTimersByTime(4000);
  });

  expect(showPromptButton).toHaveBeenCalled();
  expect(showPromptButton).not.toHaveBeenCalledWith(960, 700);
  expect(showPromptButton).toHaveBeenLastCalledWith(1765, 419);
});
```

**Step 2: Run the failing test**

Run:

```bash
npm test -- --run src/overlay/useInputTargetPolling.test.ts
```

Expected before implementation: FAIL, because the old scheduled timeout can continue running and call `showPromptButton(960, 700)`.

**Step 3: Commit after this task passes later**

Do not commit yet. This test should fail before Task 2.

---

### Task 2: Make Input Target Polling Cancellable And Single-Owner

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/useInputTargetPolling.ts`
- Test: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/useInputTargetPolling.test.ts`

**Step 1: Replace raw `setTimeout` calls with one scheduler**

Inside `useInputTargetPolling`, add:

```ts
const timeoutRef = useRef<ReturnType<typeof window.setTimeout> | null>(null);
const generationRef = useRef(0);
```

Inside the polling `useEffect`, replace `activeRef` ownership with a local generation:

```ts
const generation = generationRef.current + 1;
generationRef.current = generation;
let cancelled = false;

const isCurrent = () => !cancelled && generationRef.current === generation;

const clearScheduledPoll = () => {
  if (timeoutRef.current) {
    window.clearTimeout(timeoutRef.current);
    timeoutRef.current = null;
  }
};

const schedulePoll = (delay: number) => {
  if (!isCurrent()) return;
  clearScheduledPoll();
  timeoutRef.current = window.setTimeout(() => {
    timeoutRef.current = null;
    void poll();
  }, delay);
};
```

**Step 2: Guard every async boundary**

In `poll`, after every awaited platform call, check `isCurrent()` before moving the window:

```ts
const app = await getFrontmostApp();
if (!isCurrent()) return;

const inputTarget = (await getCurrentInputTarget()) as InputTarget | null;
if (!isCurrent()) return;
```

Replace every `setTimeout(poll, delay)` with `schedulePoll(delay)`.

**Step 3: Cleanup properly**

The effect cleanup must cancel the active loop:

```ts
return () => {
  cancelled = true;
  clearScheduledPoll();
};
```

Remove unused `activeRef`, `pollingRef`, and `prevVisibilityRef` if they are no longer needed.

**Step 4: Run the polling tests**

Run:

```bash
npm test -- --run src/overlay/useInputTargetPolling.test.ts
```

Expected: PASS, including the new stale-poll regression.

**Step 5: Commit**

```bash
git add src/overlay/useInputTargetPolling.ts src/overlay/useInputTargetPolling.test.ts
git commit -m "fix: prevent stale calico position polling"
```

---

### Task 3: Wait For Settings Before Starting Main-Window Polling

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`

**Step 1: Add a failing App-level test**

Add or update a test that proves the main window does not start polling until settings have loaded:

```ts
it("waits for saved settings before starting main window polling", async () => {
  // Mock settings read to resolve with saved overlayPlacement.buttonPosition.
  // Assert useInputTargetPolling is not called before the read resolves.
  // Assert it is called once after settings resolve, with buttonPosition from settings.
});
```

Use the existing mocked `inputTargetPollingMock` in `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`.

**Step 2: Implement `settingsLoaded` gating**

In `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`, add:

```ts
const [settingsLoaded, setSettingsLoaded] = useState(false);
```

Change the initial loading effect to avoid setting state after unmount:

```ts
useEffect(() => {
  let active = true;

  storeRef.current.list().then((items) => {
    if (active) setPrompts(items);
  });

  settingsStoreRef.current.get().then((loadedSettings) => {
    if (!active) return;
    setActiveSettings(loadedSettings);
    setSettingsLoaded(true);
  });

  let label = initialWindowLabel();
  try {
    label = getCurrentWindow().label;
  } catch {
    label = initialWindowLabel();
  }
  setWindowLabel(label);
  if (label === "main") {
    refreshAccessibilityStatus();
  }

  return () => {
    active = false;
  };
}, [refreshAccessibilityStatus]);
```

Only render the polling controller after settings are loaded:

```tsx
const pollingController =
  windowLabel === "main" && settingsLoaded ? (
    <InputTargetPollingController
      settings={activeSettings}
      onButtonDragEnd={handleButtonDragEnd}
    />
  ) : null;
```

**Step 3: Run App tests**

Run:

```bash
npm test -- --run src/app/App.test.tsx
```

Expected: PASS.

**Step 4: Commit**

```bash
git add src/App.tsx src/app/App.test.tsx
git commit -m "fix: wait for settings before positioning calico"
```

---

### Task 4: Remove Unconditional Startup Repositioning

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`
- Test: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`

**Step 1: Remove the Rust setup default move**

Remove this setup-time positioning call:

```rust
let _ = show_prompt_button(960.0, 700.0, app.handle().clone());
```

The frontend polling controller should be responsible for showing Calico after settings load. This prevents startup from placing Calico at `960,700` before the saved position is known.

**Step 2: Verify first-run fallback still exists**

Keep the existing frontend fallback behavior: if no saved `buttonPosition` exists, `useInputTargetPolling` uses `DEFAULT_BUTTON_POSITION`.

Run:

```bash
npm test -- --run src/overlay/useInputTargetPolling.test.ts src/app/App.test.tsx
cargo test
```

Expected: PASS.

**Step 3: Commit**

```bash
git add src-tauri/src/lib.rs src/app/App.test.tsx
git commit -m "fix: let settings-driven polling own calico startup position"
```

---

### Task 5: Make The Main Window Open Directly To Prompt Management

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`
- Optional modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/styles.css`

**Step 1: Add or update tests for default main-window mode**

Update the test that currently expects the dashboard/home page. The new expectation:

```ts
it("opens the main window directly on prompt management", async () => {
  render(<App />);
  expect(await screen.findByRole("heading", { name: "Manage Prompts" })).toBeInTheDocument();
  expect(screen.queryByText("Floating Button")).not.toBeInTheDocument();
  expect(screen.queryByText("Settings")).not.toBeInTheDocument();
});
```

**Step 2: Change initial mode selection**

In `App.tsx`, default the main window to manager mode while preserving popover windows:

```ts
const [mode, setMode] = useState<AppMode>(() => {
  const initialMode = new URLSearchParams(window.location.search).get("mode");
  if (initialMode === "manager") return "manager";
  if (initialMode === "settings") return "settings";
  if (initialMode === "button-controls") return "button-controls";
  return initialWindowLabel() === "main" ? "manager" : "popover";
});
```

**Step 3: Remove the main dashboard branch**

Delete the `windowLabel === "main" && mode === "popover"` branch that renders `MainWindow`.

Delete the `MainWindow` subcomponent if it becomes unused.

**Step 4: Remove the manager Back button for the main window**

The manager is now the main page, so remove the footer Back button from the manager branch. Keep navigation for modal/popover contexts only if there is a concrete use case.

**Step 5: Run tests**

Run:

```bash
npm test -- --run src/app/App.test.tsx
```

Expected: PASS.

**Step 6: Commit**

```bash
git add src/App.tsx src/app/App.test.tsx src/styles.css
git commit -m "feat: open main window to prompt manager"
```

---

### Task 6: Improve The Prompt Manager Page UX

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/ui/PromptManager.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/styles.css`
- Test: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`

**Step 1: Make the page feel like the product's primary workspace**

Target layout:

```text
Prompt Picker                                      Import  Export
Manage reusable prompts shown in the floating picker.

┌────────────────────────────────────────────────────────┐
│ New Prompt                                             │
│ Title                                                  │
│ Body                                                   │
│                                           Add Prompt    │
└────────────────────────────────────────────────────────┘

Prompt List
┌────────────────────────────────────────────────────────┐
│ 讨论方案                                      ↑ ↓ Edit Delete │
│ 使用 brainstorming skill，先和我讨论方案...                 │
└────────────────────────────────────────────────────────┘
```

**Step 2: Keep cards only for repeated prompt items and the editor**

Do not reintroduce large dashboard cards for floating button or settings. The page should be dense but readable.

**Step 3: Avoid misleading settings/status controls**

Do not show `Hide Floating Button`, `Status: Visible`, or `Autosend: Ready` on this page.

**Step 4: Run app tests**

Run:

```bash
npm test -- --run src/app/App.test.tsx
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/ui/PromptManager.tsx src/styles.css src/app/App.test.tsx
git commit -m "style: focus main page on prompt management"
```

---

### Task 7: Rename And Simplify Menu Bar Actions

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`
- Test: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`

**Step 1: Update menu copy**

Change menu labels to:

```text
Manage Prompts...
Show Calico
Hide Calico
Open Accessibility Settings
----------------------------
Quit Prompt Picker
```

Replace `"Open Prompt Picker"` with `"Manage Prompts..."`.

**Step 2: Keep behavior simple**

`Manage Prompts...` continues to call `open_main_window`. Because the main window now defaults to manager mode, no new route is needed.

**Step 3: Add a Rust test if menu labels are currently testable**

If the existing menu helper only maps IDs to actions, keep tests focused on action mapping. Do not over-engineer a menu label abstraction unless it reduces duplication.

**Step 4: Run Rust tests**

Run:

```bash
cargo test
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "chore: clarify menu bar actions"
```

---

### Task 8: Add Right-Click Calico Control Menu

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/App.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/platform/platformApi.ts`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/src/lib.rs`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/app/App.test.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/overlay/overlayHtml.test.ts`
- Optional modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/styles.css`

**Step 1: Add backend command for quit**

Add a Tauri command:

```rust
#[tauri::command]
fn quit_prompt_picker(app: tauri::AppHandle) {
    app.exit(0);
}
```

Register it in `tauri::generate_handler!`.

**Step 2: Add frontend API wrapper**

In `platformApi.ts`:

```ts
export async function quitPromptPicker(): Promise<void> {
  return invoke("quit_prompt_picker");
}
```

**Step 3: Replace button-controls UI**

The right-click Calico panel should be:

```text
Manage Prompts...
Hide Calico
Open Accessibility Settings
---------------------------
Quit Prompt Picker
```

Behavior:

- `Manage Prompts...`: call `openMainWindow()`, then `hidePromptPopover()`.
- `Hide Calico`: set `floatingButton.visible=false`, call `hidePromptButton()`, then `hidePromptPopover()`.
- `Open Accessibility Settings`: call `openAccessibilitySettings()`.
- `Quit Prompt Picker`: call `quitPromptPicker()`.

**Step 4: Keep left-click behavior unchanged**

Do not change `/Users/yang/Desktop/GitHub-pre/prompt-picker/public/overlay.html` left-click behavior:

```text
left click Calico -> begin_prompt_pick_session -> show_prompt_popover_from_button
```

**Step 5: Update tests**

Update `App.test.tsx` to expect the new button-controls labels.

Update `overlayHtml.test.ts` only if right-click assertions need new labels or command references.

**Step 6: Run tests**

Run:

```bash
npm test -- --run src/app/App.test.tsx src/overlay/overlayHtml.test.ts
cargo test
```

Expected: PASS.

**Step 7: Commit**

```bash
git add src/App.tsx src/platform/platformApi.ts src-tauri/src/lib.rs src/app/App.test.tsx src/overlay/overlayHtml.test.ts src/styles.css
git commit -m "feat: add calico right-click controls"
```

---

### Task 9: Full Regression Verification

**Files:**
- No code changes unless tests reveal a bug.

**Step 1: Run all frontend tests**

Run:

```bash
npm test -- --run
```

Expected: all Vitest tests pass.

**Step 2: Run all Rust tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test
```

Expected: all Rust tests pass.

**Step 3: Build signed app**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm run tauri:build:signed
```

Expected output includes:

```text
Built application at:
/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app
/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/dmg/Prompt Picker_1.0.0_aarch64.dmg
```

**Step 4: Install and launch `/Applications` build**

Run:

```bash
APP_SRC="/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app"
APP_DST="/Applications/Prompt Picker.app"
osascript -e 'tell application id "local.promptpicker.dev" to quit' >/dev/null 2>&1 || true
pkill -TERM -f '/Prompt Picker.app/Contents/MacOS/prompt-picker' >/dev/null 2>&1 || true
sleep 1
rm -rf "$APP_DST"
ditto "$APP_SRC" "$APP_DST"
open "$APP_DST"
```

**Step 5: Manual smoke test**

Verify:

- Calico stays at the saved position after opening the main window.
- Calico does not bounce between default, saved, and dragged positions.
- Menu bar first item reads `Manage Prompts...`.
- Main window opens directly to prompt management.
- The main page does not show `Floating Button`, `Hide Floating Button`, or the large `Settings` card.
- Right-click Calico shows management/control actions.
- Left-click Calico still opens the prompt list.
- Selecting a prompt still pastes and presses Return in Codex.
- Selecting a prompt still pastes and presses Return in WeChat.

**Step 6: Commit final verification-only changes if any**

If no files changed, do not commit.

If test fixes were needed:

```bash
git add <changed-files>
git commit -m "test: cover calico prompt manager flow"
```

---

### Task 10: Final Report

**Files:**
- No code changes.

**Step 1: Summarize user-visible result**

Report in Chinese:

```text
现在用户看到的效果：
1. 打开菜单栏的 Prompt Picker 后，直接进入提示词管理页面。
2. 小猫稳定停在用户拖动的位置，打开主页面不会乱跳。
3. 左键小猫打开提示词列表。
4. 右键小猫打开控制菜单，可以管理提示词、隐藏小猫、打开权限设置、退出 App。
5. 点击提示词仍然会粘贴并回车发送。
```

**Step 2: Include verification**

Report exact commands run:

```text
npm test -- --run
cargo test
npm run tauri:build:signed
```

**Step 3: Include artifact locations**

Report:

```text
App: /Applications/Prompt Picker.app
Bundle: /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app
DMG: /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/dmg/Prompt Picker_1.0.0_aarch64.dmg
```
