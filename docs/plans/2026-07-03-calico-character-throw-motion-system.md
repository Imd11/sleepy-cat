# Calico Character Throw Motion System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the current "static Calico plus pasted paper-plane icon" with a complete character motion system where Calico visibly enters a throw-ready pose when the prompt list opens and performs a real throw motion when a prompt is selected.

**Architecture:** Rebuild the floating Calico overlay as a small layered character rig inside `public/overlay.html`: body, head, ears, paws, tail, face, and held projectile are independently transformable. Keep all autosend, target focus, prompt storage, and menu behavior unchanged; only replace the visual motion layer and the paper flight launch point. The motion is controlled by a deterministic overlay state machine so click, drag, right-click, prompt selection, status bubbles, and popover dismissal cannot leave Calico stuck in the wrong pose.

**Tech Stack:** Tauri v2, vanilla HTML/CSS/JS overlay, inline SVG character rig, CSS transforms/keyframes, React prompt popover events, Rust window geometry helpers, Vitest, Cargo tests.

---

## Product Contract

When the user clicks Calico:

```text
Calico normal idle
    ↓
Calico compresses and leans back
    ↓
one paw lifts with the projectile held near the paw, not over the face
    ↓
prompt list appears above Calico
    ↓
Calico remains in a subtle ready/breathing loop while the list is open
```

When the user selects a prompt or prompt group:

```text
prompt list hides
    ↓
Calico deepens the wind-up
    ↓
front paw snaps forward
    ↓
projectile leaves from the paw release point
    ↓
projectile flies in the separate transparent flight window
    ↓
Calico follows through, rebounds, and returns to idle
```

The user should no longer see a normal idle Calico with a paper plane pasted on top of its face. The body pose itself must communicate "ready to throw" and "throwing."

## Motion Requirements

- The overlay window remains `132px` by `132px`; do not enlarge the Calico button window.
- The prompt list still appears above Calico when possible.
- The ready pose remains active while the prompt list is open.
- Calico must remain recognizably the same IP character: cream body, orange calico patches, rounded cute face, brown outline, small friendly proportions, and the same visual temperament as the current asset.
- Idle must not become a dead static icon. If the APNG is replaced by a rig, add subtle idle breathing or tail motion so the character still feels alive.
- The throw animation plays once per selected container:
  - Single prompt: one Calico throw animation.
  - Group prompt: one Calico throw animation for the container selection, not once per item.
- The backend autosend sequence remains exactly the same:
  - copy prompt text
  - activate target app
  - Cmd+V
  - Return
- The animation must never decide success/failure; success/failure still comes from `prompt-autosend-status`.
- The projectile should be a stylized prompt projectile / paper capsule. The physical action should feel like a grenade throw, but the object should still belong to the prompt-product visual language.

## Visual Integrity Guardrails

- The implementation may use an inline SVG rig, but that is a rendering technique, not permission to redesign the character.
- If the first SVG rig pass looks materially less like the current Calico asset, stop and adjust shapes, proportions, colors, and outlines before continuing to motion polish.
- Do not accept a result that is technically animatable but visually worse than the current IP.
- Do not place the held projectile over Calico's eyes, mouth, or cheek. The ready state must read as "paw holding projectile," not "paper icon pasted on face."
- Keep the UI silhouette compact enough for the existing `132px` overlay window. No body part should be clipped in idle, ready, throw, or recover states.

## Files To Touch

- Modify: `public/overlay.html`
- Modify: `public/paper-flight.html`
- Modify: `src/App.tsx`
- Modify: `src/overlay/overlayHtml.test.ts`
- Modify: `src/app/App.test.tsx`
- Modify: `src-tauri/src/windows.rs`

## Files To Avoid Unless A Test Proves They Are Required

- Avoid changing: `src-tauri/src/platform/macos.rs`
- Avoid changing: `src-tauri/src/lib.rs`
- Avoid changing: prompt storage files
- Avoid changing: autosend permissions or Accessibility code
- Avoid changing: app menu/tray logic

---

### Task 1: Lock The Desired Overlay Contract In Tests

**Files:**
- Modify: `src/overlay/overlayHtml.test.ts`
- Test: `src/overlay/overlayHtml.test.ts`

**Step 1: Replace the old static-sprite expectations**

Find the test named:

```ts
it("renders the floating entry as an animated Calico character", () => {
```

Replace its core assertions with:

```ts
expect(html).toContain("calico-rig");
expect(html).toContain("calico-body");
expect(html).toContain("calico-head");
expect(html).toContain("calico-tail");
expect(html).toContain("calico-throw-paw");
expect(html).toContain("calico-projectile");
expect(html).toContain('data-motion-state="idle"');
expect(html).toContain('aria-label="Open Prompt Picker"');
expect(html).not.toContain("<span>Prompts</span>");
```

Keep the existing assertions that verify the floating entry is not the old blue "Prompts" button.

**Step 2: Replace the old paper-plane ready-state test**

Find:

```ts
it("switches Calico into a paper-plane ready state before opening prompts", () => {
```

Replace it with:

```ts
it("switches Calico into a real throw-ready character pose before opening prompts", () => {
  const html = readFileSync("public/overlay.html", "utf8");

  expect(html).toContain("setMotionState('ready'");
  expect(html).toContain('[data-motion-state="ready"]');
  expect(html).toContain("calico-ready-breath");
  expect(html).not.toContain("throwReady: '/calico/calico-idle.apng'");
  expect(html.indexOf("setMotionState('ready'")).toBeLessThan(
    html.indexOf("begin_prompt_pick_session")
  );
});
```

**Step 3: Replace the old throw-send test**

Find:

```ts
it("listens for paper-plane throw events and starts the flight animation", () => {
```

Replace the body with:

```ts
const html = readFileSync("public/overlay.html", "utf8");

expect(html).toContain("prompt-throw-send");
expect(html).toContain("playCalicoThrow");
expect(html).toContain("setMotionState('throwing'");
expect(html).toContain("setMotionState('recovering'");
expect(html).toContain("show_paper_plane_flight_from_button");
expect(html).toContain("THROW_RELEASE_MS");
```

**Step 4: Add a deterministic reset test**

Add:

```ts
it("resets Calico from ready when the popover is dismissed without sending", () => {
  const html = readFileSync("public/overlay.html", "utf8");

  expect(html).toContain("prompt-popover-dismissed");
  expect(html).toContain("resetCalicoMotion");
  expect(html).toContain("motionResetTimer");
});
```

**Step 5: Run this test and verify it fails**

Run:

```bash
npm test -- --run src/overlay/overlayHtml.test.ts
```

Expected: FAIL because `public/overlay.html` does not yet contain the character rig, new motion state API, or popover dismissal handling.

**Step 6: Commit after this task is complete during execution**

```bash
git add src/overlay/overlayHtml.test.ts
git commit -m "test: define calico character throw motion contract"
```

---

### Task 2: Define Non-Send Reset Behavior Without Adding New UI

**Files:**
- Modify: `src/app/App.test.tsx`
- Modify: `src/App.tsx`
- Test: `src/app/App.test.tsx`

**Reason:** Calico must stay in ready pose while the list is open, but it must not stay ready forever after a non-send path. Current `PromptQuickList` has no explicit close button, so do not invent a new close control just to satisfy this animation task. Reset through existing non-send actions and the overlay fallback timeout.

**Step 1: Add failing tests for existing non-send hide actions**

In `src/app/App.test.tsx`, add tests near the button-controls tests. These verify that existing controls which hide the popover emit a reset event; they do not add a new quick-list close button.

```ts
it("emits prompt-popover-dismissed when button controls open the manager without sending", async () => {
  currentWindowLabel = "prompt-popover";
  window.history.pushState({}, "", "/?mode=button-controls");
  const { invoke } = await import("@tauri-apps/api/core");
  vi.mocked(invoke).mockClear();

  const { readTextFile } = await import("@tauri-apps/plugin-fs");
  (readTextFile as ReturnType<typeof vi.fn>).mockImplementation(async (path: string) => {
    if (path.includes("prompts")) return JSON.stringify({ version: 1, prompts: mockPrompts });
    if (path.includes("settings")) {
      return JSON.stringify({
        version: 1,
        blacklistedApps: [],
        overlayPlacement: { buttonOffset: null },
        floatingButton: { visible: true },
      });
    }
    throw new Error("unexpected path: " + path);
  });

  await act(async () => {
    render(<App />);
  });

  fireEvent.click(await screen.findByRole("button", { name: "Manage Prompts..." }));

  await waitFor(() => {
    expect(emitMock).toHaveBeenCalledWith("prompt-popover-dismissed");
  });
});
```

Add a second test for `Hide Calico` because it hides both the button and the popover:

```ts
it("emits prompt-popover-dismissed when hiding Calico from button controls", async () => {
  currentWindowLabel = "prompt-popover";
  window.history.pushState({}, "", "/?mode=button-controls");

  const { readTextFile } = await import("@tauri-apps/plugin-fs");
  (readTextFile as ReturnType<typeof vi.fn>).mockImplementation(async (path: string) => {
    if (path.includes("prompts")) return JSON.stringify({ version: 1, prompts: mockPrompts });
    if (path.includes("settings")) {
      return JSON.stringify({
        version: 1,
        blacklistedApps: [],
        overlayPlacement: { buttonOffset: null },
        floatingButton: { visible: true },
      });
    }
    throw new Error("unexpected path: " + path);
  });

  await act(async () => {
    render(<App />);
  });

  fireEvent.click(await screen.findByRole("button", { name: "Hide Calico" }));

  await waitFor(() => {
    expect(emitMock).toHaveBeenCalledWith("prompt-popover-dismissed");
  });
});
```

**Step 2: Run these tests and verify they fail**

Run:

```bash
npm test -- --run src/app/App.test.tsx
```

Expected: FAIL because the dismissal event is not emitted yet.

**Step 3: Implement dismissal event in `src/App.tsx`**

Add a helper next to the existing event helpers:

```ts
async function emitPromptPopoverDismissed() {
  try {
    await emit("prompt-popover-dismissed");
  } catch (error) {
    console.warn("Failed to emit prompt popover dismissal:", error);
  }
}
```

Call it only in existing non-send paths that hide the popover. Do not call it from the prompt selection path before `prompt-throw-send`; that path must trigger the throw animation instead. Do not add a new quick-list close button as part of this task.

Expected control flows:

```ts
await openMainWindow();
await hidePromptPopover();
await emitPromptPopoverDismissed();
```

```ts
await hidePromptButton();
await hidePromptPopover();
await emitPromptPopoverDismissed();
```

```ts
await openAccessibilitySettings();
await hidePromptPopover();
await emitPromptPopoverDismissed();
```

**Step 4: Keep overlay timeout as the no-selection fallback**

The quick prompt list currently has no explicit close button. For "opened list but selected nothing," the overlay fallback remains:

```js
setMotionState('ready', READY_TIMEOUT_MS);
```

This is acceptable because the popover itself remains open. If a future task adds a real close affordance, that task must emit `prompt-popover-dismissed`.

**Step 5: Run the app test**

Run:

```bash
npm test -- --run src/app/App.test.tsx
```

Expected: PASS for the new dismissal tests; existing autosend tests remain PASS.

**Step 6: Commit after this task is complete during execution**

```bash
git add src/App.tsx src/app/App.test.tsx
git commit -m "test: reset calico on existing non-send popover hides"
```

---

### Task 3: Replace Sprite Switching With A Character Motion State Machine

**Files:**
- Modify: `public/overlay.html`
- Test: `src/overlay/overlayHtml.test.ts`

**Step 1: Replace the old image-only markup**

Replace:

```html
<button id="btn" class="calico-entry" title="Open Prompt Picker" aria-label="Open Prompt Picker">
  <img id="calicoSprite" class="calico-sprite" src="/calico/calico-idle.apng" alt="" draggable="false" />
  <img id="paperPlane" class="calico-plane" src="/calico/paper-plane.svg" alt="" draggable="false" />
</button>
```

With a layered inline SVG rig:

```html
<button
  id="btn"
  class="calico-entry"
  title="Open Prompt Picker"
  aria-label="Open Prompt Picker"
  data-motion-state="idle"
>
  <div class="calico-rig" aria-hidden="true">
    <svg class="calico-svg" viewBox="0 0 132 132" role="img">
      <g class="calico-shadow">
        <ellipse cx="66" cy="103" rx="42" ry="10" />
      </g>
      <g class="calico-tail">
        <path d="M93 79 C116 67 119 91 100 95 C91 97 88 88 93 79Z" />
        <path class="calico-tail-tip" d="M104 79 C116 75 116 91 103 91 C98 91 99 82 104 79Z" />
      </g>
      <g class="calico-body">
        <ellipse cx="61" cy="82" rx="42" ry="26" />
        <path class="calico-body-patch-left" d="M32 70 C39 50 61 49 66 70 C56 76 44 77 32 70Z" />
        <path class="calico-body-patch-right" d="M73 59 C88 54 103 62 102 79 C91 78 80 72 73 59Z" />
      </g>
      <g class="calico-back-paw">
        <ellipse cx="83" cy="101" rx="11" ry="7" />
      </g>
      <g class="calico-front-paw calico-support-paw">
        <ellipse cx="42" cy="99" rx="11" ry="7" />
      </g>
      <g class="calico-head">
        <path class="calico-ear-left" d="M31 48 L40 22 L54 49Z" />
        <path class="calico-ear-right" d="M79 48 L95 22 L101 51Z" />
        <ellipse cx="65" cy="58" rx="38" ry="30" />
        <path class="calico-head-patch-left" d="M34 45 C40 27 58 30 62 51 C51 53 42 51 34 45Z" />
        <path class="calico-head-patch-right" d="M70 48 C77 27 94 27 99 49 C89 54 79 54 70 48Z" />
        <circle class="calico-eye-left" cx="51" cy="61" r="5" />
        <circle class="calico-eye-right" cx="78" cy="61" r="5" />
        <path class="calico-mouth" d="M62 68 Q66 73 70 68" />
        <path class="calico-whisker-left" d="M39 66 L25 63 M39 70 L25 72" />
        <path class="calico-whisker-right" d="M91 66 L106 63 M91 70 L106 72" />
      </g>
      <g class="calico-throw-paw">
        <ellipse cx="82" cy="86" rx="10" ry="15" />
      </g>
      <g class="calico-projectile">
        <path class="calico-projectile-body" d="M0 -9 L16 -2 L0 8 L4 0Z" />
        <path class="calico-projectile-fold" d="M4 0 L16 -2 L5 4Z" />
      </g>
    </svg>
  </div>
</button>
```

This markup is intentionally compact and uses simple shapes. During implementation, keep the visual proportions close to the existing Calico asset: cream base, orange patches, brown outline, cute face.

Before moving to motion CSS, visually compare the static rig against the current Calico asset. The rig must still read as the same character at `132px` size. If it looks like a different cat, adjust the geometry before continuing.

**Step 2: Add the motion state constants**

Replace the old `sprites` object and `setSprite` function with:

```js
const THROW_RELEASE_MS = 170;
const THROW_RECOVER_MS = 760;
const READY_TIMEOUT_MS = 30000;

let motionResetTimer = 0;

function setMotionState(state, resetMs = 0) {
  window.clearTimeout(motionResetTimer);
  btn.dataset.motionState = state;
  if (resetMs > 0) {
    motionResetTimer = window.setTimeout(resetCalicoMotion, resetMs);
  }
}

function resetCalicoMotion() {
  window.clearTimeout(motionResetTimer);
  btn.dataset.motionState = 'idle';
}
```

**Step 3: Update all old `setSprite` calls**

Replace:

```js
setSprite('idle')
```

With:

```js
resetCalicoMotion()
```

Replace:

```js
setSprite('throwReady', 30000);
```

With:

```js
setMotionState('ready', READY_TIMEOUT_MS);
```

Replace:

```js
setSprite('throwSend', 900);
```

With the new throw timeline in Task 5.

For drag:

```js
setMotionState('dragging');
```

For press:

keep the existing `.is-pressing` class; do not create a separate press motion state unless needed.

**Step 4: Run overlay tests**

Run:

```bash
npm test -- --run src/overlay/overlayHtml.test.ts
```

Expected: the rig contract tests begin passing except tests that still require CSS motion classes from later tasks.

**Step 5: Commit after this task is complete during execution**

```bash
git add public/overlay.html src/overlay/overlayHtml.test.ts
git commit -m "feat: replace calico sprite with motion rig"
```

---

### Task 4: Implement The Full Ready Pose In CSS

**Files:**
- Modify: `public/overlay.html`
- Test: `src/overlay/overlayHtml.test.ts`

**Step 1: Remove old `.calico-sprite` and `.calico-plane` CSS**

Delete these old selectors:

```css
.calico-sprite
.calico-entry:hover .calico-sprite
.calico-entry.is-pressing .calico-sprite
.calico-entry.is-dragging .calico-sprite
.calico-plane
.calico-entry[data-sprite-state="throwReady"] ...
.calico-entry[data-sprite-state="throwSend"] ...
@keyframes paper-ready-bob
@keyframes paper-local-throw
```

**Step 2: Add base rig styles**

Add:

```css
.calico-rig {
  width: 126px;
  height: 126px;
  position: relative;
  pointer-events: none;
  transform-origin: 50% 76%;
  transition: transform 160ms ease;
}

.calico-svg {
  width: 126px;
  height: 126px;
  overflow: visible;
}

.calico-shadow {
  fill: rgba(15, 23, 42, 0.1);
  transform-origin: 66px 103px;
}

.calico-body,
.calico-head,
.calico-tail,
.calico-front-paw,
.calico-back-paw,
.calico-throw-paw,
.calico-projectile {
  transform-box: fill-box;
  transform-origin: center;
}

.calico-body,
.calico-head,
.calico-front-paw,
.calico-back-paw,
.calico-throw-paw {
  fill: #fff3df;
  stroke: #8a4b2d;
  stroke-width: 3;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.calico-tail {
  fill: #f2a34b;
  stroke: #8a4b2d;
  stroke-width: 3;
  stroke-linecap: round;
  stroke-linejoin: round;
  transform-origin: 22% 72%;
}

.calico-tail-tip,
.calico-body-patch-left,
.calico-body-patch-right,
.calico-head-patch-left,
.calico-head-patch-right {
  fill: #e99a48;
  stroke: none;
}

.calico-eye-left,
.calico-eye-right {
  fill: #5a2d22;
  stroke: none;
}

.calico-mouth,
.calico-whisker-left,
.calico-whisker-right {
  fill: none;
  stroke: #5a2d22;
  stroke-width: 2.4;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.calico-projectile {
  opacity: 0;
  fill: #f8fafc;
  stroke: #334155;
  stroke-width: 2;
  transform-origin: 0 0;
  filter: drop-shadow(0 4px 8px rgba(15, 23, 42, 0.14));
}

.calico-projectile-fold {
  fill: #dbeafe;
  stroke: none;
}
```

**Step 3: Add idle and hover behavior**

Add:

```css
.calico-entry:hover .calico-rig {
  transform: translateY(-2px) scale(1.03);
}

.calico-entry.is-pressing .calico-rig {
  transform: translateY(1px) scale(0.98);
}

.calico-entry[data-motion-state="dragging"] .calico-rig,
.calico-entry.is-dragging .calico-rig {
  transform: rotate(-2deg) scale(1.03);
}

.calico-entry[data-motion-state="idle"] .calico-rig {
  animation: calico-idle-breath 1800ms ease-in-out infinite alternate;
}

.calico-entry[data-motion-state="idle"] .calico-tail {
  animation: calico-idle-tail 2200ms ease-in-out infinite alternate;
}

@keyframes calico-idle-breath {
  from { transform: translateY(0) scale(1); }
  to { transform: translateY(-1px) scale(1.008); }
}

@keyframes calico-idle-tail {
  from { transform: rotate(-2deg); }
  to { transform: rotate(5deg); }
}
```

**Step 4: Add the ready pose**

Add:

```css
.calico-entry[data-motion-state="ready"] .calico-rig {
  animation: calico-ready-breath 1100ms ease-in-out infinite alternate;
}

.calico-entry[data-motion-state="ready"] .calico-body {
  transform: translate(-3px, 5px) rotate(-8deg) scaleX(1.03);
}

.calico-entry[data-motion-state="ready"] .calico-head {
  transform: translate(-4px, 1px) rotate(-6deg);
}

.calico-entry[data-motion-state="ready"] .calico-tail {
  transform: translate(2px, -4px) rotate(18deg);
}

.calico-entry[data-motion-state="ready"] .calico-support-paw {
  transform: translate(-2px, 3px) rotate(-5deg);
}

.calico-entry[data-motion-state="ready"] .calico-throw-paw {
  transform: translate(10px, -24px) rotate(-44deg);
}

.calico-entry[data-motion-state="ready"] .calico-projectile {
  opacity: 1;
  transform: translate(91px, 54px) rotate(-28deg) scale(0.9);
}

@keyframes calico-ready-breath {
  from {
    transform: translateY(0) rotate(-1deg);
  }
  to {
    transform: translateY(-2px) rotate(-2.5deg);
  }
}
```

**Step 5: Check the visual intent before moving on**

Manual visual expectation:

```text
Calico is no longer simply sitting.
Idle Calico still has a small life-like motion.
Body leans backward.
Paw is visibly raised.
Projectile sits near the raised paw.
Projectile does not cover the face.
Tail participates in the pose.
```

**Step 6: Run overlay tests**

Run:

```bash
npm test -- --run src/overlay/overlayHtml.test.ts
```

Expected: PASS for rig and ready-state CSS tests.

**Step 7: Commit after this task is complete during execution**

```bash
git add public/overlay.html
git commit -m "feat: add calico throw-ready pose"
```

---

### Task 5: Implement The Full Throw And Recovery Timeline

**Files:**
- Modify: `public/overlay.html`
- Test: `src/overlay/overlayHtml.test.ts`

**Step 1: Add throw CSS**

Add:

```css
.calico-entry[data-motion-state="throwing"] .calico-rig {
  animation: calico-throw-body 620ms cubic-bezier(0.2, 0.85, 0.22, 1) both;
}

.calico-entry[data-motion-state="throwing"] .calico-body {
  animation: calico-throw-body-part 620ms cubic-bezier(0.2, 0.85, 0.22, 1) both;
}

.calico-entry[data-motion-state="throwing"] .calico-head {
  animation: calico-throw-head 620ms cubic-bezier(0.2, 0.85, 0.22, 1) both;
}

.calico-entry[data-motion-state="throwing"] .calico-tail {
  animation: calico-throw-tail 620ms cubic-bezier(0.2, 0.85, 0.22, 1) both;
}

.calico-entry[data-motion-state="throwing"] .calico-throw-paw {
  animation: calico-throw-paw 620ms cubic-bezier(0.16, 0.9, 0.22, 1) both;
}

.calico-entry[data-motion-state="throwing"] .calico-projectile {
  animation: calico-held-projectile-release 620ms cubic-bezier(0.16, 0.9, 0.22, 1) both;
}

@keyframes calico-throw-body {
  0% { transform: translateY(0) rotate(-2deg); }
  22% { transform: translate(-4px, 3px) rotate(-8deg) scale(1.02); }
  45% { transform: translate(5px, -2px) rotate(7deg) scale(1.03); }
  100% { transform: translateY(0) rotate(0deg) scale(1); }
}

@keyframes calico-throw-body-part {
  0% { transform: translate(-3px, 5px) rotate(-8deg) scaleX(1.03); }
  35% { transform: translate(-5px, 6px) rotate(-12deg) scaleX(1.04); }
  58% { transform: translate(4px, 2px) rotate(8deg) scaleX(1.01); }
  100% { transform: translate(0, 0) rotate(0deg) scaleX(1); }
}

@keyframes calico-throw-head {
  0% { transform: translate(-4px, 1px) rotate(-6deg); }
  35% { transform: translate(-5px, 1px) rotate(-10deg); }
  58% { transform: translate(4px, -1px) rotate(7deg); }
  100% { transform: translate(0, 0) rotate(0deg); }
}

@keyframes calico-throw-tail {
  0% { transform: translate(2px, -4px) rotate(18deg); }
  35% { transform: translate(4px, -5px) rotate(28deg); }
  58% { transform: translate(-2px, -1px) rotate(-16deg); }
  100% { transform: translate(0, 0) rotate(0deg); }
}

@keyframes calico-throw-paw {
  0% { transform: translate(10px, -24px) rotate(-44deg); }
  30% { transform: translate(5px, -29px) rotate(-66deg); }
  48% { transform: translate(19px, -12px) rotate(34deg); }
  100% { transform: translate(0, 0) rotate(0deg); }
}

@keyframes calico-held-projectile-release {
  0% {
    opacity: 1;
    transform: translate(91px, 54px) rotate(-28deg) scale(0.9);
  }
  34% {
    opacity: 1;
    transform: translate(86px, 47px) rotate(-38deg) scale(0.92);
  }
  46% {
    opacity: 0;
    transform: translate(102px, 45px) rotate(-18deg) scale(0.86);
  }
  100% {
    opacity: 0;
    transform: translate(102px, 45px) rotate(-18deg) scale(0.86);
  }
}
```

**Step 2: Add recovery CSS**

Add:

```css
.calico-entry[data-motion-state="recovering"] .calico-rig {
  animation: calico-recover 260ms cubic-bezier(0.2, 0.9, 0.25, 1) both;
}

@keyframes calico-recover {
  0% { transform: translate(3px, -1px) rotate(4deg) scale(1.01); }
  65% { transform: translate(-1px, 1px) rotate(-2deg) scale(0.995); }
  100% { transform: translate(0, 0) rotate(0deg) scale(1); }
}
```

**Step 3: Add the JS timeline**

Replace `playPaperPlaneThrow` with:

```js
function playCalicoThrow() {
  window.clearTimeout(motionResetTimer);
  setMotionState('throwing');

  window.setTimeout(() => {
    invoke('show_paper_plane_flight_from_button').catch(() => {});
  }, THROW_RELEASE_MS);

  window.setTimeout(() => {
    setMotionState('recovering');
  }, THROW_RECOVER_MS);

  motionResetTimer = window.setTimeout(() => {
    resetCalicoMotion();
  }, THROW_RECOVER_MS + 280);
}
```

Update the event listener:

```js
tauri.event.listen('prompt-throw-send', () => {
  playCalicoThrow();
})
```

**Step 4: Preserve right-click and drag behavior**

Ensure:

```js
async function openButtonControls(event) {
  event.preventDefault();
  contextMenuOpened = true;
  start = null;
  dragging = false;
  lastMove = null;
  btn.classList.remove('is-pressing', 'is-dragging');
  resetCalicoMotion();
  await invoke('show_prompt_button_controls_from_button');
}
```

Ensure drag start uses:

```js
setMotionState('dragging');
```

Ensure drag end uses:

```js
resetCalicoMotion();
```

**Step 5: Add popover-dismiss reset listener**

Add:

```js
function listenForPromptPopoverDismissed() {
  if (!tauri?.event?.listen) return;
  tauri.event.listen('prompt-popover-dismissed', () => {
    resetCalicoMotion();
  }).catch((error) => {
    console.error('Tauri event listen failed: prompt-popover-dismissed', error);
  });
}

listenForPromptPopoverDismissed();
```

**Step 6: Run overlay tests**

Run:

```bash
npm test -- --run src/overlay/overlayHtml.test.ts
```

Expected: PASS.

**Step 7: Commit after this task is complete during execution**

```bash
git add public/overlay.html
git commit -m "feat: add calico throw and recovery timeline"
```

---

### Task 6: Align The Long-Distance Flight With The Paw Release Point

**Files:**
- Modify: `src-tauri/src/windows.rs`
- Modify: `public/paper-flight.html`
- Test: `src-tauri/src/windows.rs`

**Step 1: Update the Rust geometry test**

Find:

```rust
fn paper_flight_points_move_left_and_up_when_space_allows()
```

Adjust the expected start point to be the paw release point, not the old face/paper icon point:

```rust
let (sx, sy, ex, ey) = paper_flight_points(1440.0, 900.0, 1000.0, 600.0, 0.0, 0.0);

assert_eq!((sx, sy), (1102.0, 645.0));
assert!(ex < sx);
assert!(ey < sy);
```

The `1102, 645` value corresponds to:

```text
button_x + 102
button_y + 45
```

which matches the CSS release position:

```css
translate(102px, 45px)
```

**Step 2: Update `paper_flight_points`**

Change:

```rust
let start_x = button_x + 72.0 - monitor_x;
let start_y = button_y + 56.0 - monitor_y;
```

To:

```rust
let start_x = button_x + 102.0 - monitor_x;
let start_y = button_y + 45.0 - monitor_y;
```

Keep the end-point clamp behavior.

**Step 3: Update `public/paper-flight.html` visual asset class names**

Rename `.paper` to `.projectile` and update the image class:

```html
<img class="projectile" src="/calico/paper-plane.svg" alt="" />
```

Update CSS selectors accordingly:

```css
.projectile {
  position: absolute;
  left: var(--start-x);
  top: var(--start-y);
  width: 38px;
  height: 30px;
  opacity: 0;
  filter: drop-shadow(0 10px 18px rgba(15, 23, 42, 0.22));
  transform: translate(-50%, -50%) rotate(-18deg) scale(0.8);
  animation: projectile-fly 720ms cubic-bezier(0.18, 0.82, 0.22, 1) forwards;
}

@keyframes projectile-fly {
  ...
}
```

This keeps the existing image asset but makes the code no longer conceptually tied to "paper pasted on the face."

**Step 4: Run Rust tests**

Run:

```bash
cd src-tauri && cargo test windows::tests::paper_flight_points_move_left_and_up_when_space_allows
```

Expected: PASS.

**Step 5: Run all Rust tests for the window module**

Run:

```bash
cd src-tauri && cargo test windows::tests
```

Expected: PASS.

**Step 6: Commit after this task is complete during execution**

```bash
git add src-tauri/src/windows.rs public/paper-flight.html
git commit -m "feat: launch prompt projectile from calico paw"
```

---

### Task 7: Make Prompt Selection Drive Throw Motion Without Changing Autosend

**Files:**
- Modify: `src/App.tsx`
- Test: `src/app/App.test.tsx`

**Step 1: Ensure existing throw event helper remains**

Keep or add:

```ts
async function emitPromptThrowSend(kind: "single" | "group") {
  try {
    await emit("prompt-throw-send", { kind });
  } catch (error) {
    console.warn("Failed to emit prompt throw animation:", error);
  }
}
```

**Step 2: Keep event ordering**

In `handleSelect`, preserve this order:

```ts
await hidePromptPopover();
await waitForPopoverToHide();
await emitPromptThrowSend(prompt.type === "group" ? "group" : "single");
const bodies = getPromptContainerBodies(prompt);
const status = prompt.type === "group"
  ? await pastePromptSequenceAndSubmitToLastTarget(bodies, prompt.intervalMs)
  : await pastePromptAndSubmitToLastTarget(bodies[0]);
await emitAutosendStatus(status);
```

If there is already a hide delay, keep the existing delay constant. Do not add new backend delays.

**Step 3: Confirm group still emits one throw event**

Existing test should assert:

```ts
const throwCalls = emitMock.mock.calls.filter(([event]) => event === "prompt-throw-send");
expect(throwCalls).toHaveLength(1);
```

**Step 4: Run App tests**

Run:

```bash
npm test -- --run src/app/App.test.tsx
```

Expected: PASS.

**Step 5: Commit after this task is complete during execution**

```bash
git add src/App.tsx src/app/App.test.tsx
git commit -m "feat: drive calico throw from prompt selection"
```

---

### Task 8: Full Regression Verification

**Files:**
- No source changes unless tests reveal a defect.

**Step 1: Run frontend tests**

Run:

```bash
npm test -- --run
```

Expected:

```text
Test Files 13 passed
Tests 114+ passed
```

The exact test count may increase because this plan adds tests.

**Step 2: Run Rust tests**

Run:

```bash
cd src-tauri && cargo test
```

Expected:

```text
test result: ok
```

**Step 3: Run production build**

Run:

```bash
npm run tauri build
```

Expected:

```text
Finished 2 bundles at:
  /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app
  /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/dmg/Prompt Picker_1.0.0_aarch64.dmg
```

**Step 4: Check signing remains correct**

Run:

```bash
codesign -dv --verbose=4 "/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app" 2>&1 | sed -n '1,44p'
```

Expected:

```text
Identifier=local.promptpicker.dev
Authority=Apple Development: Jinhang Yang (33T9V6C2V2)
```

**Step 5: Manual visual verification**

Launch the packaged app and verify:

```text
Click Calico:
  Calico visibly enters throw-ready pose.
  Calico remains visually consistent with the existing cream/orange Calico IP.
  The projectile is near the raised paw.
  The projectile does not cover Calico's face.
  The prompt list appears above Calico.

Move mouse without selecting:
  Calico stays ready while the list remains open, then returns through the ready timeout if no action occurs.

Use existing non-send controls that hide the popover:
  Calico returns to idle through prompt-popover-dismissed.

Click one single prompt:
  prompt list hides.
  Calico throws once.
  projectile launches from paw.
  Calico recovers to idle.

Click one group prompt:
  Calico throws once for the group container.
  autosend sequence still sends group items one by one.

Right-click Calico:
  controls open.
  Calico does not remain in ready/throwing state.

Drag Calico:
  drag still works.
  Calico does not jump.
  Calico does not accidentally open the prompt list.
```

**Step 6: Commit after verification**

```bash
git add public/overlay.html public/paper-flight.html src/App.tsx src/app/App.test.tsx src/overlay/overlayHtml.test.ts src-tauri/src/windows.rs
git commit -m "feat: add complete calico throw motion system"
```

---

## Acceptance Criteria

- Calico no longer uses `throwReady: '/calico/calico-idle.apng'`.
- Calico no longer uses `throwSend: '/calico/calico-react-poke.apng'`.
- Calico remains recognizably the same IP character, not a generic replacement cat.
- Idle Calico still has a subtle life-like motion.
- Ready state visibly changes body, head, paw, tail, and projectile position.
- Throw state visibly includes wind-up, release, follow-through, and recovery.
- The projectile launch point aligns with the raised paw release point.
- The prompt list still opens from clicking Calico.
- Selecting a prompt still autosends through the existing backend path.
- Group prompts still send as a sequence and emit only one visual throw animation.
- Dragging and right-clicking Calico still work.
- No Accessibility, clipboard, autosend, prompt storage, or menu-bar behavior is changed.
- Frontend tests pass.
- Rust tests pass.
- Tauri build succeeds.

## Non-Acceptable Outcomes

- Calico remains in the old idle pose with an icon overlaid.
- The projectile covers Calico's face.
- The prompt list opens but Calico does not hold the ready pose.
- The projectile launches from the center of the face or old static location.
- The animation requires user-visible delays before autosend starts.
- A new quick-list close button is added only to satisfy animation reset tests.
- The button window is resized larger than `132px` by `132px`.
- The flight window captures clicks or focus.
- Existing non-send controls leave Calico stuck in ready pose.
- Group prompt execution plays a throw animation for every item in the group.
