# Automatic Claude and WeChat Input Focus Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let users click Calico and select a prompt to automatically paste, or paste and submit, into the correct composer in Codex, Claude Desktop, and WeChat without first clicking the input field.

**Architecture:** Preserve Codex's existing first-responder fast path while adding a fail-closed macOS automation state machine for apps that require explicit input focus. Capture an immutable target session before the picker opens; AX profiles such as Claude and WeChat then resolve a trusted process group, discover one unambiguous composer inside the captured window, verify exact focus before and after paste, and submit only when every safety gate passes.

**Tech Stack:** Tauri 2, Rust 2021, macOS Accessibility API (`AXUIElement`), AppKit (`NSRunningApplication`, `NSPanel`), CoreGraphics (`CGEvent`, `CGWindowListCopyWindowInfo`), React, TypeScript, Vitest, Cargo tests.

---

## Scope And Non-Negotiable Behavior

- The user does **not** click the target input field first. Clicking Calico is the only prerequisite interaction.
- The prompt picker remains a never-key panel and must not switch a full-screen app back to the desktop.
- `paste_only` must be structurally unable to emit Return or Command-Return.
- Codex keeps its working activation/first-responder route. Claude and WeChat changes must not force AX scanning or coordinate clicks onto Codex.
- Claude uses its Electron browser/application AX tree. `AXManualAccessibility` is enabled only when a read-only probe proves the normal tree is insufficient.
- WeChat is treated as a process group: the main `com.tencent.xinWeChat` process plus only validated `com.tencent.flue.WeChatAppEx` application processes. Renderer/helper PIDs are not assumed to be keyboard targets.
- Never select the first visible app as a target.
- Never click the current mouse location, the Calico position, or a guessed bottom-center window coordinate.
- Never treat `AXWebArea` itself as a composer.
- Never consider `AXFocused=true` successful until the exact candidate is read back from the system/app AX roots.
- Never retry paste when paste completion is unknown, and never retry submit when submit completion is unknown.
- Diagnostics must not record prompt text, conversation text, clipboard content, or raw `AXValue`; record only lengths, normalized hashes, enum-like attributes, geometry, PIDs, errors, and timings.
- Do not modify prompt management, category, import/export, sync, Calico animation, or unrelated window styling.
- Keep the existing clipboard behavior during this focus-reliability project. Full multi-type clipboard restoration is a separate follow-up because it introduces asynchronous pasteboard-provider and restore-timing risks.
- Keep non-Codex/Claude/WeChat applications on an explicit compatibility route; do not silently apply Claude/WeChat AX heuristics to unknown apps.
- Copy immutable session data while holding `Mutex` guards, then release every guard before AX calls, process queries, sleeps, or Tauri/AppKit dispatch. Never hold application state locks across blocking work.
- Run bounded AX traversal on the existing `spawn_blocking` worker path. Dispatch `NSRunningApplication` activation and all `NSWindow`/`NSPanel` operations through the existing main-thread helper; keep those main-thread closures short and never perform AX tree traversal inside them.

## Current Worktree Preconditions

- Execute in `/Users/yang/Desktop/GitHub-pre/prompt-picker/.worktrees/macos-popover-main-thread`.
- The worktree currently contains an uncommitted native AX prototype in `src-tauri/src/platform/macos.rs`. Treat it as input to this plan; do not discard, revert, or publish it before the tasks below make it safe.
- Before each task, run `git status --short` and inspect unexpected changes. Work with user changes; never reset them.
- Do not edit generated output under `dist`, `src-tauri/target`, or `node_modules`.
- Use @karpathy-guidelines while implementing, @superpowers:test-driven-development for behavioral changes, and @superpowers:verification-before-completion before declaring completion.

## Target User Flow

```text
Target app is visible (Codex / Claude / WeChat)
  -> User clicks Calico
  -> Backend captures the exact app instance and window
  -> Picker opens without becoming key or switching Space
  -> User selects a prompt
  -> Backend resolves and verifies the target composer automatically
  -> paste_only: text appears once and no submit key is created
  -> paste_and_submit: text appears once, focus is reverified, one submit key is sent
  -> Original clipboard is restored only if the user did not change it meanwhile
```

## Failure Semantics

```text
Before paste fails     -> no clipboard replacement, no keyboard event, no submit
After paste is unknown -> do not paste again, do not submit
Before submit changes  -> do not submit
After submit unknown   -> do not submit again
Target/app/window lost -> abort; never select another visible app
```

---

### Task 0: Synchronize The Worktree Without Losing The Native AX Prototype

**Files:**
- Preserve: `src-tauri/src/platform/macos.rs`
- Preserve: `docs/plans/2026-07-12-automatic-claude-wechat-input-focus.md`

**Step 1: Inspect local and remote ancestry**

Run:

```bash
git status --short
git fetch origin
git rev-list --left-right --count HEAD...origin/main
git diff --check
```

Expected: record the exact ahead/behind counts. Do not assume the counts remain the same as when this plan was written.

**Step 2: Create two non-destructive recovery copies**

Run:

```bash
mkdir -p /tmp/prompt-drawer-autosend-plan
git diff --binary > /tmp/prompt-drawer-autosend-plan/pre-sync.patch
cp docs/plans/2026-07-12-automatic-claude-wechat-input-focus.md \
  /tmp/prompt-drawer-autosend-plan/implementation-plan.md
test -s /tmp/prompt-drawer-autosend-plan/pre-sync.patch
test -s /tmp/prompt-drawer-autosend-plan/implementation-plan.md
```

Expected: both recovery files exist and are non-empty.

**Step 3: Temporarily stash tracked and untracked work**

```bash
git stash push --include-untracked -m "pre-execution automatic input focus"
git status --short
```

Expected: the worktree is clean and the stash contains the native AX prototype and this plan. Do not drop the stash.

**Step 4: Fast-forward/rebase onto current `origin/main`**

```bash
git rebase origin/main
```

Expected: success without rewriting `origin/main`. Never use `git reset --hard` or force push.

**Step 5: Restore the saved work**

```bash
git stash pop
git status --short
git diff --check
```

Expected:

- the plan file is restored;
- the native AX prototype remains modified;
- no conflict markers or whitespace errors exist.

If a conflict occurs, stop and use @resolving-merge-conflicts. Compare against both `/tmp` recovery files; do not choose one side wholesale.

**Step 6: Verify ancestry before implementation**

```bash
git rev-list --left-right --count HEAD...origin/main
```

Expected: the second count (commits only on `origin/main`) is `0` before Task 1 starts. The first count may be nonzero if this execution branch already contains intentional commits.

---

### Task 1: Freeze The Existing Contracts And Protect The Codex Fast Path

**Files:**
- Modify: `src-tauri/src/platform/macos.rs:55-70`
- Modify: `src-tauri/src/platform/macos.rs` test module
- Modify: `src-tauri/src/lib.rs` test module
- Test: `src-tauri/src/platform/macos.rs`
- Test: `src-tauri/src/lib.rs`

**Step 1: Record the starting diff without changing it**

Run:

```bash
git status --short
git diff -- src-tauri/src/platform/macos.rs > /tmp/prompt-drawer-ax-prototype.patch
git diff --check
```

Expected:

- `src-tauri/src/platform/macos.rs` is listed as modified.
- `/tmp/prompt-drawer-ax-prototype.patch` is non-empty.
- `git diff --check` reports no whitespace errors.

**Step 2: Add failing Codex routing tests**

Add tests that assert the policy remains explicit and isolated:

```rust
#[test]
fn codex_keeps_first_responder_fast_path() {
    assert_eq!(
        input_focus_policy("com.openai.codex"),
        InputFocusPolicy::PreserveApplicationFirstResponder
    );
}

#[test]
fn claude_and_wechat_require_composer_resolution() {
    for bundle_id in [
        "com.anthropic.claudefordesktop",
        "com.tencent.xinWeChat",
    ] {
        assert_eq!(
            input_focus_policy(bundle_id),
            InputFocusPolicy::ResolveEditableElement
        );
    }
}
```

Add a sender test proving `NativeSubmitKey::None` cannot call the submit closure even when paste succeeds.

**Step 3: Run the focused tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib codex_keeps_first_responder_fast_path
cargo test --manifest-path src-tauri/Cargo.toml --lib activating_clipboard_sender_respects_submit_key_none
```

Expected: PASS. If either test fails, stop and restore the contract before continuing.

**Step 4: Add a command-path contract test**

Test that `paste_prompt_and_submit_to_last_target_impl` delegates `submit_key` unchanged and that `none` does not become `enter` in any fallback branch.

**Step 5: Run the Rust test suite**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

Expected: all tests pass.

**Step 6: Create a self-contained local baseline commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/platform/macos.rs \
  docs/plans/2026-07-12-automatic-claude-wechat-input-focus.md
git diff --cached --check
cargo test --manifest-path src-tauri/Cargo.toml --lib
git commit -m "refactor: establish native input focus baseline"
```

Expected: the commit is local only, contains the existing native AX prototype plus its compatibility tests, and can be checked out and tested independently. Do not push this intermediate commit before the remaining safety tasks are complete and reviewed.

---

### Task 2: Add A Read-Only AX And Process Diagnostic Probe

**Files:**
- Create: `src-tauri/src/platform/macos/ax_client.rs`
- Create: `src-tauri/src/platform/macos/ax_diagnostics.rs`
- Modify: `src-tauri/src/platform/macos.rs`
- Test: `src-tauri/src/platform/macos/ax_client.rs`
- Test: `src-tauri/src/platform/macos/ax_diagnostics.rs`

**Step 1: Write failing redaction and budget tests**

Define a serializable diagnostic model that excludes raw values:

```rust
#[derive(Debug, Serialize)]
struct AxCandidateDiagnostic {
    owner_pid: u32,
    role: Option<String>,
    subrole: Option<String>,
    frame: Option<CandidateInput>,
    enabled: Option<bool>,
    focused: Option<bool>,
    focused_settable: bool,
    value_settable: bool,
    text_length: Option<usize>,
    semantic_hash: Option<String>,
    depth: usize,
}
```

Tests must prove:

- serialization contains no `raw_value`, `prompt`, or clipboard field;
- traversal stops at the configured node, depth, and elapsed-time budgets;
- diagnostics are disabled unless `PROMPT_DRAWER_AX_DIAGNOSTICS=1`.

**Step 2: Run the failing tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib ax_diagnostics
```

Expected: FAIL because the diagnostics module does not exist.

**Step 3: Extract the minimum reusable read-only AX client**

Move the prototype's CF ownership, attribute-copy, string/bool/frame conversion, `AXUIElementGetPid`, and bounded child-query operations into `ax_client.rs`. Do not duplicate FFI in the diagnostics module. Apply `AXUIElementSetMessagingTimeout` to every AX object before querying that object; setting it only on the application root is insufficient.

Keep this step read-only: no focus setting, window raising, mouse event, key event, clipboard write, or `AXManualAccessibility` mutation.

**Step 4: Implement the minimal read-only probe**

Implement:

```rust
pub(super) fn collect_frontmost_input_diagnostics(
    limits: AxTraversalLimits,
) -> Result<AxDiagnosticReport, AxError>
```

The report must include:

- macOS version;
- bundle ID and app version;
- main PID and discovered application/helper PIDs;
- active/focused window identity and frame;
- candidate role, subrole, PID, frame, settable flags, score reasons;
- nodes visited, maximum depth, elapsed time, and each AX error;
- whether a usable tree existed before `AXManualAccessibility`.

Do not set focus, click, paste, raise a window, or submit from this function.

The probe must accept an explicit target selector:

```text
PROMPT_DRAWER_AX_TARGET_BUNDLE_ID
PROMPT_DRAWER_AX_TARGET_PID (optional, preferred when supplied)
PROMPT_DRAWER_AX_WAIT_MS (default 3000)
```

Resolve the specified application after the wait interval. Do not infer the target from whichever app happens to be frontmost when `cargo test` starts.

**Step 5: Add ignored manual probe tests**

```rust
#[test]
#[ignore = "requires a frontmost real app and accessibility permission"]
fn print_target_ax_input_diagnostics() {
    assert_eq!(std::env::var("PROMPT_DRAWER_AX_DIAGNOSTICS").as_deref(), Ok("1"));
    let selector = DiagnosticTargetSelector::from_env().unwrap();
    let report = collect_target_input_diagnostics(selector, AxTraversalLimits::diagnostic());
    println!("{}", serde_json::to_string_pretty(&report.unwrap()).unwrap());
}
```

Add a separate ignored test named `print_target_ax_manual_accessibility_comparison`. It must require both `PROMPT_DRAWER_AX_DIAGNOSTICS=1` and `PROMPT_DRAWER_AX_ALLOW_MANUAL_ACCESSIBILITY=1`. Only this explicitly opted-in test may set `AXManualAccessibility=true`; it must create a fresh AX root before collecting the second report and must never toggle the value back to false.

**Step 6: Run automated tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib ax_diagnostics
```

Expected: PASS; ignored real-app test remains ignored.

**Step 7: Run the read-only probe against each app**

Start the command, then switch to the requested target during the configured wait. Prefer passing a known PID so multiple instances cannot be confused:

```bash
PROMPT_DRAWER_AX_DIAGNOSTICS=1 \
PROMPT_DRAWER_AX_TARGET_BUNDLE_ID=com.anthropic.claudefordesktop \
PROMPT_DRAWER_AX_WAIT_MS=3000 \
  cargo test --manifest-path src-tauri/Cargo.toml --lib \
  print_target_ax_input_diagnostics -- --ignored --nocapture
```

Expected:

- Codex report identifies its main application process without changing focus.
- Claude report shows whether its normal Electron AX tree contains a composer candidate.
- WeChat report records both the main application and any `WeChatAppEx` application process/window relationship.
- No prompt or conversation text is printed.

Save sanitized outputs under `/tmp/prompt-drawer-ax-probes/`; do not commit raw machine diagnostics. Run the manual-accessibility comparison only for a target whose read-only report proves the Electron tree is empty or structurally insufficient.

**Step 8: Commit the diagnostic infrastructure**

```bash
git add src-tauri/src/platform/macos.rs \
  src-tauri/src/platform/macos/ax_client.rs \
  src-tauri/src/platform/macos/ax_diagnostics.rs
git commit -m "test: add read-only macos input diagnostics"
```

---

### Task 3: Make Target Capture Immutable And Fail Closed

**Files:**
- Modify: `src-tauri/src/lib.rs:112-170`
- Modify: `src-tauri/src/lib.rs:1500-1644`
- Modify: `src-tauri/src/platform/macos.rs`
- Modify: `src-tauri/src/platform/unsupported.rs`
- Modify: `public/overlay-interaction.html:240-275`
- Test: `src-tauri/src/lib.rs`
- Test: `src/overlay/overlayHtml.test.ts`

**Step 1: Write failing target-session tests**

Introduce an immutable identity model:

```rust
#[derive(Clone, Debug, PartialEq)]
struct TargetApplicationIdentity {
    bundle_id: String,
    main_pid: u32,
    launch_identity: ProcessLaunchIdentity,
}

#[derive(Clone, Debug, PartialEq)]
struct TargetWindowIdentity {
    owner_pid: u32,
    frame: CandidateInput,
    role: Option<String>,
    title_hash: Option<String>,
    cg_window_id: Option<u32>,
}

struct CapturedTargetIdentity {
    application: TargetApplicationIdentity,
    window: Option<TargetWindowIdentity>,
}
```

Define `ProcessLaunchIdentity` from public process start metadata (for example the seconds/microseconds start fields returned by `proc_pidinfo`/`proc_bsdinfo`), not from PID alone.

Add a platform function with matching macOS/non-macOS signatures:

```rust
pub fn process_launch_identity(pid: u32) -> Option<ProcessLaunchIdentity>;
```

On macOS, read process start time using a small RAII-safe `proc_pidinfo(PROC_PIDTBSDINFO, ...)` wrapper and combine seconds plus microseconds/nanoseconds into the identity. In `unsupported.rs`, return `None`; callers must retain existing non-macOS behavior rather than rejecting every target.

`cg_window_id` is diagnostic/supporting evidence only. macOS does not provide a reliable public one-to-one bridge between an arbitrary third-party AX window and a CoreGraphics window ID. During one operation, keep the live AX window element and use `CFEqual`; across asynchronous boundaries, match the window by owner PID plus frame, role, and title hash with a documented tolerance.

Window evidence is profile-dependent. Claude and WeChat require an unambiguous captured window before AX discovery. Codex may expose no `AXWindows`; its fast path requires the exact application instance but treats window evidence as best-effort so this project does not break the currently working activation/first-responder behavior. The legacy compatibility route follows the same best-effort rule.

Add tests proving:

- capture preserves PID and target window;
- Prompt Picker frontmost with no recent exact target returns `None`;
- an unrelated first visible app is never selected;
- a restarted process with the same bundle ID is rejected;
- a replaced window or a window fingerprint outside tolerance is rejected;
- Claude/WeChat reject a missing window identity;
- Codex remains usable when the application identity is valid but no AX window is exposed;
- a newer session invalidates an older async result.

**Step 2: Run the failing tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib prompt_pick_session_target
cargo test --manifest-path src-tauri/Cargo.toml --lib target_application_identity
npm test -- src/overlay/overlayHtml.test.ts
```

Expected: at least the visible-app fallback and PID preservation tests fail.

**Step 3: Capture at Calico pointerdown before opening the picker**

Keep this order in `overlay-interaction.html`:

```text
permission check
-> allocate session ID
-> await begin_prompt_pick_session
-> open/toggle prompt popover
```

Do not require or simulate a click inside the target input field.

**Step 4: Remove unsafe target fallbacks**

Delete the `visible_apps.into_iter().find(...)` fallback from `prompt_pick_session_target` and remove `visible_apps` from its production signature if no longer needed.

Change `record_last_input_target_if_valid` to preserve the captured PID instead of writing `pid: None`.

Do not use `current_pointer_location()` to create an input click point.

**Step 5: Validate target identity at command time**

Before composer discovery, require:

- same process launch identity;
- same captured live AX window or matching window fingerprint within the documented tolerance;
- target process still running;
- frontmost is either the target app or Prompt Drawer itself;
- if a third app is frontmost, abort without activation.

**Step 6: Run focused tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib prompt_pick_session_target
cargo test --manifest-path src-tauri/Cargo.toml --lib target_application_identity
npm test -- src/overlay/overlayHtml.test.ts
```

Expected: PASS.

**Step 7: Commit**

```bash
git add public/overlay-interaction.html src/overlay/overlayHtml.test.ts \
  src-tauri/src/lib.rs src-tauri/src/platform/macos.rs \
  src-tauri/src/platform/unsupported.rs
git commit -m "fix: bind prompt selection to an exact target window"
```

---

### Task 4: Complete The Bounded Native AX Client

**Files:**
- Modify: `src-tauri/src/platform/macos/ax_client.rs`
- Modify: `src-tauri/src/platform/macos.rs:326-883`
- Modify: `src-tauri/Cargo.toml`
- Test: `src-tauri/src/platform/macos/ax_client.rs`

**Step 1: Write failing low-level wrapper tests**

Define wrappers and pure test doubles for:

```rust
struct OwnedAxElement(AXUIElementRef);
struct OwnedCfValue(CFTypeRef);

#[derive(Clone, Copy)]
struct AxTraversalLimits {
    max_nodes: usize,
    max_depth: usize,
    max_elapsed: Duration,
    per_element_timeout: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AxQueryError {
    CannotComplete,
    InvalidElement,
    Unsupported,
    TimedOut,
    Other(i32),
}
```

Tests must cover:

- Create/Copy values release exactly once;
- values retained from a CFArray remain valid after the array is released;
- invalid elements return `InvalidElement`, not panic;
- traversal stops at each budget;
- timeout is applied to every queried element, not only the application root;
- identity comparison uses `CFEqual`, not raw pointer equality.

**Step 2: Run the failing tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib ax_client
```

Expected: FAIL because `ax_client` does not exist.

**Step 3: Extend the minimal read-only client without duplicating FFI**

Complete the extraction started in Task 2. Move, do not duplicate:

- CF ownership wrappers;
- string/bool/point/size conversion;
- AX attribute copy and settable checks;
- `AXUIElementGetPid`;
- messaging timeout and error mapping.

Add support for:

- `AXSubrole`;
- `AXIdentifier` when exposed;
- `AXPlaceholderValue`, `AXTitle`, `AXDescription`, `AXHelp`;
- `AXWindow`, `AXTopLevelUIElement`, `AXParent`;
- `AXChildren`, `AXVisibleChildren`, and `AXContents`;
- system-wide focused element;
- `AXUIElementCopyElementAtPosition`;
- `AXRaise` on windows.

Do not make raw AX references `Send` or cache them across long-lived sessions.

**Step 4: Add required AppKit features only if used**

Add the minimum `objc2-app-kit` features required for `NSRunningApplication`; do not add pasteboard features or a third-party process/accessibility framework in this project.

**Step 5: Run low-level tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib ax_client
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

**Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock \
  src-tauri/src/platform/macos.rs src-tauri/src/platform/macos/ax_client.rs
git commit -m "refactor: add bounded native accessibility client"
```

---

### Task 5: Resolve Trusted Application Process Groups

**Files:**
- Create: `src-tauri/src/platform/macos/process_group.rs`
- Modify: `src-tauri/src/platform/macos.rs`
- Test: `src-tauri/src/platform/macos/process_group.rs`

**Step 1: Write failing resolver tests**

Define:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
enum ProcessRole {
    MainApplication,
    BrowserApplication,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TrustedProcess {
    pid: u32,
    role: ProcessRole,
    bundle_id: String,
    executable_path: PathBuf,
}
```

Fixture tests must prove:

- Claude main process is accepted; its renderer helpers are rejected;
- WeChat main process is accepted;
- `com.tencent.flue.WeChatAppEx` is accepted only when its executable is inside the captured WeChat bundle, its process ancestry is trusted, and its window overlaps/matches the captured WeChat content window;
- an unrelated process with the same name is rejected;
- a PID reused after capture is rejected by launch identity;
- Codex does not require process-group expansion.

**Step 2: Run the failing tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib process_group
```

Expected: FAIL because the resolver does not exist.

**Step 3: Implement `ProcessGroupResolver`**

Use native application/process/window metadata. Do not select processes solely by display name.

Expose:

```rust
fn resolve_process_group(
    target: &TargetApplicationIdentity,
    window: Option<&TargetWindowIdentity>,
    profile: &InputCapabilityProfile,
) -> Result<Vec<TrustedProcess>, ProcessGroupError>
```

Return `MissingRequiredWindow` when an Accessibility profile requires window evidence and `window` is `None`. For Codex/legacy profiles, resolve only the captured application instance when window evidence is unavailable.

For WeChat, retain the main PID for application activation and the actual candidate owner PID for AX querying/observation. Do not infer that keyboard events should be posted to the candidate PID.

**Step 4: Run resolver tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib process_group
```

Expected: PASS.

**Step 5: Re-run the read-only probe**

Run the ignored probe against Claude and WeChat and confirm each candidate PID is either the main application or a validated browser application process.

**Step 6: Commit**

```bash
git add src-tauri/src/platform/macos.rs src-tauri/src/platform/macos/process_group.rs
git commit -m "feat: resolve trusted macos input process groups"
```

---

### Task 6: Add Declarative Input Capability Profiles

**Files:**
- Create: `src-tauri/src/platform/macos/input_profiles.rs`
- Modify: `src-tauri/src/platform/macos.rs`
- Test: `src-tauri/src/platform/macos/input_profiles.rs`

**Step 1: Write failing profile tests**

Model only proven differences:

```rust
enum InputCapabilityProfile {
    CodexFirstResponder,
    Accessibility(AccessibilityProfile),
    LegacyCapturedTarget,
}

struct AccessibilityProfile {
    process_scope: ProcessScope,
    window_identity: WindowIdentityRequirement,
    manual_accessibility: ManualAccessibilityPolicy,
    allowed_roles: &'static [&'static str],
    forbidden_subroles: &'static [&'static str],
    semantic_exclusions: &'static [&'static str],
    submit_key: SubmitKeyPolicy,
    paste_verification: PasteVerificationPolicy,
}

enum WindowIdentityRequirement {
    Required,
    BestEffort,
}

enum PasteVerificationPolicy {
    ValueLengthOrHashChange,
    SelectionRangeChange,
    FocusStableAfterProfiledDelay { min_ms: u64, max_ms: u64 },
    PasteOnlyWithoutSubmitEvidence,
}
```

Tests must assert:

- Codex is `CodexFirstResponder`;
- Claude uses its main browser/application process and `ManualAccessibilityPolicy::OnlyWhenTreeSparse`;
- WeChat uses `MainAndValidatedBrowserApplications`;
- Claude/WeChat require exact window evidence while Codex/legacy compatibility do not;
- all AX profiles forbid `AXSearchField` as a **subrole**;
- no profile permits guessed coordinates or direct `AXWebArea` selection.
- applications other than Codex, Claude, and WeChat use `LegacyCapturedTarget`; they do not inherit Claude/WeChat AX heuristics and never use saved/guessed click coordinates. The route preserves the captured app activation plus existing keyboard delivery only.
- Claude/WeChat submit-capable profiles must select a concrete verification policy proven by Task 2 and Task 15; `PasteOnlyWithoutSubmitEvidence` can never enter the submit phase.

**Step 2: Run the failing tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib input_profiles
```

Expected: FAIL.

**Step 3: Implement the minimal profiles**

Bind Claude and WeChat profiles to bundle ID and observed version range from Task 2. Unknown versions may use generic read-only discovery, but must not inherit version-specific click or submit assumptions. Existing non-target applications remain on `LegacyCapturedTarget`, with exact captured-target validation added but no new composer scan or any coordinate click behavior.

Do not scatter `if bundle_id == ...` through the resolver; keep differences in this module.

**Step 4: Run tests and commit**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib input_profiles
git add src-tauri/src/platform/macos.rs src-tauri/src/platform/macos/input_profiles.rs
git commit -m "feat: define macos input capability profiles"
```

---

### Task 7: Discover One Unambiguous Composer

**Files:**
- Create: `src-tauri/src/platform/macos/composer_resolver.rs`
- Modify: `src-tauri/src/platform/macos.rs:329-680`
- Test: `src-tauri/src/platform/macos/composer_resolver.rs`

**Step 1: Write failing fixture tests**

Build pure fixtures for:

- AppKit `NSTextView` composer plus `NSSearchField`;
- Electron `contenteditable` where focus is on an `AXGroup`/`AXStaticText` descendant and the editable root is an `AXTextArea` ancestor;
- Claude window containing sidebar search and bottom composer;
- WeChat window containing contact search, chat search, and message composer owned by `WeChatAppEx`;
- two equally plausible text areas;
- offscreen, zero-size, disabled, secure, wrong-window, and wrong-PID fields;
- an `AXWebArea` containing an editable descendant.

**Step 2: Run the failing tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib composer_resolver
```

Expected: FAIL.

**Step 3: Implement hard exclusions before scoring**

Reject a candidate when any condition holds:

- `AXSubrole == AXSearchField`;
- secure text subrole;
- disabled, hidden, zero-size, or outside the captured window;
- owner PID not in the trusted process group;
- `AXWindow`/`AXTopLevelUIElement` does not match the captured window;
- search/contact/global-search semantics in normalized title, description, placeholder, help, or identifier;
- candidate is `AXWebArea` rather than an editable descendant.

**Step 4: Implement bounded discovery**

Search in this order:

1. exact current focused element if it passes all hard constraints;
2. a bounded `AXParent` walk to an editable root;
3. `AXChildren`, `AXVisibleChildren`, and `AXContents` inside the captured window;
4. app-root/window-restricted hit testing only for points inside already discovered candidate frames.

Use the Task 2 probe to add only stable semantic signals. Do not require nonstandard editable-ancestor attributes; use them as optional hints if exposed.

**Step 5: Require threshold and ambiguity margin**

Return a candidate only when:

```text
top_score >= absolute_threshold
and
top_score - second_score >= ambiguity_margin
```

Return `Ambiguous` rather than choosing the lower/last field.

**Step 6: Run tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib composer_resolver
```

Expected: PASS, including search-field and ambiguous-candidate fixtures.

**Step 7: Commit**

```bash
git add src-tauri/src/platform/macos.rs src-tauri/src/platform/macos/composer_resolver.rs
git commit -m "feat: resolve an unambiguous target composer"
```

---

### Task 8: Focus And Verify The Exact Composer

**Files:**
- Create: `src-tauri/src/platform/macos/focus_controller.rs`
- Modify: `src-tauri/src/platform/macos.rs:683-742`
- Test: `src-tauri/src/platform/macos/focus_controller.rs`

**Step 1: Write failing state-machine tests**

Cover:

- target already focused;
- `AXFocused=true` succeeds and exact candidate is read back;
- set call succeeds but another editable field is focused;
- focus briefly succeeds then is reset by the app;
- candidate becomes invalid during focus;
- bounded polling observes the exact focus becoming stable;
- precise hit-test returns the candidate or editable descendant;
- hit-test returns another element and click is refused;
- user switches to a third app during focus and the session aborts.

**Step 2: Run the failing tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib focus_controller
```

Expected: FAIL.

**Step 3: Implement candidate identity**

Use `CFEqual` while the element is live. Also maintain a diagnostic fingerprint:

```rust
struct ComposerFingerprint {
    owner_pid: u32,
    window_fingerprint: TargetWindowIdentity,
    role: String,
    subrole: Option<String>,
    identifier_hash: Option<String>,
    frame: CandidateInput,
}
```

Do not compare raw pointers.

**Step 4: Implement focus order**

```text
target still frontmost -> no activation
Prompt Drawer unexpectedly frontmost -> activate captured application instance and raise captured window
third app frontmost -> abort
set candidate AXFocused=true
bounded poll of system-wide and process-root focused elements
read system-wide and process-root focused elements
verify exact candidate or verified editable descendant/root relationship
verify again after a short stability interval
```

Use bounded polling for the first production implementation. Do not add `AXObserver` to the production path in this task; it requires an owned CFRunLoop source, callback context, cancellation, and teardown on the creating thread. If profiling later proves polling inadequate, add observer support in a separate plan with explicit lifecycle tests.

`AXRaise` applies to the captured window, not to the composer.

**Step 5: Add exact-click fallback**

Click only when:

- the profile permits it;
- AX focus was rejected;
- candidate frame is still in the captured window;
- `AXUIElementCopyElementAtPosition` returns the candidate or its verified editable descendant.

After clicking, rerun exact focus verification. Delete use of saved mouse, Calico, and guessed bottom-center coordinates for Claude/WeChat.

Preserve insertion semantics:

- if the exact composer is already focused, do not click and preserve its caret/selection;
- if `AXFocused=true` succeeds, do not click afterward;
- if click fallback is required and the composer already contains text, use it only when the validated profile defines a tested caret policy; otherwise abort rather than moving the caret to an arbitrary position;
- never set `AXSelectedTextRange` unless the probe and profile prove that operation is supported and preserves the app's editor state.

**Step 6: Run tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib focus_controller
```

Expected: PASS.

**Step 7: Commit**

```bash
git add src-tauri/src/platform/macos.rs src-tauri/src/platform/macos/focus_controller.rs
git commit -m "feat: focus and verify the exact target composer"
```

---

### Task 9: Preserve Existing Clipboard Semantics And Defer Restoration

**Files:**
- Modify: `src-tauri/src/platform/macos.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/platform/macos.rs`

**Step 1: Write clipboard-ordering contract tests**

Cover:

- target validation and exact focus complete before the prompt replaces clipboard text;
- failure before focus leaves the clipboard untouched;
- prompt text is written once immediately before the paste event;
- paste result unknown never causes a second clipboard write or second paste;
- no clipboard restoration is attempted in this project.

**Step 2: Run the focused tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib clipboard
```

Expected: the pre-focus clipboard test fails if any current branch still copies before target focus succeeds.

**Step 3: Make the minimal ordering correction**

Keep the current product behavior in which the selected prompt becomes the clipboard text. Do not add `NSPasteboard` snapshots or restoration here: promised/lazy pasteboard types and asynchronous Electron paste consumption need a separate product decision and implementation plan.

**Step 4: Run tests and commit**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib clipboard
git add src-tauri/src/lib.rs src-tauri/src/platform/macos.rs
git commit -m "test: protect clipboard ordering during autosend"
```

---

### Task 10: Implement The Verified Paste And Submit Transaction

**Files:**
- Create: `src-tauri/src/platform/macos/autosend_transaction.rs`
- Modify: `src-tauri/src/platform/macos.rs:933-1405`
- Modify: `src-tauri/src/lib.rs:146-305`
- Test: `src-tauri/src/platform/macos/autosend_transaction.rs`
- Test: `src-tauri/src/lib.rs`

**Step 1: Write failing state-machine tests**

Model explicit phases:

```rust
enum AutosendPhase {
    ValidateTarget,
    ResolveComposer,
    FocusComposer,
    VerifyFocus,
    Paste,
    VerifyAfterPaste,
    Submit,
    Complete,
    Aborted,
}
```

Tests must prove:

- pre-paste failure emits no `Cmd+V` and no submit key;
- paste unknown emits no second paste and no submit key;
- focus changes after paste emits no submit key;
- `NativeSubmitKey::None` never enters `Submit`;
- Enter and Command-Enter are emitted exactly once only after verification;
- target process/window change at every phase aborts;
- Codex uses its first-responder fast path without composer scanning or clicking;
- Codex fast path accepts a valid captured application instance when AX window evidence is unavailable;
- Claude/WeChat use exact composer focus;
- Claude/WeChat abort before paste when required window evidence is missing or ambiguous;
- clipboard replacement happens only after exact focus succeeds and occurs once.
- a `paste_only` group joins its bodies and performs one paste with no submit event;
- a submitting group runs one verified transaction per body, preserves the clamped interval, revalidates the same target/composer before every body, and stops at the first failure;
- sequence outcomes preserve `sent_count` and `failed_index` without retrying the failed body;
- `paste_enter`, `paste_command_enter`, and `inherit` preserve the current authoritative backend mapping;
- a settings-file read/parse failure resolves inherited behavior to `NativeSubmitKey::None`.

**Step 2: Run the failing tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib autosend_transaction
```

Expected: FAIL.

**Step 3: Implement the transaction**

Use dependency-injected operations so every failure branch is testable. The production order must be:

```text
validate exact target
-> resolve profile/process group
-> Codex fast path OR resolve composer
-> focus and verify
-> snapshot clipboard
-> write prompt
-> revalidate target/focus
-> post one global Cmd+V
-> observe profile-supported paste evidence
-> revalidate target/focus
-> if submit key != None, post one submit key
-> leave the selected prompt in the clipboard under the existing product behavior
```

Use global `CGEventPost` after focus is verified. Do not make `CGEventPostToPid` the default; retain it only behind a profile capability proven by a diagnostic/acceptance test.

**Step 4: Implement layered paste evidence**

Choose evidence from the Task 2 probe:

- AX value/character-count change when exposed reliably;
- selection/caret change;
- a versioned `FocusStableAfterProfiledDelay` policy only when controlled and approved real-app testing establishes a safe bounded interval for that exact app version.

Do not require a universal `AXValue` change from every Electron contenteditable. Never log raw values: compare length and an in-memory normalized hash only. When no post-paste evidence is available, `paste_only` may complete after verified key delivery, but submit mode must abort for that profile/version. A delay is permitted only as an explicit versioned profile result backed by acceptance evidence, never as a generic fallback.

**Step 5: Remove the production bypass**

Make `paste_prompt_and_submit_to_last_target_impl` use the verified transaction. Remove or restrict direct production calls that activate, paste, sleep 220ms, and submit without exact revalidation.

Route `paste_prompt_sequence_and_submit_to_last_target_impl` through the same transaction factory. Keep the existing behavior that `paste_only` groups are joined into one body. For submitting groups, create a fresh per-body transaction using the same immutable target identity; re-resolve stale AX elements and revalidate target/window/focus before every body. Sleep only between successful bodies, using the existing clamped interval, and never after the final body or after failure.

**Step 6: Run focused tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib autosend_transaction
cargo test --manifest-path src-tauri/Cargo.toml --lib paste_prompt_and_submit_to_session_target
```

Expected: PASS.

**Step 7: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/platform/macos.rs \
  src-tauri/src/platform/macos/autosend_transaction.rs
git commit -m "feat: verify prompt paste before optional submit"
```

---

### Task 11: Map Outcomes To Existing Localized Status UI

**Files:**
- Modify: `src-tauri/src/platform/macos.rs:44-137`
- Modify: `src-tauri/src/platform/unsupported.rs:20-120`
- Modify: `src-tauri/src/platform/mod.rs:1-15` only if a shared outcome type is extracted
- Modify: `src/platform/platformApi.ts:43-99`
- Modify: `src/App.tsx:50-165`
- Modify: `src/shared/i18n.ts`
- Test: `src/app/App.test.tsx`
- Test: `src/platform/platformApi.test.ts`

**Step 1: Write failing outcome tests**

Distinguish:

```text
pasted_only
pasted_and_submitted
target_changed
composer_not_found
composer_ambiguous
focus_not_acquired
paste_not_confirmed
accessibility_permission_missing
```

Represent successful completion explicitly rather than overloading `sent`:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutosendCompletion {
    PastedOnly,
    Submitted,
}

pub struct AutosendOutcome {
    pub copied: bool,
    pub sent: bool,
    pub completion: Option<AutosendCompletion>,
    // existing error/reason fields
}
```

Add the equivalent optional completion field to `AutosendSequenceOutcome` plus `processed_count`. For a joined `paste_only` group, `completion=PastedOnly`, `processed_count=bodies.len()`, and `sent_count=0`. For a submitted sequence, `processed_count` advances with each successfully submitted body and `sent_count` retains its current meaning.

Tests must prove:

- successful `paste_only` is not reported as submitted;
- successful single and group `paste_only` outcomes are rendered as success even though `sent=false`;
- a joined `paste_only` group reports all bodies processed and zero bodies submitted;
- partially failed submitted groups preserve `processed_count`, `sent_count`, and `failed_index` consistently;
- failure before paste does not claim “Copied”;
- failure after an unknown paste does not advise automatic retry;
- messages are available in every currently supported UI language;
- no modal or settings page opens during normal success.
- macOS and non-macOS backends serialize the same outcome/reason schema.

**Step 2: Run the failing tests**

```bash
npm test -- src/app/App.test.tsx src/platform/platformApi.test.ts
```

Expected: FAIL because outcomes are not yet distinct enough.

**Step 3: Extend outcome enums without redesigning UI**

Reuse the existing `prompt-autosend-status` bubble. Determine success from `completion`, not from `sent` alone. Keep messages concise and localized. Do not add new controls, instructional cards, or application-specific UI. Update `unsupported.rs` in the same step so Windows and other non-macOS targets continue to compile and return the same serialized shape.

**Step 4: Run tests and commit**

```bash
npm test -- src/app/App.test.tsx src/platform/platformApi.test.ts
git add src-tauri/src/platform/macos.rs src-tauri/src/platform/unsupported.rs \
  src-tauri/src/platform/mod.rs \
  src/App.tsx src/app/App.test.tsx src/platform/platformApi.ts \
  src/platform/platformApi.test.ts src/shared/i18n.ts
git commit -m "feat: report verified prompt delivery outcomes"
```

---

### Task 12: Add Regression Fixtures For AppKit And Electron Trees

**Files:**
- Create: `src-tauri/tests/fixtures/ax/appkit-composer.json`
- Create: `src-tauri/tests/fixtures/ax/electron-contenteditable.json`
- Create: `src-tauri/tests/fixtures/ax/claude-composer.json`
- Create: `src-tauri/tests/fixtures/ax/wechat-composer.json`
- Create: `src-tauri/tests/fixtures/ax/ambiguous-inputs.json`
- Modify: `src-tauri/src/platform/macos/composer_resolver.rs`

**Step 1: Create sanitized fixtures from the diagnostic schema**

Fixtures contain only structural attributes, hashes, geometry, process roles, and expected selection result. Do not include raw text values.

**Step 2: Write fixture-driven tests**

For every fixture, assert:

- selected candidate fingerprint or expected ambiguity;
- excluded search candidates and reason;
- visited node and elapsed-time budget;
- no candidate outside the captured process group/window is selected.

**Step 3: Run fixture tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib composer_fixture
```

Expected: PASS.

**Step 4: Commit**

```bash
git add src-tauri/tests/fixtures/ax src-tauri/src/platform/macos/composer_resolver.rs
git commit -m "test: cover native and electron composer trees"
```

---

### Task 13: Verify Full-Screen, Spaces, Multi-Display, And Cancellation Contracts

**Files:**
- Modify: `src-tauri/src/windows.rs:600-720`
- Modify: `src-tauri/src/macos_panels.rs:84-145`
- Modify: `src-tauri/src/platform/macos/autosend_transaction.rs`
- Test: `src-tauri/src/windows.rs`
- Test: `src-tauri/src/macos_panels.rs`
- Test: `src-tauri/src/platform/macos/autosend_transaction.rs`

**Step 1: Add regression tests before changing window code**

Assert:

- picker windows are built hidden and configured as never-key before first visible display;
- `canBecomeKeyWindow` and `canBecomeMainWindow` remain false at runtime;
- opening the picker does not activate Prompt Drawer;
- a third-app frontmost transition cancels the autosend session;
- stale session generations cannot paste or submit;
- coordinate transforms support left/right, stacked, and negative-origin displays.

**Step 2: Run the tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib non_activating
cargo test --manifest-path src-tauri/Cargo.toml --lib autosend_session
```

Expected: existing never-key tests pass; any new cancellation/coordinate test fails until implemented.

**Step 3: Make only required fixes**

Do not refactor panel class replacement or collection behaviors unless a failing test or observed defect requires it. The autosend task should preserve the currently working never-key panel implementation.

**Step 4: Run tests and commit**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib
git add src-tauri/src/windows.rs src-tauri/src/macos_panels.rs \
  src-tauri/src/platform/macos/autosend_transaction.rs
git commit -m "test: protect autosend across spaces and displays"
```

---

### Task 14: Run Automated Verification

**Files:**
- No production files unless verification finds a defect

**Step 1: Format and static checks**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
rustup target add x86_64-pc-windows-msvc
cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc
npm run build
```

Expected: all commands exit 0. The Windows check proves changes to shared outcomes and `platform::unsupported` still compile before the existing `.github/workflows/build-windows.yml` performs a real Windows build.

**Step 2: Run all tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib
npm test
```

Expected: all tests pass; ignored real-app probes remain ignored.

**Step 3: Check diffs and scope**

```bash
git diff --check
git status --short
git diff --stat origin/main...HEAD
git diff --name-only origin/main...HEAD
```

Expected:

- no generated files;
- no prompt management, sync, category, animation, or unrelated UI refactor;
- only planned backend, API outcome, localization, test, fixture, and plan files.

**Step 4: Run @superpowers:verification-before-completion**

Do not claim completion from earlier test output. Run the verification commands fresh and record exact results.

**Step 5: Commit verification-only corrections if needed**

If verification reveals a defect, add a failing regression test, implement the minimal correction, rerun the affected suite, and commit separately:

```bash
git commit -m "fix: address verified autosend regression"
```

---

### Task 15: Run Real-App Acceptance And Release Gate

**Files:**
- Create: `docs/verification/2026-07-12-automatic-input-focus.md`
- Modify: `src-tauri/src/platform/macos/input_profiles.rs` only when calibration evidence selects a different verification policy
- Modify: `src-tauri/tests/fixtures/ax/claude-composer.json` only with sanitized structural evidence
- Modify: `src-tauri/tests/fixtures/ax/wechat-composer.json` only with sanitized structural evidence

**Step 1: Confirm the tested application versions**

Record macOS, Prompt Drawer, Codex, Claude Desktop, and WeChat versions. Never record conversation content.

**Step 2: Calibrate paste evidence using `paste_only` only**

With explicit approval and a dedicated test conversation, run a small `paste_only` calibration for Claude and WeChat. Capture only:

- exact composer fingerprint before and after paste;
- value length or normalized in-memory hash change when exposed;
- selected-text/caret range change when exposed;
- exact focus stability duration;
- elapsed time until the observable state stabilizes.

Do not submit during calibration. Do not record raw prompt or conversation text.

**Step 3: Finalize versioned verification profiles**

For each tested Claude/WeChat version:

- choose `ValueLengthOrHashChange` or `SelectionRangeChange` when reliable;
- use `FocusStableAfterProfiledDelay` only when repeated calibration proves a bounded interval and exact focus remains stable;
- otherwise retain `PasteOnlyWithoutSubmitEvidence`, which means paste-and-submit is not yet accepted for that app/version.

Update profile/fixture tests first, then rerun:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib input_profiles
cargo test --manifest-path src-tauri/Cargo.toml --lib autosend_transaction
```

Expected: a submit-capable profile cannot exist without explicit evidence and tests.

**Step 4: Run the user-flow matrix**

For Codex, Claude, and WeChat test:

- no prior input-field click;
- `paste_only`;
- paste and Enter;
- existing text and selection;
- search box currently focused;
- full-screen target;
- target starts in the current Space, then is deliberately moved/switched away; the captured session must abort rather than switching Spaces and sending;
- two displays including negative coordinates;
- Chinese IME candidate window open;
- large prompt;
- prompt selection followed by a deliberate switch to a third app;
- target app exit/restart during selection;
- Accessibility permission revoked.

Do not run paste-and-submit for a profile still marked `PasteOnlyWithoutSubmitEvidence`; record that version as not yet meeting the release gate instead of bypassing the gate.

**Step 5: Run non-destructive repetition gates in controlled fixtures**

Use the dependency-injected transaction harness plus the sanitized AppKit/Electron AX fixtures from Task 12, not real conversations:

- 100 consecutive `paste_only` transactions;
- 100 consecutive paste-and-submit transactions captured by the fake event sink instead of transmitted to a network service;
- zero wrong window;
- zero wrong input field;
- zero unexpected Return;
- zero duplicate paste;
- zero duplicate submit.

Ordinary recognition failures may occur only as a safe abort with no wrong-target action.

**Step 6: Run limited real-app smoke tests only with explicit approval**

Before any real Claude or WeChat submit test, obtain explicit confirmation that a dedicated test conversation/account is selected. Never send repetitive prompts into the user's normal conversations.

For each supported real app/version:

- 10 `paste_only` trials;
- up to 5 paste-and-submit trials in the approved test conversation;
- stop immediately on the first wrong target, wrong field, duplicate action, or unexpected Return.

Codex may use a disposable test task. Claude and WeChat send tests must not run unattended.

**Step 7: Record results**

Document counts, failure categories, timings, and versions. Do not include prompt or conversation text.

**Step 8: Commit profile calibration and acceptance evidence**

```bash
git add docs/verification/2026-07-12-automatic-input-focus.md \
  src-tauri/src/platform/macos/input_profiles.rs \
  src-tauri/tests/fixtures/ax/claude-composer.json \
  src-tauri/tests/fixtures/ax/wechat-composer.json
cargo test --manifest-path src-tauri/Cargo.toml --lib
git commit -m "test: calibrate automatic input focus profiles"
```

---

### Task 16: Final Review, Push, And User-Facing Confirmation

**Files:**
- Modify only files required by review findings

**Step 1: Review against this plan**

Use @superpowers:requesting-code-review with scope limited to this plan. Ask the reviewer to verify:

- exact target identity and no visible-app fallback;
- Codex fast-path preservation;
- Claude/WeChat process-group correctness;
- `AXSearchField` subrole exclusion;
- exact focus verification;
- no guessed coordinate fallback;
- no submit after unknown paste;
- `paste_only` structural no-submit guarantee;
- clipboard replacement occurs only after focus and is never repeated;
- macOS and Windows/unsupported outcome schemas remain compatible;
- test and real-app evidence.

The release gate is not satisfied unless the tested Codex, Claude Desktop, and WeChat versions all pass both `paste_only` and paste-and-submit. A safe abort is correct failure behavior but is not evidence that the requested feature is complete.

**Step 2: Process findings**

Use @superpowers:receiving-code-review. Validate each finding against code and tests; do not blindly accept speculative panel or unrelated UI changes.

**Step 3: Fix only real in-scope findings**

For every accepted finding:

```text
write failing regression test
-> run and observe failure
-> implement minimal fix
-> rerun focused and full suites
-> commit
```

**Step 4: Push only after fresh verification**

Before pushing, read `docs/verification/2026-07-12-automatic-input-focus.md` and confirm all three required app/version rows are accepted for both modes. If Claude or WeChat remains `PasteOnlyWithoutSubmitEvidence`, continue investigation and profile calibration; do not push or declare the plan complete merely because failure is safe.

Because Task 15 may modify versioned profiles after the first automated verification, rerun every command in Task 14 after the final calibration commit. Earlier Task 14 output is not sufficient evidence.

```bash
git status --short
git log --oneline --decorate -10
git fetch origin
git rev-list --left-right --count HEAD...origin/main
git push origin HEAD:main
git status --short
```

Expected: the ancestry check reports that `HEAD` is not behind `origin/main`, push succeeds without `--force`, and the working tree contains no uncommitted task files. If `origin/main` advanced, integrate it first and rerun Task 14 before pushing.

**Step 5: Report the user-visible result**

Describe only observable behavior:

- clicking Calico works without first clicking the composer;
- picker stays in the current full-screen/Space context;
- Codex remains compatible;
- Claude and WeChat receive text in the correct composer;
- `paste_only` never sends;
- paste-and-submit sends once;
- unsafe or ambiguous states stop without acting on another app or field.

Do not claim support for untested future app versions.
