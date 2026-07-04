import { describe, expect, it, vi } from "vitest";

type Runtime = {
  apply(payload?: Record<string, unknown>): boolean;
  reset(): boolean;
};

type RuntimeModule = {
  createCalicoMotionRuntime(options: {
    image: HTMLImageElement;
    host: HTMLElement;
    manifest: unknown;
    now?: () => number;
  }): Runtime;
};

const manifest = {
  defaultState: "idle-follow",
  states: {
    "idle-follow": {
      file: "/calico/calico-idle-follow.svg",
      priority: 0,
      scale: 1,
      offsetX: 0,
      offsetY: 0,
    },
    happy: {
      file: "/calico/calico-happy.apng",
      priority: 50,
      durationMs: 3000,
      minMs: 800,
      replay: true,
      scale: 1.2,
      offsetX: 8,
      offsetY: 6,
    },
    error: {
      file: "/calico/calico-error.apng",
      priority: 90,
      minMs: 5000,
      scale: 1.25,
      offsetX: 0,
      offsetY: 7,
    },
    "working-typing": {
      file: "/calico/calico-working-typing.apng",
      priority: 65,
      scale: 1.2,
      offsetX: -3,
      offsetY: -5,
    },
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
  },
};

async function loadRuntime() {
  // @ts-expect-error public overlay runtime is intentionally outside the src build graph.
  return (await import("../../public/calico/motion-runtime.js")) as RuntimeModule;
}

function elements() {
  return {
    image: document.createElement("img"),
    host: document.createElement("button"),
  };
}

describe("Calico motion runtime", () => {
  it("applies file, state, scale, and offsets", async () => {
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { image, host } = elements();
    const runtime = createCalicoMotionRuntime({ image, host, manifest });

    runtime.apply({ state: "working-typing" });

    expect(host.dataset.motionState).toBe("working-typing");
    expect(image.getAttribute("src")).toBe("/calico/calico-working-typing.apng");
    expect(image.style.getPropertyValue("--calico-scale")).toBe("1.2");
    expect(image.style.getPropertyValue("--calico-offset-x")).toBe("-3px");
    expect(image.style.getPropertyValue("--calico-offset-y")).toBe("-5px");
  });

  it("does not allow lower priority motion to interrupt minimum display time", async () => {
    vi.useFakeTimers();
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { image, host } = elements();
    const runtime = createCalicoMotionRuntime({ image, host, manifest, now: () => Date.now() });

    runtime.apply({ state: "error" });
    runtime.apply({ state: "happy" });

    expect(host.dataset.motionState).toBe("error");
    vi.useRealTimers();
  });

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

  it("replays one-shot animations by replacing the image src", async () => {
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { image, host } = elements();
    const runtime = createCalicoMotionRuntime({ image, host, manifest });

    runtime.apply({ state: "happy" });
    const firstSrc = image.getAttribute("src");
    runtime.apply({ state: "happy" });

    expect(image.getAttribute("src")).not.toBe(firstSrc);
  });

  it("resets to the manifest default state even during a minimum display window", async () => {
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { image, host } = elements();
    const runtime = createCalicoMotionRuntime({ image, host, manifest });

    runtime.apply({ state: "error" });
    runtime.reset();

    expect(host.dataset.motionState).toBe("idle-follow");
    expect(image.getAttribute("src")).toBe("/calico/calico-idle-follow.svg");
  });
});
