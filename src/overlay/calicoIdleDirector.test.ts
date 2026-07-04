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
    const states = IDLE_MOTION_TIERS.flatMap((tier) => tier.states);

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
      applyMotion: (payload) => {
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
      applyMotion: (payload) => {
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
      applyMotion: (payload) => {
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

  it("restarts idle tiers from light motions after user activity resets the clock", async () => {
    const { createCalicoIdleDirector } = await loadDirectorModule();
    const applied: Array<{ state: string; priority?: number; reason?: string }> = [];
    const timers: Array<{ id: number; at: number; callback: () => void; cancelled: boolean }> = [];
    let now = 0;
    let timerId = 1;

    const setScheduledTimeout = ((callback: TimerHandler, delay?: number) => {
      const id = timerId++;
      const callbackFn = typeof callback === "function" ? callback : () => undefined;
      timers.push({
        id,
        at: now + Number(delay ?? 0),
        callback: callbackFn as () => void,
        cancelled: false,
      });
      return id;
    }) as typeof window.setTimeout;

    const clearScheduledTimeout = ((id?: number) => {
      const timer = timers.find((item) => item.id === id);
      if (timer) timer.cancelled = true;
    }) as typeof window.clearTimeout;

    function runNextTimer() {
      const timer = timers
        .filter((item) => !item.cancelled)
        .sort((first, second) => first.at - second.at)[0];
      if (!timer) throw new Error("Expected a scheduled idle timer");
      timer.cancelled = true;
      now = timer.at;
      timer.callback();
    }

    const director = createCalicoIdleDirector({
      applyMotion: (payload) => {
        applied.push(payload);
        return true;
      },
      resetMotion: vi.fn(),
      getCurrentState: () => "idle-follow",
      isUserActive: () => false,
      random: () => 0.95,
      setTimeout: setScheduledTimeout,
      clearTimeout: clearScheduledTimeout,
      now: () => now,
    });

    director.start();
    now = 70_000;
    director.resetIdleClock();
    director.pause(6_000);

    runNextTimer();
    expect(applied).toEqual([]);

    runNextTimer();
    expect(applied[0]).toMatchObject({
      state: "mini-peek",
      reason: "idle-director",
      priority: 1,
    });
  });

  it("does not treat a rejected idle flourish as played", async () => {
    vi.useFakeTimers();
    const { createCalicoIdleDirector } = await loadDirectorModule();
    const applied: Array<{ state: string }> = [];
    let allowApply = false;

    const director = createCalicoIdleDirector({
      applyMotion: (payload) => {
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
