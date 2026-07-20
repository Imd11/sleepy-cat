import { afterEach, describe, expect, it, vi } from "vitest";

const manifest = {
  defaultState: "idle",
  states: {
    "idle-follow": {
      file: "/calico/calico-idle-follow.svg",
      priority: 0,
      durationMs: 0,
      minMs: 0,
      replay: false,
      scale: 1,
      offsetX: 0,
      offsetY: 0,
    },
    idle: {
      priority: 0,
      durationMs: 0,
      minMs: 0,
      replay: false,
      scale: 1,
      offsetX: 0,
      offsetY: 0,
    },
    waking: {
      priority: 10,
      durationMs: 100,
      minMs: 0,
      replay: true,
      completeBeforeTransition: true,
      scale: 1,
      offsetX: 0,
      offsetY: 0,
    },
    happy: {
      priority: 50,
      durationMs: 3000,
      minMs: 800,
      replay: true,
      scale: 1.2,
      offsetX: 8,
      offsetY: 6,
    },
    error: {
      priority: 90,
      durationMs: 5000,
      minMs: 5000,
      replay: true,
      scale: 1.25,
      offsetX: 0,
      offsetY: 7,
    },
    "react-drag": {
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

const sheetManifest = {
  states: {
    idle: { file: "/calico/sheets/idle.png", plays: 0 },
    waking: { file: "/calico/sheets/waking.png", plays: 1 },
    happy: { file: "/calico/sheets/happy.png", plays: 1 },
    error: { file: "/calico/sheets/error.png", plays: 1 },
    "react-drag": { file: "/calico/sheets/react-drag.png", plays: 0 },
  },
};

async function loadRuntime() {
  // @ts-expect-error public overlay runtime is intentionally outside the src build graph.
  return import("../../public/calico/motion-runtime.js");
}

function setup(now = () => Date.now()) {
  const renderer = {
    play: vi.fn().mockResolvedValue(true),
    showBaseline: vi.fn().mockResolvedValue(true),
    setPresentation: vi.fn(),
    suspend: vi.fn(),
    resume: vi.fn().mockResolvedValue(true),
    dispose: vi.fn(),
  };
  return {
    renderer,
    host: document.createElement("button"),
    options: { renderer, host: document.createElement("button"), manifest, sheetManifest, now },
  };
}

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("Calico motion runtime", () => {
  it("routes generated states through the renderer with one presentation owner", async () => {
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { options, renderer } = setup();
    const runtime = createCalicoMotionRuntime(options);

    expect(runtime.apply({ state: "happy" })).toBe(true);

    expect(options.host.dataset.motionState).toBe("happy");
    expect(renderer.setPresentation).toHaveBeenCalledWith(manifest.states.happy);
    expect(renderer.play).toHaveBeenCalledWith("happy", sheetManifest.states.happy, {
      restart: true,
    });
    expect(renderer.showBaseline).not.toHaveBeenCalled();
  });

  it("renders the default state through the neutral idle sheet", async () => {
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { options, renderer } = setup();
    const runtime = createCalicoMotionRuntime(options);

    expect(runtime.reset()).toBe(true);

    expect(options.host.dataset.motionState).toBe("idle");
    expect(renderer.play).toHaveBeenCalledWith("idle", sheetManifest.states.idle, {
      restart: false,
    });
    expect(renderer.showBaseline).not.toHaveBeenCalled();
  });

  it("preserves priority and minimum-display rules", async () => {
    vi.useFakeTimers();
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { options, renderer } = setup(() => Date.now());
    const runtime = createCalicoMotionRuntime(options);

    expect(runtime.apply({ state: "error" })).toBe(true);
    expect(runtime.apply({ state: "happy" })).toBe(false);

    expect(options.host.dataset.motionState).toBe("error");
    expect(renderer.play).toHaveBeenCalledTimes(1);
  });

  it("force reset bypasses a minimum-display window", async () => {
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { options, renderer } = setup();
    const runtime = createCalicoMotionRuntime(options);

    runtime.apply({ state: "error" });
    expect(runtime.reset()).toBe(true);

    expect(options.host.dataset.motionState).toBe("idle");
    expect(renderer.play).toHaveBeenLastCalledWith("idle", sheetManifest.states.idle, {
      restart: false,
    });
  });

  it("finishes waking before applying a queued result motion", async () => {
    vi.useFakeTimers();
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { options, renderer } = setup(() => Date.now());
    const runtime = createCalicoMotionRuntime(options);

    expect(runtime.apply({ state: "waking" })).toBe(true);
    expect(runtime.requestReset()).toBe(true);
    expect(runtime.apply({ state: "happy" })).toBe(true);

    expect(options.host.dataset.motionState).toBe("waking");
    expect(renderer.play).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(99);
    expect(options.host.dataset.motionState).toBe("waking");

    await vi.advanceTimersByTimeAsync(1);
    expect(options.host.dataset.motionState).toBe("happy");
    expect(renderer.play).toHaveBeenLastCalledWith("happy", sheetManifest.states.happy, {
      restart: true,
    });
  });

  it("allows direct manipulation to interrupt waking", async () => {
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { options, renderer } = setup();
    const runtime = createCalicoMotionRuntime(options);

    runtime.apply({ state: "waking" });
    expect(runtime.apply({ state: "react-drag", interruptProtected: true })).toBe(true);

    expect(options.host.dataset.motionState).toBe("react-drag");
    expect(renderer.play).toHaveBeenLastCalledWith(
      "react-drag",
      sheetManifest.states["react-drag"],
      { restart: false }
    );
  });

  it("auto-return is independent of intrinsic sheet playback", async () => {
    vi.useFakeTimers();
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { options, renderer } = setup(() => Date.now());
    const runtime = createCalicoMotionRuntime(options);

    runtime.apply({ state: "happy", durationMs: 100 });
    await vi.advanceTimersByTimeAsync(100);

    expect(renderer.play).toHaveBeenCalledTimes(2);
    expect(renderer.play).toHaveBeenLastCalledWith("idle", sheetManifest.states.idle, {
      restart: false,
    });
    expect(options.host.dataset.motionState).toBe("idle");
  });

  it("keeps durationless drag active until explicit reset", async () => {
    vi.useFakeTimers();
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { options, renderer } = setup(() => Date.now());
    const runtime = createCalicoMotionRuntime(options);

    runtime.apply({ state: "react-drag" });
    await vi.advanceTimersByTimeAsync(60_000);

    expect(options.host.dataset.motionState).toBe("react-drag");
    expect(renderer.showBaseline).not.toHaveBeenCalled();
  });

  it("plays a compatible sequence before returning to neutral idle", async () => {
    vi.useFakeTimers();
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { options, renderer } = setup(() => Date.now());
    const runtime = createCalicoMotionRuntime(options);

    runtime.apply({ state: "happy", durationMs: 100, sequence: ["error"] });
    await vi.advanceTimersByTimeAsync(100);

    expect(options.host.dataset.motionState).toBe("error");
    expect(renderer.play).toHaveBeenLastCalledWith("error", sheetManifest.states.error, {
      restart: true,
    });
  });

  it("replay metadata changes timing without creating resource URLs", async () => {
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { options, renderer } = setup();
    const runtime = createCalicoMotionRuntime(options);

    runtime.apply({ state: "happy" });
    runtime.apply({ state: "happy" });
    runtime.apply({ state: "react-drag", force: true });
    runtime.apply({ state: "react-drag", force: true });

    expect(renderer.play.mock.calls.slice(0, 2).every((call) => call[2].restart)).toBe(true);
    expect(renderer.play.mock.calls.slice(2).every((call) => !call[2].restart)).toBe(true);
    expect(JSON.stringify(renderer.play.mock.calls)).not.toContain("?replay=");
  });

  it("ignores asynchronous renderer rejection without reverting newer state", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { options, renderer } = setup();
    let rejectOld!: (error: Error) => void;
    renderer.play
      .mockReturnValueOnce(new Promise((_, reject) => { rejectOld = reject; }))
      .mockResolvedValueOnce(true);
    const runtime = createCalicoMotionRuntime(options);

    runtime.apply({ state: "happy" });
    runtime.apply({ state: "react-drag", force: true });
    rejectOld(new Error("stale load"));
    await Promise.resolve();

    expect(options.host.dataset.motionState).toBe("react-drag");
    expect(errorSpy).toHaveBeenCalledTimes(1);
  });

  it("forwards reversible suspension and disposes renderer ownership once", async () => {
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { options, renderer } = setup();
    const runtime = createCalicoMotionRuntime(options);

    runtime.suspend({ retainFrame: true });
    await runtime.resume();
    runtime.dispose();
    runtime.dispose();

    expect(renderer.suspend).toHaveBeenCalledWith({ retainFrame: true });
    expect(renderer.resume).toHaveBeenCalledTimes(1);
    expect(renderer.dispose).toHaveBeenCalledTimes(1);
    expect(runtime.apply({ state: "happy" })).toBe(false);
  });
});
