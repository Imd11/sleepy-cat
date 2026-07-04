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
