# Calico Rich Idle Motion Director Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make Calico feel alive while idle by using the existing non-semantic motion assets broadly, without weakening the protected drag and send-success motions.

**Architecture:** Add a small idle motion director beside the existing Calico runtime. Keep the runtime responsible for rendering one requested state; keep the director responsible for low-priority idle scheduling, cooldowns, and protected-action pauses. Wire the director into `public/overlay.html` only after the manifest is loaded, so the current fallback path remains simple and safe.

**Tech Stack:** Tauri overlay HTML, browser ES modules, JavaScript, Vitest, TypeScript test files, existing Calico APNG/SVG assets.

---

## Current Context

The current overlay uses `idle-follow` as the default state:

- `public/calico/manifest.json` has `"defaultState": "idle-follow"`.
- `public/overlay.html` initializes the button with `data-motion-state="idle-follow"`.
- `public/overlay.html` initializes the sprite with `/calico/calico-idle-follow.svg`.
- `public/calico/motion-runtime.js` resets every finished motion back to `manifest.defaultState`.

The project already ships 27 Calico states:

- Protected and semantic states that must not be used as random idle motions:
  - `react-drag`
  - `happy`
  - `working-typing`
  - `working-conducting`
  - `working-juggling`
  - `working-building`
  - `working-carrying`
  - `working-sweeping`
  - `notification`
  - `error`
  - `thinking`
- Idle/life states that should be used by the new idle director:
  - `idle`
  - `yawning`
  - `dozing`
  - `collapsing`
  - `sleeping`
  - `waking`
  - `react-poke`
  - `react-left`
  - `mini-enter`
  - `mini-idle`
  - `mini-peek`
  - `mini-alert`
  - `mini-happy`
  - `mini-crabwalk`
  - `mini-sleep`

`idle-follow` remains the baseline resting state. It is not counted as a scheduled flourish; the director returns to it between idle motions.

## User Experience Contract

From the user's point of view:

- If the user does nothing, Calico should mostly rest in `idle-follow`, then occasionally show small life motions.
- The idle experience should feel richer than only `idle`, `yawning`, and `dozing`; use as many existing non-semantic assets as reasonable.
- The idle director must not make Calico constantly jump around. Motion should be occasional and staged by idle duration.
- Dragging Calico must immediately show `react-drag`. This is a protected favorite motion.
- Successful prompt send must show `happy` completely. This is a protected favorite motion.
- Clicking Calico to open the prompt panel must not switch into `thinking`, `react-poke`, or any sudden state.
- Work, error, notification, and success motions must be triggered only by their real events, not by idle randomness.

## Motion Tiers

Use these tiers in the first implementation.

```js
const IDLE_MOTION_TIERS = [
  {
    name: "light",
    availableAfterMs: 8_000,
    delayRangeMs: [9_000, 16_000],
    states: ["idle", "react-left", "mini-peek"],
  },
  {
    name: "settled",
    availableAfterMs: 25_000,
    delayRangeMs: [12_000, 22_000],
    states: ["yawning", "dozing", "react-poke", "mini-enter", "mini-idle", "mini-crabwalk"],
  },
  {
    name: "deep",
    availableAfterMs: 60_000,
    delayRangeMs: [18_000, 32_000],
    states: ["collapsing", "sleeping", "waking", "mini-happy", "mini-sleep", "mini-alert"],
  },
];
```

This uses 15 scheduled idle/life states, plus `idle-follow` as the baseline. Do not add `thinking`, `happy`, `react-drag`, `error`, `notification`, or any `working-*` state to these tiers.

## Important Priority Rule

Some idle-looking reserved states have high manifest priority, for example `react-poke`, `react-left`, and `mini-*`. The idle director must call runtime `apply` with an explicit low priority:

```js
applyMotion({ state, reason: "idle-director", priority: 1, durationMs });
```

Without this override, an idle flourish could temporarily block `happy`, `error`, or another semantic motion during `minMs`. This would be a regression.

## Scope Boundaries

- Do not change Calico asset files.
- Do not add new animation assets.
- Do not change prompt autosend behavior.
- Do not change drag threshold `DRAG_START_DISTANCE_PX = 10`.
- Do not reintroduce paper-plane throw assets or throw states.
- Do not trigger `thinking` from plain click-to-open.
- Do not let idle motion run while dragging, pointer is down, or button controls are opening.
- Do not stage or commit `dist`, `node_modules`, `src-tauri/target`, release bundles, or unrelated parallel-task files.

---

### Task 1: Add Idle Director Unit Tests

**Files:**
- Create: `src/overlay/calicoIdleDirector.test.ts`
- Create later: `public/calico/idle-director.js`

**Step 1: Write the failing tests**

Create `src/overlay/calicoIdleDirector.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";

const protectedStates = [
  "react-drag",
  "happy",
  "thinking",
  "working-typing",
  "working-conducting",
  "working-juggling",
  "working-building",
  "working-carrying",
  "working-sweeping",
  "notification",
  "error",
];

type IdleMotionTier = {
  name: string;
  availableAfterMs: number;
  delayRangeMs: [number, number];
  states: string[];
};

type IdleDirectorModule = {
  IDLE_MOTION_TIERS: IdleMotionTier[];
  createCalicoIdleDirector(options: {
    applyMotion: (payload: { state: string; priority?: number; reason?: string }) => boolean;
    resetMotion: () => void;
    getCurrentState: () => string;
    isUserActive: () => boolean;
    random?: () => number;
    setTimeout?: typeof window.setTimeout;
    clearTimeout?: typeof window.clearTimeout;
    now?: () => number;
  }): {
    start(): void;
    stop(): void;
    pause(durationMs?: number): void;
    resetIdleClock(): void;
    resetToBaseline(): void;
  };
};

async function loadDirectorModule() {
  // @ts-expect-error public overlay module is intentionally outside the src build graph.
  return (await import("../../public/calico/idle-director.js")) as IdleDirectorModule;
}

describe("Calico idle director", () => {
  it("uses a broad idle motion pool without protected semantic motions", async () => {
    const { IDLE_MOTION_TIERS } = await loadDirectorModule();
    const states = IDLE_MOTION_TIERS.flatMap((tier: { states: string[] }) => tier.states);

    expect(new Set(states).size).toBe(states.length);
    expect(states).toEqual(
      expect.arrayContaining([
        "idle",
        "yawning",
        "dozing",
        "collapsing",
        "sleeping",
        "waking",
        "react-poke",
        "react-left",
        "mini-enter",
        "mini-idle",
        "mini-peek",
        "mini-alert",
        "mini-happy",
        "mini-crabwalk",
        "mini-sleep",
      ])
    );
    expect(states.length).toBeGreaterThanOrEqual(15);
    for (const state of protectedStates) {
      expect(states).not.toContain(state);
    }
  });

  it("starts from idle-follow and schedules low-priority idle flourishes", async () => {
    vi.useFakeTimers();
    const { createCalicoIdleDirector } = await loadDirectorModule();
    const applied: Array<{ state: string; priority?: number; reason?: string }> = [];

    const director = createCalicoIdleDirector({
      applyMotion: (payload: { state: string; priority?: number; reason?: string }) => {
        applied.push(payload);
        return true;
      },
      resetMotion: vi.fn(),
      getCurrentState: () => "idle-follow",
      isUserActive: () => false,
      random: () => 0,
      setTimeout: window.setTimeout.bind(window),
      clearTimeout: window.clearTimeout.bind(window),
      now: () => Date.now(),
    });

    director.start();
    vi.advanceTimersByTime(8_000 + 9_000);

    expect(applied).toContainEqual(
      expect.objectContaining({
        state: "idle",
        reason: "idle-director",
        priority: 1,
      })
    );
    vi.useRealTimers();
  });

  it("pauses idle scheduling while protected motion is active", async () => {
    vi.useFakeTimers();
    const { createCalicoIdleDirector } = await loadDirectorModule();
    const applied: Array<{ state: string }> = [];
    let active = false;

    const director = createCalicoIdleDirector({
      applyMotion: (payload: { state: string }) => {
        applied.push(payload);
        return true;
      },
      resetMotion: vi.fn(),
      getCurrentState: () => "idle-follow",
      isUserActive: () => active,
      random: () => 0,
      setTimeout: window.setTimeout.bind(window),
      clearTimeout: window.clearTimeout.bind(window),
      now: () => Date.now(),
    });

    director.start();
    active = true;
    director.pause(4_000);
    vi.advanceTimersByTime(30_000);

    expect(applied).toEqual([]);
    active = false;
    vi.useRealTimers();
  });

  it("does not play idle flourishes while a semantic state is still visible", async () => {
    vi.useFakeTimers();
    const { createCalicoIdleDirector } = await loadDirectorModule();
    const applied: Array<{ state: string }> = [];
    let currentState = "happy";

    const director = createCalicoIdleDirector({
      applyMotion: (payload: { state: string }) => {
        applied.push(payload);
        return true;
      },
      resetMotion: vi.fn(),
      getCurrentState: () => currentState,
      isUserActive: () => false,
      random: () => 0,
      setTimeout: window.setTimeout.bind(window),
      clearTimeout: window.clearTimeout.bind(window),
      now: () => Date.now(),
    });

    director.start();
    vi.advanceTimersByTime(60_000);
    expect(applied).toEqual([]);

    currentState = "react-drag";
    vi.advanceTimersByTime(10_000);
    expect(applied).toEqual([]);

    currentState = "idle-follow";
    vi.advanceTimersByTime(3_000);
    expect(applied.length).toBeGreaterThan(0);
    vi.useRealTimers();
  });

  it("does not treat a rejected idle flourish as played", async () => {
    vi.useFakeTimers();
    const { createCalicoIdleDirector } = await loadDirectorModule();
    const applied: Array<{ state: string }> = [];
    let allowApply = false;

    const director = createCalicoIdleDirector({
      applyMotion: (payload: { state: string }) => {
        if (!allowApply) return false;
        applied.push(payload);
        return true;
      },
      resetMotion: vi.fn(),
      getCurrentState: () => "idle-follow",
      isUserActive: () => false,
      random: () => 0,
      setTimeout: window.setTimeout.bind(window),
      clearTimeout: window.clearTimeout.bind(window),
      now: () => Date.now(),
    });

    director.start();
    vi.advanceTimersByTime(8_000 + 9_000);
    expect(applied).toEqual([]);

    allowApply = true;
    vi.advanceTimersByTime(2_000);
    expect(applied[0]?.state).toBe("idle");
    vi.useRealTimers();
  });
});
```

**Step 2: Run the test to verify it fails**

Run:

```bash
npm test -- src/overlay/calicoIdleDirector.test.ts
```

Expected: FAIL because `public/calico/idle-director.js` does not exist.

**Step 3: Commit**

Do not commit yet. This task should be committed together with Task 2 once the tests pass.

---

### Task 2: Implement the Idle Director Module

**Files:**
- Create: `public/calico/idle-director.js`
- Test: `src/overlay/calicoIdleDirector.test.ts`

**Step 1: Add the module**

Create `public/calico/idle-director.js`:

```js
export const IDLE_MOTION_TIERS = [
  {
    name: "light",
    availableAfterMs: 8_000,
    delayRangeMs: [9_000, 16_000],
    states: ["idle", "react-left", "mini-peek"],
  },
  {
    name: "settled",
    availableAfterMs: 25_000,
    delayRangeMs: [12_000, 22_000],
    states: ["yawning", "dozing", "react-poke", "mini-enter", "mini-idle", "mini-crabwalk"],
  },
  {
    name: "deep",
    availableAfterMs: 60_000,
    delayRangeMs: [18_000, 32_000],
    states: ["collapsing", "sleeping", "waking", "mini-happy", "mini-sleep", "mini-alert"],
  },
];

const DEFAULT_IDLE_DISPLAY_MS = 3_200;
const BASELINE_STATE = "idle-follow";
const IDLE_PRIORITY = 1;

const DISPLAY_OVERRIDES_MS = {
  idle: 5_200,
  yawning: 8_000,
  dozing: 3_000,
  collapsing: 5_200,
  sleeping: 6_500,
  waking: 5_800,
  "mini-idle": 3_500,
  "mini-sleep": 5_500,
};

function clampElapsed(value) {
  return Number.isFinite(value) && value > 0 ? value : 0;
}

function pick(items, random) {
  if (items.length === 0) return null;
  const index = Math.min(items.length - 1, Math.floor(random() * items.length));
  return items[index];
}

function randomDelay([min, max], random) {
  return Math.round(min + (max - min) * random());
}

export function createCalicoIdleDirector({
  applyMotion,
  resetMotion,
  getCurrentState,
  isUserActive,
  random = Math.random,
  setTimeout: schedule = window.setTimeout.bind(window),
  clearTimeout: cancel = window.clearTimeout.bind(window),
  now = () => Date.now(),
}) {
  let timer = 0;
  let running = false;
  let idleStartedAt = now();
  let pausedUntil = 0;
  let lastState = "";

  function clearTimer() {
    cancel(timer);
    timer = 0;
  }

  function eligibleTiers() {
    const elapsed = clampElapsed(now() - idleStartedAt);
    return IDLE_MOTION_TIERS.filter((tier) => elapsed >= tier.availableAfterMs);
  }

  function nextDelay() {
    const tiers = eligibleTiers();
    const tier = tiers[tiers.length - 1] ?? IDLE_MOTION_TIERS[0];
    return randomDelay(tier.delayRangeMs, random);
  }

  function scheduleNext(delay = nextDelay()) {
    clearTimer();
    if (!running) return;
    timer = schedule(playNext, delay);
  }

  function playableStates() {
    const states = eligibleTiers().flatMap((tier) => tier.states);
    return states.filter((state) => state !== lastState);
  }

  function displayMsFor(state) {
    return DISPLAY_OVERRIDES_MS[state] ?? DEFAULT_IDLE_DISPLAY_MS;
  }

  function playNext() {
    if (!running) return;
    if (isUserActive?.()) {
      scheduleNext(1_500);
      return;
    }
    if (now() < pausedUntil) {
      scheduleNext(pausedUntil - now() + 1_000);
      return;
    }
    const currentState = getCurrentState?.();
    if (currentState && currentState !== BASELINE_STATE) {
      scheduleNext(2_000);
      return;
    }

    const state = pick(playableStates(), random);
    if (!state) {
      scheduleNext(2_000);
      return;
    }

    const durationMs = displayMsFor(state);
    const applied = applyMotion({
      state,
      reason: "idle-director",
      priority: IDLE_PRIORITY,
      durationMs,
    });
    if (!applied) {
      scheduleNext(2_000);
      return;
    }
    lastState = state;
    scheduleNext(durationMs + nextDelay());
  }

  function start() {
    if (running) return;
    running = true;
    idleStartedAt = now();
    scheduleNext();
  }

  function stop() {
    running = false;
    clearTimer();
  }

  function pause(durationMs = 3_000) {
    pausedUntil = Math.max(pausedUntil, now() + durationMs);
    clearTimer();
    if (running) {
      scheduleNext(durationMs + 1_000);
    }
  }

  function resetIdleClock() {
    idleStartedAt = now();
    lastState = "";
  }

  function resetToBaseline() {
    resetIdleClock();
    resetMotion?.();
    pause(3_000);
  }

  return {
    start,
    stop,
    pause,
    resetIdleClock,
    resetToBaseline,
  };
}
```

**Step 2: Run the tests**

Run:

```bash
npm test -- src/overlay/calicoIdleDirector.test.ts
```

Expected: PASS.

**Step 3: Commit**

```bash
git add public/calico/idle-director.js src/overlay/calicoIdleDirector.test.ts
git commit -m "feat: add Calico idle motion director"
```

Do not stage generated artifacts.

---

### Task 3: Wire the Idle Director Into the Overlay

**Files:**
- Modify: `public/overlay.html`
- Modify: `src/overlay/overlayHtml.test.ts`

**Step 1: Write failing overlay tests**

Add tests to `src/overlay/overlayHtml.test.ts`:

```ts
it("loads the Calico idle director after the motion runtime", () => {
  const html = readOverlayHtml();

  expect(html).toContain("/calico/idle-director.js");
  expect(html).toContain("createCalicoIdleDirector");
  expect(html).toContain("calicoIdleDirector.start()");
});

it("pauses idle motion for drag and external Calico events", () => {
  const html = readOverlayHtml();

  expect(html).toContain("calicoIdleDirector?.pause");
  expect(html).toContain("applyCalicoMotion({ state: 'react-drag'");
  expect(html).toContain("pauseIdleForExternalMotion(event.payload)");
  expect(html).toContain("pauseIdleForPointerInteraction");
});

it("does not let click-to-open trigger an idle flourish directly", () => {
  const html = readOverlayHtml();
  const clickBlockStart = html.indexOf("const sessionId = ++promptPickSessionId");
  const clickBlockEnd = html.indexOf("start = null;", clickBlockStart);
  const clickBlock = html.slice(clickBlockStart, clickBlockEnd);

  expect(clickBlock).not.toContain("react-poke");
  expect(clickBlock).not.toContain("thinking");
  expect(clickBlock).not.toContain("idle-director");
  expect(clickBlock).toContain("pauseIdleForPointerInteraction");
  expect(clickBlock).not.toContain("applyCalicoMotion");
});

it("does not fallback when an initialized runtime rejects a lower-priority motion", () => {
  const html = readOverlayHtml();

  expect(html).toContain("if (calicoMotion) return calicoMotion.apply(payload);");
  expect(html).not.toContain("if (calicoMotion?.apply(payload)) return true;");
});
```

If `readOverlayHtml` is not already a helper in this test file, use the existing local helper pattern already present in `src/overlay/overlayHtml.test.ts`.

**Step 2: Run tests to verify they fail**

Run:

```bash
npm test -- src/overlay/overlayHtml.test.ts
```

Expected: FAIL because `overlay.html` does not import or start the idle director.

**Step 3: Import the idle director**

In `public/overlay.html`, change:

```js
import { createCalicoMotionRuntime } from '/calico/motion-runtime.js';
```

to:

```js
import { createCalicoMotionRuntime } from '/calico/motion-runtime.js';
import { createCalicoIdleDirector } from '/calico/idle-director.js';
```

**Step 4: Add state for the director**

Near the current Calico state variables:

```js
let calicoMotion = null;
```

add:

```js
let calicoIdleDirector = null;
```

**Step 5: Make `applyCalicoMotion` return whether it applied**

Change:

```js
function applyCalicoMotion(payload = {}) {
  if (calicoMotion?.apply(payload)) return;
  applyFallbackMotion(payload);
}
```

to:

```js
function applyCalicoMotion(payload = {}) {
  if (calicoMotion) return calicoMotion.apply(payload);
  applyFallbackMotion(payload);
  return true;
}
```

Reason: fallback is only for the manifest/runtime unavailable path. If the initialized runtime rejects a lower-priority motion during a protected minimum display window, the correct behavior is to keep the current motion, not to fallback to `idle-follow`.

**Step 6: Add external-motion pause helper**

Add after `applyCalicoMotion`:

```js
function pauseIdleForExternalMotion(payload = {}) {
  const durationMs = Number.isFinite(payload.durationMs) ? payload.durationMs : 3_600;
  calicoIdleDirector?.pause(durationMs + 1_000);
}

function pauseIdleForPointerInteraction(durationMs = 5_000) {
  calicoIdleDirector?.pause(durationMs);
}
```

This helper does not need perfect semantic duration knowledge because the runtime itself owns the actual auto-return. It just prevents idle flourishes from starting immediately after an external action.

**Step 7: Start the director after manifest load**

In `initializeCalicoMotion`, after:

```js
calicoMotion = createCalicoMotionRuntime({ image: sprite, host: btn, manifest });
resetCalicoMotion();
```

add:

```js
calicoIdleDirector = createCalicoIdleDirector({
  applyMotion: applyCalicoMotion,
  resetMotion: resetCalicoMotion,
  getCurrentState: () => btn.dataset.motionState,
  isUserActive: () => dragging || Boolean(start) || contextMenuOpened,
});
calicoIdleDirector.start();
```

Do not start the director in the catch/fallback path. The fallback only knows `idle-follow` and `react-drag`.

**Step 8: Pause idle on external Calico events**

Change the `calico-motion` listener from:

```js
tauri.event.listen('calico-motion', (event) => {
  applyCalicoMotion(event.payload);
}).catch((error) => {
```

to:

```js
tauri.event.listen('calico-motion', (event) => {
  pauseIdleForExternalMotion(event.payload);
  applyCalicoMotion(event.payload);
}).catch((error) => {
```

**Step 9: Pause idle on drag**

In the pointer move block, just before or immediately after applying `react-drag`, add:

```js
calicoIdleDirector?.pause(4_000);
```

Keep the existing `applyCalicoMotion({ state: 'react-drag', reason: 'drag' });` line.

**Step 10: Pause idle around pointer press and click-to-open**

In the non-context-menu `pointerdown` path, after the right-click / Ctrl-click early return and before creating `start`, add:

```js
pauseIdleForPointerInteraction(5_000);
```

This does not change Calico's visible state. It only prevents a pre-scheduled idle flourish from firing while the user is pressing or clicking the pet.

In the non-drag click branch, before:

```js
const toggleResult = await invoke('toggle_prompt_popover_from_button', { sessionId });
```

add:

```js
pauseIdleForPointerInteraction(6_000);
```

This protects the click-to-open path from an idle flourish starting immediately after the panel opens. Do not call `applyCalicoMotion` here.

In `openButtonControls`, after `resetCalicoMotion();`, add:

```js
pauseIdleForPointerInteraction(6_000);
```

**Step 11: Reset director after drag ends**

Where the code currently does:

```js
if (wasDragging) {
  resetCalicoMotion();
}
```

change to:

```js
if (wasDragging) {
  calicoIdleDirector?.resetToBaseline();
}
```

If the director is unavailable, keep a fallback:

```js
if (wasDragging) {
  if (calicoIdleDirector) {
    calicoIdleDirector.resetToBaseline();
  } else {
    resetCalicoMotion();
  }
}
```

**Step 12: Keep click-to-open neutral**

Do not add any idle or reaction call in the non-drag click branch:

```js
const toggleResult = await invoke('toggle_prompt_popover_from_button', { sessionId });
```

Plain click-to-open should not call `applyCalicoMotion`. It may call `pauseIdleForPointerInteraction(...)` because that does not change the visible motion.

**Step 13: Run overlay tests**

Run:

```bash
npm test -- src/overlay/overlayHtml.test.ts
```

Expected: PASS.

**Step 14: Commit**

```bash
git add public/overlay.html src/overlay/overlayHtml.test.ts
git commit -m "feat: run Calico idle motions in overlay"
```

---

### Task 4: Strengthen Runtime Protection Tests

**Files:**
- Modify: `src/overlay/calicoMotionRuntime.test.ts`
- Verify: `public/calico/motion-runtime.js`

**Step 1: Add a test proving low-priority idle does not block happy**

In `src/overlay/calicoMotionRuntime.test.ts`, add:

```ts
it("allows semantic motion to interrupt low-priority idle flourishes", async () => {
  vi.useFakeTimers();
  const { createCalicoMotionRuntime } = await loadRuntime();
  const { image, host } = elements();
  const runtime = createCalicoMotionRuntime({ image, host, manifest, now: () => Date.now() });

  runtime.apply({ state: "react-left", reason: "idle-director", priority: 1 });
  runtime.apply({ state: "happy" });

  expect(host.dataset.motionState).toBe("happy");
  expect(image.getAttribute("src")).toContain("/calico/calico-happy.apng");
  vi.useRealTimers();
});
```

If the local test manifest in this file does not include `react-left`, add it to the test manifest with:

```ts
"react-left": {
  file: "/calico/calico-react-left.apng",
  priority: 80,
  durationMs: 2500,
  minMs: 800,
  replay: true,
  scale: 1.05,
  offsetX: 10,
  offsetY: 0,
},
```

**Step 2: Run the runtime tests**

Run:

```bash
npm test -- src/overlay/calicoMotionRuntime.test.ts
```

Expected: PASS.

**Step 3: Commit**

```bash
git add src/overlay/calicoMotionRuntime.test.ts
git commit -m "test: protect Calico semantic motion priority"
```

---

### Task 5: Add Manifest Guard for Idle Pool Assets

**Files:**
- Modify: `src/overlay/calicoManifest.test.ts`
- Test: `public/calico/manifest.json`
- Test: `public/calico/idle-director.js`

**Step 1: Add a test that every idle director state exists in the manifest**

Add to `src/overlay/calicoManifest.test.ts`:

```ts
it("declares every idle director state in the Calico manifest", async () => {
  const manifest = readManifest();
  // @ts-expect-error public overlay module is intentionally outside the src build graph.
  const { IDLE_MOTION_TIERS } = await import("../../public/calico/idle-director.js");
  const idleStates = IDLE_MOTION_TIERS.flatMap((tier: { states: string[] }) => tier.states);

  for (const state of idleStates) {
    expect(manifest.states[state], state).toBeTruthy();
    expect(manifest.states[state].file, state).toMatch(/^\/calico\/calico-.+\.(apng|svg)$/);
  }
});
```

**Step 2: Run manifest tests**

Run:

```bash
npm test -- src/overlay/calicoManifest.test.ts
```

Expected: PASS.

**Step 3: Commit**

```bash
git add src/overlay/calicoManifest.test.ts
git commit -m "test: guard Calico idle motion assets"
```

---

### Task 6: Focused Verification

**Files:**
- Verify: `public/overlay.html`
- Verify: `public/calico/idle-director.js`
- Verify: `public/calico/motion-runtime.js`
- Verify: `public/calico/manifest.json`

**Step 1: Run focused tests**

Run:

```bash
npm test -- \
  src/overlay/calicoIdleDirector.test.ts \
  src/overlay/overlayHtml.test.ts \
  src/overlay/calicoMotionRuntime.test.ts \
  src/overlay/calicoManifest.test.ts \
  src/app/App.test.tsx
```

Expected: PASS.

Why `src/app/App.test.tsx` is included:

- It already verifies `working-*`, `happy`, `notification`, and `error` event emissions.
- It already verifies prompt panel opening does not emit the old unwanted thinking motion in reused popover cases.

**Step 2: Run type check**

Run:

```bash
npx tsc --noEmit
```

Expected: PASS.

**Step 3: Build to a temp output**

Run:

```bash
npx vite build --outDir /tmp/prompt-picker-calico-idle-director-build --emptyOutDir
```

Expected: PASS.

Do not build to tracked `dist`.

---

### Task 7: Browser Verification

**Files:**
- Verify: `public/overlay.html`
- Verify: `public/calico/idle-director.js`
- Optional docs: `docs/qa/calico-rich-idle-motion-director.md`

**Step 1: Start Vite dev server**

Run:

```bash
npm run dev -- --host 127.0.0.1
```

Expected: Vite prints a local URL, usually `http://127.0.0.1:1420/`.

**Step 2: Open the overlay directly**

Open:

```text
http://127.0.0.1:1420/overlay.html
```

Expected:

- The Calico sprite renders.
- Initial state is `idle-follow`.
- No console errors from loading `/calico/idle-director.js`.
- No console errors from loading `/calico/manifest.json`.

**Step 3: Inspect idle state changes with browser console or Playwright**

Use Playwright or browser console to observe:

```js
document.getElementById("btn").dataset.motionState
```

Expected:

- Starts as `idle-follow`.
- Eventually changes to a scheduled idle state such as `idle`, `react-left`, or `mini-peek`.
- Returns to `idle-follow` between flourishes.

If using Playwright, do not wait for every state in the full pool. Unit tests cover the pool. Browser verification only needs to prove that the director loads and schedules at least one idle flourish.

**Step 4: Verify protected interactions in the real app**

Run the Tauri app as appropriate for this project, then verify manually:

- Dragging Calico still switches immediately to the flying/drag motion `react-drag`.
- Releasing after drag returns to `idle-follow`; idle scheduling resumes only after a short pause.
- Sending a prompt successfully still plays the favorite `happy` motion.
- Clicking Calico to open the prompt panel does not switch to `thinking`, `react-poke`, or another abrupt motion.
- Idle flourishes do not fire while the pointer is down or during dragging.
- Idle flourishes do not fire for at least a few seconds immediately after click-to-open.

**Step 5: Spot-check mini and sleep-family visual framing**

The unit tests prove the full idle pool exists. Browser verification must still spot-check the riskiest visual states because several mini/sleep assets use large offsets or zero manifest duration.

In the browser console for `http://127.0.0.1:1420/overlay.html`, run this helper and inspect each state:

```js
const manifest = await fetch("/calico/manifest.json").then((response) => response.json());
const btn = document.getElementById("btn");
const sprite = document.getElementById("calicoSprite");
const states = ["sleeping", "waking", "mini-enter", "mini-idle", "mini-peek", "mini-happy", "mini-crabwalk", "mini-sleep", "mini-alert"];
for (const state of states) {
  const entry = manifest.states[state];
  btn.dataset.motionState = state;
  sprite.src = `${entry.file}?qa=${Date.now()}`;
  sprite.style.setProperty("--calico-scale", String(entry.scale ?? 1));
  sprite.style.setProperty("--calico-offset-x", `${entry.offsetX ?? 0}px`);
  sprite.style.setProperty("--calico-offset-y", `${entry.offsetY ?? 0}px`);
  console.log(state);
  await new Promise((resolve) => setTimeout(resolve, 1400));
}
```

Expected:

- No state is fully clipped out of the 132x132 overlay window.
- Mini states can sit lower, but the primary character should remain readable.
- Sleep-family states should not create a jarring permanent state because the idle director supplies explicit `durationMs` overrides.

**Step 6: Stop dev server**

Use `Ctrl-C`.

**Step 7: Optional QA record**

If screenshots or notes are captured, add:

```text
docs/qa/calico-rich-idle-motion-director.md
```

Keep this short. Do not add large videos unless explicitly requested.

---

### Task 8: Final Verification and Push

**Files:**
- No new code changes expected.

**Step 1: Run the full test suite**

Run:

```bash
npm test
```

Expected: PASS.

**Step 2: Run type check and build again**

Run:

```bash
npx tsc --noEmit
npx vite build --outDir /tmp/prompt-picker-calico-idle-director-final-build --emptyOutDir
```

Expected: PASS.

**Step 3: Check git status**

Run:

```bash
git status --short --branch
```

Expected:

- Only this task's files are changed/staged.
- No `dist`, `node_modules`, `src-tauri/target`, release bundles, or unrelated parallel-task files are staged.

**Step 4: Push**

Only after all tests and verification pass:

```bash
git push origin HEAD:main
```

Do not force-push.

---

## Acceptance Criteria

- Calico no longer stays indefinitely on the same visible resting pose when untouched.
- Idle scheduling uses at least 15 non-semantic idle/life states from the existing manifest.
- `idle-follow` remains the baseline default state.
- Scheduled idle motions use explicit low priority so they cannot block semantic motions.
- Dragging Calico still immediately shows `react-drag`.
- Prompt send success still shows `happy`.
- Plain click-to-open does not trigger `thinking`, `react-poke`, or any abrupt action.
- Plain click-to-open pauses idle scheduling briefly without changing the visible motion.
- `working-*`, `notification`, and `error` remain event-driven only.
- Idle motion does not run during pointer down, drag, or button-controls opening.
- Idle motion does not start while the current visible state is `happy`, `react-drag`, `error`, `notification`, or any other non-`idle-follow` semantic state.
- Mini and sleep-family idle states are visually spot-checked for clipping and jarring offsets.
- The fallback path still supports `idle-follow` and `react-drag` if manifest loading fails.
- Focused overlay tests pass.
- Full test suite passes.
- Type check passes.
- Vite build to `/tmp` passes.
- No generated artifacts are staged or committed.

## Notes for Execution

The current local working tree has unrelated dirty files and is behind `origin/main`. Execute this plan in a clean worktree or carefully stage only files listed in this plan. Do not reset or overwrite the user's parallel task changes.
