export const IDLE_RHYTHM_PHASES = [
  { name: "early", availableAfterMs: 7_000, delayRangeMs: [2_500, 5_000] },
  { name: "settled", availableAfterMs: 30_000, delayRangeMs: [2_000, 4_500] },
  { name: "longIdle", availableAfterMs: 90_000, delayRangeMs: [3_000, 6_000] },
  { name: "deepIdle", availableAfterMs: 10 * 60_000, delayRangeMs: [45_000, 90_000] },
];

export const IDLE_MOTION_POOL = [
  { state: "idle", category: "light", weights: { early: 8, settled: 5, longIdle: 3, deepIdle: 2 } },
  { state: "react-left", category: "light", weights: { early: 6, settled: 4, longIdle: 3, deepIdle: 0 } },
  { state: "yawning", category: "life", weights: { early: 1, settled: 6, longIdle: 4, deepIdle: 0 } },
  {
    state: "react-poke",
    category: "attention",
    weights: { early: 1, settled: 2, longIdle: 1, deepIdle: 0 },
  },
];

const BASELINE_STATE = "idle";
const SLEEP_SEQUENCE = ["dozing", "collapsing", "sleeping"];
const IDLE_PRIORITY = 1;
const QUIET_START_MS = 7_000;
const HOVER_PRIORITY = 2;
const HOVER_COOLDOWN_MS = 10_000;
const HOVER_IDLE_PAUSE_MS = 6_000;
const RESTING_STATES = new Set(["sleeping", "dozing", "mini-sleep"]);
const NEUTRAL_ATTENTION_STATES = new Set(["idle-follow", "idle"]);
const PROTECTED_STATES = new Set([
  "happy",
  "react-drag",
  "error",
  "notification",
  "thinking",
  "working-typing",
  "working-conducting",
  "working-juggling",
  "working-building",
  "working-carrying",
  "working-sweeping",
]);

function clampElapsed(value) {
  return Math.max(0, value);
}

function randomDelay(range, random) {
  const [min, max] = range;
  return Math.round(min + (max - min) * random());
}

function weightedEntriesForPhase(phaseName) {
  return IDLE_MOTION_POOL.filter((entry) => (entry.weights[phaseName] ?? 0) > 0);
}

function pickWeighted(entries, phaseName, random) {
  const total = entries.reduce((sum, entry) => sum + (entry.weights[phaseName] ?? 0), 0);
  if (total <= 0) return null;

  let threshold = random() * total;
  for (const entry of entries) {
    threshold -= entry.weights[phaseName] ?? 0;
    if (threshold <= 0) return entry.state;
  }
  return entries[entries.length - 1]?.state ?? null;
}

function attentionStateFor(currentState) {
  if (PROTECTED_STATES.has(currentState)) return null;
  if (RESTING_STATES.has(currentState)) return "waking";
  if (NEUTRAL_ATTENTION_STATES.has(currentState)) return "react-poke";
  return "react-poke";
}

export function createCalicoIdleDirector({
  applyMotion,
  resetMotion,
  getCurrentState,
  isUserActive,
  random = Math.random,
  setTimeout: setTimer = globalThis.setTimeout.bind(globalThis),
  clearTimeout: clearTimer = globalThis.clearTimeout.bind(globalThis),
  now = () => Date.now(),
  motionDurations = {},
} = {}) {
  let running = false;
  let timer = 0;
  let idleStartedAt = now();
  let pausedUntil = 0;
  let lastState = "";
  let attentionCooldownUntil = 0;

  function displayMsFor(state) {
    const duration = motionDurations[state];
    if (Number.isFinite(duration) && duration > 0) return duration;
    return 2600;
  }

  function currentPhase() {
    const elapsed = clampElapsed(now() - idleStartedAt);
    const phases = IDLE_RHYTHM_PHASES.filter((phase) => elapsed >= phase.availableAfterMs);
    return phases[phases.length - 1] ?? null;
  }

  function nextDelay() {
    const phase = currentPhase() ?? IDLE_RHYTHM_PHASES[0];
    return randomDelay(phase.delayRangeMs, random);
  }

  function playableEntries(phaseName) {
    return weightedEntriesForPhase(phaseName).filter((entry) => entry.state !== lastState);
  }

  function scheduleNext(delayMs) {
    clearTimer(timer);
    if (!running) return;
    timer = setTimer(playNext, Math.max(0, delayMs));
    timer?.unref?.();
  }

  function resetIdleClock() {
    idleStartedAt = now();
  }

  function playNext() {
    if (!running) return;

    const pauseRemaining = pausedUntil - now();
    if (pauseRemaining > 0) {
      scheduleNext(pauseRemaining);
      return;
    }

    if (isUserActive?.()) {
      scheduleNext(1000);
      return;
    }

    const currentState = getCurrentState?.() ?? BASELINE_STATE;
    if (RESTING_STATES.has(currentState)) {
      scheduleNext(IDLE_RHYTHM_PHASES[3].delayRangeMs[0]);
      return;
    }
    if (PROTECTED_STATES.has(currentState)) {
      scheduleNext(nextDelay());
      return;
    }

    const phase = currentPhase();
    if (!phase) {
      scheduleNext(1000);
      return;
    }

    const state = pickWeighted(playableEntries(phase.name), phase.name, random);
    if (!state) {
      scheduleNext(nextDelay());
      return;
    }

    const durationMs = displayMsFor(state);
    const sequence = state === "yawning" ? SLEEP_SEQUENCE : [];
    const applied = applyMotion?.({
      state,
      reason: "idle-director",
      priority: IDLE_PRIORITY,
      durationMs,
      sequence,
    });
    if (applied) {
      lastState = state;
      scheduleNext(durationMs + nextDelay());
      return;
    }

    scheduleNext(nextDelay());
  }

  function start() {
    if (running) return;
    running = true;
    resetIdleClock();
    scheduleNext(QUIET_START_MS);
  }

  function stop() {
    running = false;
    clearTimer(timer);
  }

  function pause(durationMs = 0) {
    pausedUntil = Math.max(pausedUntil, now() + durationMs);
    scheduleNext(durationMs);
  }

  function resetToBaseline() {
    resetIdleClock();
    resetMotion?.();
  }

  function handleAttention() {
    if (!running) return false;
    if (isUserActive?.()) return false;
    if (now() < attentionCooldownUntil) return false;

    const currentState = getCurrentState?.() ?? BASELINE_STATE;
    const state = attentionStateFor(currentState);
    if (!state) return false;

    const durationMs = displayMsFor(state);
    const applied = applyMotion?.({
      state,
      reason: "hover-attention",
      priority: HOVER_PRIORITY,
      durationMs,
      sequence: state === "waking" ? [BASELINE_STATE] : [],
    });
    if (!applied) return false;

    attentionCooldownUntil = now() + HOVER_COOLDOWN_MS;
    resetIdleClock();
    pause(HOVER_IDLE_PAUSE_MS);
    return true;
  }

  return {
    start,
    stop,
    pause,
    resetIdleClock,
    resetToBaseline,
    handleAttention,
  };
}
