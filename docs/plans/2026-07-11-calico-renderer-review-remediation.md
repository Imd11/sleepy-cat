# Calico Renderer Review Remediation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the verified renderer lifecycle and resource-budget gaps without adding fallback image layers, periodic window rebuilds, or changing prompt-picker behavior.

**Architecture:** Keep one native prompt-button window and one visible canvas. A successful canvas commit rearms fatal reporting; a fatal commit hides the native window, performs a bounded same-renderer recovery, and shows it only after a successful redraw. WebContent termination reloads the same window from a stored creation URL with explicit errors and bounded retries. Baseline and motion surfaces share one hard two-surface budget.

**Tech Stack:** Tauri 2 / Rust, vanilla JavaScript Canvas 2D, Vitest, real WKWebView/WebView2 probe, GitHub Actions.

---

## Constraints

- Follow `karpathy-guidelines`: changes must map directly to the four review findings.
- Use TDD for each behavioral fix.
- Keep one visible `canvas`; never add a default image fallback layer.
- Never close/rebuild a healthy same-label prompt-button window.
- Never add an age timer, heartbeat reload, or periodic renderer reset.
- Preserve pet interactions, motion triggers, prompt popover, autosend, settings, and import/export behavior.
- The two-hour plateau and eight-hour user soak remain release acceptance gates. Automated stress tests may strengthen evidence but must not be described as substitutes for elapsed-time acceptance.

### Task 1: Rearm Fatal Reporting After a Successful Redraw

**Files:**
- Modify: `src/overlay/calicoFrameRenderer.test.ts`
- Modify: `public/calico/frame-renderer.js`

**Step 1: Write a failing regression test**

Add a test that makes visible-canvas commits follow this sequence:

```text
success -> fatal -> recovery success -> second fatal
```

Assert that:

- the first and second failures both call `onFatalRender` exactly once;
- repeated failures before a successful commit remain coalesced;
- the successful recovery returns diagnostics to `state: "ready"` and `visualReady: true`.

**Step 2: Run the focused test and confirm RED**

```bash
npm test -- src/overlay/calicoFrameRenderer.test.ts
```

Expected: FAIL because `fatalReported` remains latched after recovery.

**Step 3: Implement the minimum fix**

In `commitScratch`, set `fatalReported = false` only after `visibleContext.drawImage` succeeds. Do not reset the latch during `suspend`, before a draw, or after a failed recovery.

**Step 4: Run the focused test and confirm GREEN**

```bash
npm test -- src/overlay/calicoFrameRenderer.test.ts
```

Expected: PASS.

**Step 5: Commit**

```bash
git add public/calico/frame-renderer.js src/overlay/calicoFrameRenderer.test.ts
git commit -m "fix: rearm calico fatal reporting after recovery"
```

### Task 2: Recover a Fatal Renderer Failure Without External User Action

**Files:**
- Modify: `src/overlay/overlayHtml.test.ts`
- Modify: `public/overlay.html`

**Step 1: Write failing lifecycle-contract tests**

Assert the overlay contains one single-flight fatal recovery operation that:

- stops the idle director and suspends the current renderer while retaining the last frame;
- reports `ready=false` and waits for the native hide transition to be accepted/applied;
- retries same-renderer `resumeAndReportReady()` with delays `0`, `100`, and `400` ms;
- never increments the lifecycle generation again while the same fatal recovery is in flight;
- logs a terminal recovery failure instead of silently waiting for `contextrestored`, `pageshow`, or a tray action.

**Step 2: Run the focused test and confirm RED**

```bash
npm test -- src/overlay/overlayHtml.test.ts
```

Expected: FAIL because `reportFatalRendererError` currently only hides the native window.

**Step 3: Implement bounded same-renderer recovery**

Add `fatalRecoveryPromise` and `recoverFatalRenderer(generation)`. `reportFatalRendererError` must return the existing promise when recovery is already active; otherwise it advances `lifecycleGeneration` once, stops idle work, suspends the renderer, and launches the bounded recovery. Each attempt must verify the generation, use the existing renderer/motion runtime, and show the native window only through `reportRendererReady(true)` after a successful redraw.

Do not create a new renderer, image element, WebView, or native window.

**Step 4: Run the focused lifecycle and renderer tests**

```bash
npm test -- src/overlay/overlayHtml.test.ts src/overlay/calicoFrameRenderer.test.ts
```

Expected: PASS.

**Step 5: Commit**

```bash
git add public/overlay.html src/overlay/overlayHtml.test.ts
git commit -m "fix: recover fatal calico draws in place"
```

### Task 3: Enforce a Two-Surface Total Budget Including Baseline

**Files:**
- Modify: `src/overlay/calicoFrameRenderer.test.ts`
- Modify: `public/calico/frame-renderer.js`

**Step 1: Write failing baseline-first budget tests**

Exercise the production sequence:

```text
showBaseline -> play A -> play B -> showBaseline -> play C
```

Track every allocated and released surface, including the baseline. Assert at every await boundary:

- total live surfaces are at most 2;
- the active source remains owned until the replacement candidate commits;
- baseline is released after a motion commits;
- returning to baseline evicts inactive motion caches before loading and keeps at most one decoded motion afterward;
- `dispose()` releases all remaining surfaces exactly once.

**Step 2: Run the focused test and confirm RED**

```bash
npm test -- src/overlay/calicoFrameRenderer.test.ts
```

Expected: FAIL with `liveSurfaceCount === 3` in the baseline-plus-two-motion sequence.

**Step 3: Implement one shared capacity calculation**

Replace the decoded-only incoming eviction rule with a reservation helper that counts `decoded.size + baselineSurface`. Before loading any new surface, evict oldest non-active cached motion surfaces until one slot is reserved. After a motion commits, release the no-longer-active baseline. After a baseline commits, trim inactive decoded motion surfaces so the shared limit remains satisfied.

Do not release the currently active source before the replacement frame has committed.

**Step 4: Run renderer tests**

```bash
npm test -- src/overlay/calicoFrameRenderer.test.ts
```

Expected: PASS.

**Step 5: Commit**

```bash
git add public/calico/frame-renderer.js src/overlay/calicoFrameRenderer.test.ts
git commit -m "fix: share calico surface budget with baseline"
```

### Task 4: Make WebContent Termination Recovery Observable and Retryable

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/windows.rs`

**Step 1: Add failing Rust tests**

Using Tauri's mock runtime, build a `Prompt Button` WebView and assert one recovery attempt:

- hides the existing window;
- preserves the same window label/identity;
- navigates to `overlay.html` with a newly allocated `rendererInstanceId`;
- uses a stored creation URL when reading the terminated WebView URL fails;
- returns an error for missing window, hide failure, missing URL, or navigation failure instead of swallowing it.

Add a pure retry-policy test for the bounded delays `0`, `100`, and `400` ms and no fourth attempt.

**Step 2: Run focused Rust tests and confirm RED**

```bash
cd src-tauri
cargo test prompt_button_renderer_tests -- --nocapture
```

Expected: FAIL because the current closure discards hide, URL, navigation, and scheduling errors.

**Step 3: Store the exact creation URL**

Add a small managed `PromptButtonRecoveryUrlState`. When `build_prompt_button_window` succeeds, save the exact `window.url()` so production and development schemes are preserved. Update only the query's `rendererInstanceId` during recovery.

**Step 4: Implement explicit recovery outcomes and bounded retry**

Extract one testable recovery attempt returning `Result<(), String>`. The main-thread closure must log the exact failed step and schedule at most the remaining bounded attempts. `run_on_main_thread` scheduling failure must also be logged. Stale instance checks must cancel retries.

Keep the existing native window; do not close and rebuild it.

**Step 5: Run focused and full Rust tests**

```bash
cd src-tauri
cargo test prompt_button_renderer_tests -- --nocapture
cargo test
```

Expected: PASS.

**Step 6: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/windows.rs
git commit -m "fix: make calico webcontent recovery observable"
```

### Task 5: Extend the Real WebView Probe to the Production Baseline Sequence

**Files:**
- Modify: `tests/fixtures/calico-runtime-surface-probe.html`
- Modify: `scripts/run-calico-webview-probe.mjs`

**Step 1: Make the probe reproduce the missed sequence**

Start the production renderer with `showBaseline()`, then stress rapid A/B/C motion requests, return to baseline, and stress motions again. Sample diagnostics after every baseline and motion boundary. Record whether two distinct fatal cycles, each separated by a successful redraw, both invoke the fatal callback.

**Step 2: Tighten probe validation**

Require:

- `maxima.liveSurfaceCount <= 2` including baseline;
- `maxima.decodedSheetCount <= 2`;
- all pending/queued/timer maxima <= 1;
- both separated fatal cycles are reported;
- recovery redraw returns `state: ready` and `visualReady: true`;
- disposal returns every count to zero.

**Step 3: Run the macOS real-WebView probe**

```bash
npm run test:calico-webview
```

Expected: PASS and artifact JSON includes baseline-first and repeated-fatal evidence.

**Step 4: Commit**

```bash
git add tests/fixtures/calico-runtime-surface-probe.html scripts/run-calico-webview-probe.mjs
git commit -m "test: cover baseline and repeated calico failures"
```

### Task 6: Full Verification and Push

**Files:**
- No source changes expected.

Use `superpowers:verification-before-completion` before any completion claim.

**Step 1: Run all frontend tests and production build**

```bash
npm test
npm run build
```

Expected: PASS.

**Step 2: Run all Rust tests and release compile**

```bash
cd src-tauri
cargo test
cargo build --release
cd ..
```

Expected: PASS.

**Step 3: Run real WebView validation**

```bash
npm run test:calico-webview
```

Expected: PASS with total live surfaces <= 2 and repeated-fatal recovery evidence.

**Step 4: Verify scope and repository cleanliness**

```bash
git diff --check 4cad88ded6318e6ce70517f03a42b99967774a4d...HEAD
git diff --name-only 4cad88ded6318e6ce70517f03a42b99967774a4d...HEAD
git status --short
```

Expected: only this plan and the renderer/lifecycle/probe files listed above have changed; no prompt data, autosend, settings UI, import/export, or release metadata changed.

**Step 5: Push the reviewed commits**

```bash
git push origin HEAD:fix/calico-single-window-renderer
git push origin HEAD:main
```

Expected: both remote refs point to the verified SHA.

**Step 6: Record remaining elapsed-time acceptance honestly**

Do not claim the original long-running disappearance is empirically closed until the native two-hour plateau and eight-hour user soak from `docs/plans/2026-07-10-calico-single-window-bounded-renderer.md` pass. Report the code-level and automated-WebView status separately from those manual elapsed-time gates.

---

## Acceptance Criteria

- Every fatal visible-canvas failure hides the native window and starts at most one bounded same-renderer recovery.
- A successful redraw rearms fatal reporting, so a later independent failure follows the same lifecycle.
- No recovery requires a user click, tray toggle, `pageshow`, or `contextrestored` event.
- WebContent termination errors are visible and bounded retries use the stored exact creation URL.
- The existing prompt-button window is navigated in place; no duplicate native window is created.
- Baseline plus all decoded motion surfaces never exceed two live surfaces in unit tests and a real Tauri WebView.
- One visible canvas remains the only pet image layer.
- Full frontend tests, Rust tests, production build, release compile, and macOS WebView probe pass.
- Two-hour and eight-hour native acceptance remain explicitly pending until actually run.
