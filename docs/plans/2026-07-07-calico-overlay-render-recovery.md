# Calico Overlay Render Recovery Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Prevent the floating Calico pet from disappearing during long-running app sessions while preserving the existing prompt workflow and animation semantics.

**Architecture:** Keep the existing APNG action library and idle director, but change APNG playback from a long-lived single-image resource model to an action-scoped resource model that cleans up after each motion. Add a native Tauri watchdog that can rebuild the `prompt-button` WebView/window when the overlay renderer becomes stale or has safely aged out.

**Tech Stack:** Tauri 2, Rust, macOS WKWebView, vanilla browser modules under `public/calico`, React/Vite/Vitest test harness.

---

## Root Cause

The app backend and native transparent `prompt-button` window can remain alive while the WebKit rendering layer inside that window stops painting. The user-visible symptom is: Calico disappears, but the transparent overlay area still blocks clicks underneath.

The strongest evidence points to APNG decoder/resource accumulation in the overlay WebContent process:

- Calico actions are APNG files switched by `public/calico/motion-runtime.js`.
- `public/calico/idle-director.js` keeps selecting APNG states during long idle sessions.
- WebKit can keep native image decoder/GPU resources even after JS replaces `img.src`.
- Once the renderer is overloaded or stale, the window remains present but the sprite becomes visually blank.

This plan fixes the resource model first and adds native recovery as a second line of defense.

---

## UX Contract

The user should experience Calico as a stable entry point, not as something that needs manual restart.

```text
Normal
  Calico stays visible
  User clicks Calico
  Prompt panel opens
  User selects prompt
  Prompt is filled/sent
  Calico shows short success/error motion

Long idle
  Calico may play lightweight/random motions
  Finished APNG actions are cleaned up
  If the renderer gets stale anyway, the native app rebuilds the overlay quietly
```

Do not change:

- Prompt/category data behavior.
- Prompt selection, paste, or autosend behavior.
- Existing Calico action names and trigger semantics.
- Existing APNG/SVG asset files.

---

## Non-Goals

- Do not preload all APNGs as persistent hidden DOM nodes. That risks breaking replay-from-frame-zero semantics and can make many APNGs animate in the background.
- Do not rely only on `location.reload()`. If WebKit is already stuck, JS may not get a chance to run.
- Do not disable system sleep with an aggressive `NSActivityIdleSystemSleepDisabled` assertion.
- Do not refactor unrelated prompt manager, settings, import/export, or autosend code.
- Do not touch release packaging/versioning in this plan.

---

### Task 0: Confirm Boundaries and Current Dirty State

**Files:**
- Inspect: `public/calico/motion-runtime.js`
- Inspect: `public/overlay.html`
- Inspect: `public/calico/idle-director.js`
- Inspect: `src-tauri/src/windows.rs`
- Inspect: `src-tauri/src/lib.rs`
- Inspect: `src/overlay/calicoMotionRuntime.test.ts`
- Inspect: `src/overlay/overlayHtml.test.ts`

**Step 1: Check the worktree**

Run:

```bash
git status --short
```

Expected:
- Note any existing unrelated build artifacts such as `dist/`, `src-tauri/target/`, `release/`, or generated schemas.
- Do not revert or stage unrelated files.

**Step 2: Confirm the Calico render path**

Run:

```bash
rg -n "createCalicoMotionRuntime|createCalicoIdleDirector|calicoSprite|show_prompt_button|BUTTON_WINDOW_LABEL" public src src-tauri/src
```

Expected:
- Sprite rendering is centered in `public/overlay.html` and `public/calico/motion-runtime.js`.
- Native overlay window creation/reuse is centered in `src-tauri/src/windows.rs`.
- Main React prompt workflow remains separate.

**Step 3: Commit nothing**

This is a boundary check only.

---

### Task 1: Add Tests for Action-Scoped APNG Cleanup

**Files:**
- Modify: `src/overlay/calicoMotionRuntime.test.ts`
- Later modify: `public/calico/motion-runtime.js`

**Step 1: Add helper functions for active sprite lookup**

Add these helpers near the existing `elements()` helper:

```ts
function activeActionImage(host: HTMLElement): HTMLImageElement | null {
  return host.querySelector<HTMLImageElement>(".calico-action-sprite");
}

function activeSpriteSrc(image: HTMLImageElement, host: HTMLElement): string {
  return activeActionImage(host)?.getAttribute("src") ?? image.getAttribute("src") ?? "";
}
```

**Step 2: Add a failing cleanup test**

Add this test inside `describe("Calico motion runtime", () => { ... })`:

```ts
  it("removes transient APNG action images after auto-returning to default", async () => {
    vi.useFakeTimers();
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { image, host } = elements();
    host.appendChild(image);
    const runtime = createCalicoMotionRuntime({
      image,
      host,
      manifest,
      now: () => Date.now(),
    });

    runtime.apply({ state: "happy", durationMs: 100 });

    expect(activeActionImage(host)?.getAttribute("src")).toContain("/calico/calico-happy.apng");

    vi.advanceTimersByTime(100);

    expect(host.dataset.motionState).toBe("idle-follow");
    expect(image.getAttribute("src")).toBe("/calico/calico-idle-follow.svg");
    expect(activeActionImage(host)).toBeNull();
    vi.useRealTimers();
  });
```

Expected before implementation: FAIL, because the current runtime only changes the single image `src` and has no transient action image to remove.

**Step 3: Add a drag-state guard test**

Add:

```ts
  it("keeps durationless APNG actions visible until an explicit reset", async () => {
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { image, host } = elements();
    host.appendChild(image);
    const dragManifest = {
      ...manifest,
      states: {
        ...manifest.states,
        "react-drag": {
          file: "/calico/calico-react-drag.apng",
          priority: 100,
          durationMs: 0,
          minMs: 0,
          replay: false,
          scale: 1.1,
          offsetX: 0,
          offsetY: 6,
        },
      },
    };
    const runtime = createCalicoMotionRuntime({ image, host, manifest: dragManifest });

    runtime.apply({ state: "react-drag", reason: "drag" });

    expect(activeSpriteSrc(image, host)).toBe("/calico/calico-react-drag.apng");

    runtime.reset();

    expect(host.dataset.motionState).toBe("idle-follow");
    expect(activeActionImage(host)).toBeNull();
  });
```

This preserves drag behavior: a durationless action must not be cleaned up prematurely.

**Step 4: Run focused tests and confirm failure**

Run:

```bash
npm test -- src/overlay/calicoMotionRuntime.test.ts
```

Expected: FAIL on the new cleanup assertions.

---

### Task 2: Implement Transient APNG Action Layers

**Files:**
- Modify: `public/calico/motion-runtime.js`
- Modify: `src/overlay/calicoMotionRuntime.test.ts`

**Step 1: Preserve the base SVG image as the stable layer**

In `createCalicoMotionRuntime`, treat the passed `image` as the stable baseline image. The baseline image should normally point at `manifest.defaultState`.

Add constants:

```js
const REPLAY_SLOT_COUNT = 2;
const ACTION_SPRITE_CLASS = "calico-action-sprite";
```

**Step 2: Add action image lifecycle helpers**

Add local state and helpers inside `createCalicoMotionRuntime`:

```js
let actionImage = null;

function releaseActionImage() {
  if (!actionImage) return;
  actionImage.removeAttribute("src");
  actionImage.remove();
  actionImage = null;
  image.hidden = false;
}

function handleActionImageError() {
  releaseActionImage();
  const entry = defaultEntry();
  if (!entry?.file) return;
  host.dataset.motionState = manifest.defaultState;
  image.setAttribute("src", entry.file);
  applyRenderMetadataTo(image, entry);
}

function createActionImage() {
  releaseActionImage();
  actionImage = document.createElement("img");
  actionImage.className = `${image.className} ${ACTION_SPRITE_CLASS}`.trim();
  actionImage.alt = "";
  actionImage.draggable = false;
  actionImage.setAttribute("aria-hidden", "true");
  actionImage.addEventListener("error", handleActionImageError, { once: true });
  actionImage.addEventListener("load", () => {
    if (actionImage?.isConnected) image.hidden = true;
  }, { once: true });
  host.appendChild(actionImage);
  return actionImage;
}

function applyRenderMetadataTo(target, entry) {
  target.style.setProperty("--calico-scale", String(entry.scale ?? 1));
  target.style.setProperty("--calico-offset-x", `${entry.offsetX ?? 0}px`);
  target.style.setProperty("--calico-offset-y", `${entry.offsetY ?? 0}px`);
}
```

Do not keep removed action images in arrays. There must be at most one transient APNG image at a time. When a new APNG action interrupts an old APNG action, release the old action image first, then create a fresh action image. This is intentional: reusing the same transient image would keep the old WebKit decoder path alive longer than necessary.

**Step 3: Route default state to baseline and APNG states to transient action image**

Replace `setImageSource(entry)` with a state-aware function:

```js
function setImageSource(state, entry) {
  if (!entry?.file) return;
  if (state === manifest.defaultState) {
    releaseActionImage();
    image.setAttribute("src", entry.file);
    applyRenderMetadataTo(image, entry);
    return;
  }

  const target = createActionImage();
  target.setAttribute("src", entry.replay ? replaySourceFor(entry) : entry.file);
  applyRenderMetadataTo(target, entry);
}
```

Update `apply()` to call:

```js
setImageSource(state, entry);
```

Update the default reset path so it removes the action image and shows the baseline SVG.

Do not hide the baseline SVG before the APNG action image has loaded. The `load` listener above hides the baseline only after the transient action image is ready. If the APNG fails, `handleActionImageError()` releases the transient image and returns to `idle-follow.svg`, avoiding a blank transparent button.

**Step 4: Update existing tests to use active sprite helpers**

For assertions that currently read `image.getAttribute("src")` while a non-default action is active, replace with:

```ts
expect(activeSpriteSrc(image, host)).toBe("...");
```

Keep reset/default assertions against the baseline `image`.

**Step 5: Run focused tests**

Run:

```bash
npm test -- src/overlay/calicoMotionRuntime.test.ts
```

Expected: PASS.

**Step 6: Commit**

```bash
git add public/calico/motion-runtime.js src/overlay/calicoMotionRuntime.test.ts
git commit -m "fix: clean up transient calico action sprites"
```

---

### Task 3: Update Overlay CSS and HTML Expectations

**Files:**
- Modify: `public/overlay.html`
- Modify: `src/overlay/overlayHtml.test.ts`

**Step 1: Ensure overlaid sprite layers align exactly**

In `public/overlay.html`, update sprite CSS so the baseline image and transient action image share the same centered geometry:

```css
.calico-sprite {
  position: absolute;
  inset: 50% auto auto 50%;
  width: var(--calico-sprite-size);
  height: var(--calico-sprite-size);
  object-fit: contain;
  pointer-events: none;
  transform-origin: 45% 76%;
  transform:
    translate(
      calc(-50% + var(--calico-offset-x)),
      calc(-50% + var(--calico-offset-y) + var(--calico-hover-y) + var(--calico-press-y))
    )
    rotate(var(--calico-rotate))
    scale(var(--calico-scale));
  transition: transform 140ms ease;
}

.calico-sprite[hidden] {
  display: none;
}
```

Keep the existing hover/press/drag selectors. They should continue to affect both baseline and transient `.calico-sprite` layers.

Also update the `@keyframes calico-idle-breath` transform to use the same centered coordinate system. Do not leave the old non-centered `translate(var(--calico-offset-x), ...)` keyframes in place, because keyframe transforms override `.calico-sprite` transforms while the idle breathing animation is active.

Use:

```css
@keyframes calico-idle-breath {
  from {
    transform:
      translate(
        calc(-50% + var(--calico-offset-x)),
        calc(-50% + var(--calico-offset-y) + var(--calico-hover-y) + var(--calico-press-y))
      )
      rotate(var(--calico-rotate))
      scale(var(--calico-scale));
  }
  to {
    transform:
      translate(
        calc(-50% + var(--calico-offset-x)),
        calc(-50% + var(--calico-offset-y) + var(--calico-hover-y) + var(--calico-press-y) - 1px)
      )
      rotate(var(--calico-rotate))
      scale(var(--calico-scale));
  }
}
```

**Step 2: Keep the button hit area stable**

Do not change:

```css
:root {
  --calico-hit-area-size: 132px;
  --calico-sprite-size: 126px;
}

.calico-entry {
  width: var(--calico-hit-area-size);
  height: var(--calico-hit-area-size);
}
```

**Step 3: Add overlay HTML expectations**

In `src/overlay/overlayHtml.test.ts`, update or add an assertion that the overlay still has one baseline sprite:

```ts
expect(html).toContain('id="calicoSprite"');
expect(html).toContain("calico-sprite");
```

Add an assertion that hidden sprites are explicitly handled:

```ts
expect(html).toContain(".calico-sprite[hidden]");
expect(html).toContain("calc(-50% + var(--calico-offset-x))");
```

**Step 4: Run overlay HTML tests**

Run:

```bash
npm test -- src/overlay/overlayHtml.test.ts
```

Expected: PASS.

**Step 5: Commit**

```bash
git add public/overlay.html src/overlay/overlayHtml.test.ts
git commit -m "fix: keep calico sprite layers aligned"
```

---

### Task 4: Add Overlay Health Heartbeats

**Files:**
- Modify: `public/overlay.html`
- Modify: `src/overlay/overlayHtml.test.ts`

**Step 1: Add heartbeat state calculation**

In `public/overlay.html`, add:

```js
const OVERLAY_HEARTBEAT_MS = 10_000;
let overlayStartedAt = Date.now();
let lastUserInteractionAt = Date.now();

function markOverlayInteraction() {
  lastUserInteractionAt = Date.now();
}

function isSafeToRebuildPromptButtonNow() {
  const motionState = btn.dataset.motionState || "idle-follow";
  const quietForMs = Date.now() - lastUserInteractionAt;
  return quietForMs > 15_000
    && !start
    && !dragging
    && !contextMenuOpened
    && !statusBubble?.classList.contains("is-visible")
    && (motionState === "idle-follow" || motionState === "idle");
}
```

Call `markOverlayInteraction()` from pointerdown, pointermove when dragging starts, pointerup, contextmenu, and status bubble display.

`isSafeToRebuildPromptButtonNow()` is only a safety signal. It must not mean "rebuild now". The native side should rebuild only when there is a separate trigger such as stale heartbeat or excessive overlay age.

**Step 2: Emit health heartbeat**

Add:

```js
function emitOverlayHeartbeat() {
  emit("prompt-button-health", {
    startedAt: overlayStartedAt,
    updatedAt: Date.now(),
    motionState: btn.dataset.motionState || "idle-follow",
    safeToRebuild: isSafeToRebuildPromptButtonNow(),
  }).catch(() => {});
}

function startOverlayHealthHeartbeat() {
  emitOverlayHeartbeat();
  window.setInterval(emitOverlayHeartbeat, OVERLAY_HEARTBEAT_MS);
}
```

Call `startOverlayHealthHeartbeat();` after existing listeners are installed.

**Step 3: Add text-level tests**

In `src/overlay/overlayHtml.test.ts`, add:

```ts
  it("emits prompt button health heartbeats for native recovery", () => {
    const html = readOverlayHtml();

    expect(html).toContain("prompt-button-health");
    expect(html).toContain("OVERLAY_HEARTBEAT_MS");
    expect(html).toContain("isSafeToRebuildPromptButtonNow");
    expect(html).toContain("safeToRebuild");
    expect(html).toContain("startOverlayHealthHeartbeat();");
  });
```

**Step 4: Run tests**

```bash
npm test -- src/overlay/overlayHtml.test.ts
```

Expected: PASS.

**Step 5: Commit**

```bash
git add public/overlay.html src/overlay/overlayHtml.test.ts
git commit -m "feat: report calico overlay health"
```

---

### Task 5: Add Native Prompt Button Rebuild Logic

**Files:**
- Modify: `src-tauri/src/windows.rs`

**Step 1: Extract prompt button builder**

Refactor the builder branch in `show_prompt_button` into:

```rust
fn build_prompt_button_window(
    app: &tauri::AppHandle,
    x: f64,
    y: f64,
) -> Result<tauri::WebviewWindow, String> {
    let monitor = app.primary_monitor().map_err(|e| e.to_string())?;
    let (x, y) = clamp_button_position_for_monitor(x, y, monitor.as_ref());
    let (window_x, window_y) = prompt_button_visual_to_window_position(x, y);
    let window = WebviewWindowBuilder::new(
        app,
        BUTTON_WINDOW_LABEL,
        WebviewUrl::App("overlay.html".into()),
    )
    .title("Prompt Button")
    .inner_size(BUTTON_WINDOW_WIDTH, BUTTON_WINDOW_HEIGHT)
    .resizable(false)
    .decorations(false)
    .always_on_top(true)
    .accept_first_mouse(true)
    .skip_taskbar(true)
    .position(window_x, window_y)
    .build()
    .map_err(|e| e.to_string())?;

    if BUTTON_WINDOW_TRANSPARENT {
        crate::macos_panels::configure_transparent_webview_window(&window)?;
    }
    crate::macos_panels::configure_non_activating_panel(&window)?;
    Ok(window)
}
```

Then make `show_prompt_button` call this helper when no window exists.

**Step 2: Add a rebuild helper**

Add:

```rust
pub fn rebuild_prompt_button_window(app: &tauri::AppHandle) -> Result<(), String> {
    let position = prompt_button_position_cmd(app.clone())?
        .map(|point| (point.x, point.y))
        .unwrap_or((960.0, 700.0));

    if let Some(window) = app.get_webview_window(BUTTON_WINDOW_LABEL) {
        let _ = window.close();
    }

    build_prompt_button_window(app, position.0, position.1)?;
    Ok(())
}
```

Use the current visual position when possible so the user does not see the pet jump.

**Step 3: Add source-level regression tests**

In the existing `#[cfg(test)]` section for `windows.rs`, add a test that checks the command source contains a real close-and-build path:

```rust
#[test]
fn prompt_button_rebuild_closes_existing_window_and_rebuilds_at_same_position() {
    let source = include_str!("windows.rs");

    assert!(source.contains("pub fn rebuild_prompt_button_window"));
    assert!(source.contains("prompt_button_position_cmd(app.clone())"));
    assert!(source.contains("window.close()"));
    assert!(source.contains("build_prompt_button_window(app"));
}
```

This follows the existing source-contains test style in this file.

**Step 4: Run Rust tests**

```bash
cd src-tauri && cargo test
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/windows.rs
git commit -m "feat: rebuild calico prompt button window"
```

---

### Task 6: Add Native Overlay Watchdog State

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/windows.rs`

**Step 1: Add health payload and state**

In `src-tauri/src/lib.rs`, add:

```rust
#[derive(Clone, Copy)]
struct PromptButtonHealthSnapshot {
    first_seen: Option<std::time::Instant>,
    last_seen: Option<std::time::Instant>,
    safe_to_rebuild: bool,
}

impl Default for PromptButtonHealthSnapshot {
    fn default() -> Self {
        Self {
            first_seen: None,
            last_seen: None,
            safe_to_rebuild: false,
        }
    }
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptButtonHealthPayload {
    started_at: Option<u64>,
    updated_at: Option<u64>,
    motion_state: Option<String>,
    safe_to_rebuild: Option<bool>,
}

#[derive(Clone, Default)]
struct PromptButtonHealthState {
    snapshot: std::sync::Arc<std::sync::Mutex<PromptButtonHealthSnapshot>>,
}
```

Manage it in the builder:

```rust
.manage(PromptButtonHealthState::default())
```

**Step 2: Listen for heartbeats in setup**

Inside `.setup(|app| { ... })`, after `setup_menu_bar_app(app.handle())?`, register an event listener:

```rust
let health_state = app.state::<PromptButtonHealthState>().inner().clone();
app.handle().listen("prompt-button-health", move |event| {
    let Ok(payload) = serde_json::from_str::<PromptButtonHealthPayload>(event.payload()) else {
        return;
    };
    let now = std::time::Instant::now();
    let mut snapshot = health_state
        .snapshot
        .lock()
        .expect("prompt button health lock poisoned");
    if snapshot.first_seen.is_none() {
        snapshot.first_seen = Some(now);
    }
    snapshot.last_seen = Some(now);
    snapshot.safe_to_rebuild = payload.safe_to_rebuild.unwrap_or(false);
});
```

Do not use a raw pointer for managed state. The event listener is long-lived, so the captured state must be owned/cloned safely through `Arc<Mutex<...>>` as shown above.

**Step 3: Add pure rebuild decision function**

Add a pure helper for tests:

```rust
fn should_rebuild_prompt_button(
    now: std::time::Instant,
    last_seen: Option<std::time::Instant>,
    first_seen: Option<std::time::Instant>,
    safe_to_rebuild: bool,
    popover_visible: bool,
) -> bool {
    if popover_visible {
        return false;
    }
    let stale_heartbeat = last_seen
        .map(|seen| now.duration_since(seen) > std::time::Duration::from_secs(45))
        .unwrap_or(false);
    let aged_out = first_seen
        .map(|seen| now.duration_since(seen) > std::time::Duration::from_secs(30 * 60))
        .unwrap_or(false);

    stale_heartbeat || (safe_to_rebuild && aged_out)
}
```

The exact production logic should avoid rebuilding while the popover is visible. `safe_to_rebuild` must not be used as a standalone trigger; it only allows an age-based maintenance rebuild. A stale heartbeat is an actual recovery trigger because it means the overlay renderer may already be unable to report current safety state.

**Step 4: Add tests for the pure function**

Add tests in `src-tauri/src/lib.rs`:

```rust
#[test]
fn prompt_button_watchdog_rebuilds_when_heartbeat_is_stale() {
    let now = std::time::Instant::now();

    assert!(should_rebuild_prompt_button(
        now,
        Some(now - std::time::Duration::from_secs(60)),
        Some(now - std::time::Duration::from_secs(60)),
        false,
        false
    ));
}

#[test]
fn prompt_button_watchdog_does_not_rebuild_while_popover_is_visible() {
    let now = std::time::Instant::now();

    assert!(!should_rebuild_prompt_button(
        now,
        Some(now - std::time::Duration::from_secs(60)),
        Some(now - std::time::Duration::from_secs(60)),
        true,
        true
    ));
}
```

Add one more regression test to ensure an idle safety signal alone does not cause rebuild churn:

```rust
#[test]
fn prompt_button_watchdog_does_not_rebuild_only_because_overlay_is_safe() {
    let now = std::time::Instant::now();

    assert!(!should_rebuild_prompt_button(
        now,
        Some(now - std::time::Duration::from_secs(10)),
        Some(now - std::time::Duration::from_secs(10)),
        true,
        false
    ));
}
```

**Step 5: Run Rust tests**

```bash
cd src-tauri && cargo test
```

Expected: PASS.

**Step 6: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: track calico overlay health"
```

---

### Task 7: Run the Native Watchdog Loop

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/windows.rs`

**Step 1: Start a conservative background watchdog**

In setup, after the initial `show_prompt_button`, spawn a background thread:

```rust
let watchdog_app = app.handle().clone();
let watchdog_health = app.state::<PromptButtonHealthState>().inner().clone();
std::thread::spawn(move || loop {
    std::thread::sleep(std::time::Duration::from_secs(15));

    let Some(button) = watchdog_app.get_webview_window(crate::windows::BUTTON_WINDOW_LABEL) else {
        continue;
    };
    if !button.is_visible().unwrap_or(false) {
        continue;
    }
    let popover_visible = watchdog_app
        .get_webview_window(crate::windows::POPOVER_WINDOW_LABEL)
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);

    let snapshot = *watchdog_health
        .snapshot
        .lock()
        .expect("prompt button health lock poisoned");

    if should_rebuild_prompt_button(
        std::time::Instant::now(),
        snapshot.last_seen,
        snapshot.first_seen,
        snapshot.safe_to_rebuild,
        popover_visible,
    ) {
        let rebuild_app = watchdog_app.clone();
        let rebuild_health = watchdog_health.clone();
        let _ = watchdog_app.run_on_main_thread(move || {
            if crate::windows::rebuild_prompt_button_window(&rebuild_app).is_ok() {
                *rebuild_health
                    .snapshot
                    .lock()
                    .expect("prompt button health lock poisoned") =
                    PromptButtonHealthSnapshot::default();
            }
        });
    }
});
```

The watchdog thread must not directly close or build WebView windows. It may inspect lightweight window state, but the rebuild call must be scheduled with `AppHandle::run_on_main_thread`.

**Step 2: Reset health state after rebuild**

After a successful rebuild, clear the full `PromptButtonHealthSnapshot` so the new WebView gets time to start and send its first heartbeat.

**Step 3: Add source-level regression checks**

Add a test in `src-tauri/src/lib.rs`:

```rust
#[test]
fn prompt_button_watchdog_rebuilds_on_main_thread() {
    let source = include_str!("lib.rs");

    assert!(source.contains("run_on_main_thread"));
    assert!(source.contains("rebuild_prompt_button_window"));
    assert!(source.contains("PromptButtonHealthSnapshot::default()"));
}
```

**Step 4: Run Rust tests**

```bash
cd src-tauri && cargo test
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/windows.rs
git commit -m "fix: recover stale calico overlay renderer"
```

---

### Task 8: Final Verification

**Files:**
- No code changes unless tests expose a real issue.

**Step 1: Run focused frontend tests**

```bash
npm test -- src/overlay/calicoMotionRuntime.test.ts src/overlay/overlayHtml.test.ts
```

Expected: PASS.

**Step 2: Run all frontend tests**

```bash
npm test
```

Expected: PASS.

**Step 3: Run Rust tests**

```bash
cd src-tauri && cargo test
```

Expected: PASS.

**Step 4: Run production build**

```bash
npm run build
```

Expected: PASS.

**Step 5: Review changed files only**

```bash
git diff --stat HEAD~4..HEAD
git diff --check HEAD~4..HEAD
```

Expected:
- No whitespace errors.
- Changes limited to Calico overlay runtime, overlay HTML/tests, and native prompt-button recovery.

**Step 6: Verify specific safety constraints are encoded**

Run:

```bash
rg -n "safeToRebuild|safe_to_rebuild|run_on_main_thread|PromptButtonHealthSnapshot::default|calico-action-sprite|handleActionImageError|calc\\(-50% \\+ var\\(--calico-offset-x\\)" public src src-tauri/src
```

Expected:
- `safeToRebuild` appears in `public/overlay.html`.
- `safe_to_rebuild` appears in `src-tauri/src/lib.rs`.
- `run_on_main_thread` appears in the watchdog rebuild path.
- `handleActionImageError` appears in `public/calico/motion-runtime.js`.
- Centered transform math appears in both `.calico-sprite` and `@keyframes calico-idle-breath`.

**Step 7: Do not package unless explicitly requested**

This plan fixes source behavior. Packaging/signing/release upload is not part of this plan.

---

## Acceptance Criteria

- Finished APNG actions are removed from the DOM and have their `src` cleared.
- The baseline Calico SVG remains the stable always-visible fallback.
- A new APNG action releases the previous transient APNG node before creating the next one.
- APNG action load failure returns Calico to `idle-follow.svg` instead of leaving a blank overlay.
- Centered sprite CSS and `@keyframes calico-idle-breath` use the same transform coordinate system.
- Drag APNG remains visible during drag and is cleaned up on reset.
- Existing Calico motion names and trigger conditions remain intact.
- The native app can rebuild a stale `prompt-button` WebView/window without moving it.
- The native watchdog does not rebuild while the prompt popover is visible.
- `safeToRebuild` is only a safety signal; it does not cause rebuilds by itself.
- Native close/build recovery is scheduled through `AppHandle::run_on_main_thread`.
- Heartbeat payload casing is compatible between JS camelCase and Rust `serde`.
- No prompt selection, prompt storage, category, import/export, or autosend behavior is changed.

---

## User-Visible Result

After implementation, Calico should feel the same in normal use, but more stable:

```text
Calico visible on desktop
  Idle SVG is the stable base
  Short APNG actions play when appropriate
  Finished actions are cleaned up immediately
  If WebKit rendering still gets stale, the native app quietly recreates the overlay
```

The user should not need to restart the app to recover a missing pet.
