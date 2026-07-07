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

function activeActionImage(host: HTMLElement): HTMLImageElement | null {
  return host.querySelector<HTMLImageElement>(".calico-action-sprite");
}

function activeSpriteSrc(image: HTMLImageElement, host: HTMLElement): string {
  return activeActionImage(host)?.getAttribute("src") ?? image.getAttribute("src") ?? "";
}

describe("Calico motion runtime", () => {
  it("applies file, state, scale, and offsets", async () => {
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { image, host } = elements();
    const runtime = createCalicoMotionRuntime({ image, host, manifest });

    runtime.apply({ state: "working-typing" });

    expect(host.dataset.motionState).toBe("working-typing");
    expect(activeSpriteSrc(image, host)).toBe("/calico/calico-working-typing.apng");
    expect(activeActionImage(host)?.style.getPropertyValue("--calico-scale")).toBe("1.2");
    expect(activeActionImage(host)?.style.getPropertyValue("--calico-offset-x")).toBe("-3px");
    expect(activeActionImage(host)?.style.getPropertyValue("--calico-offset-y")).toBe("-5px");
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
    expect(activeSpriteSrc(image, host)).toContain("/calico/calico-happy.apng");
    vi.useRealTimers();
  });

  it("replays one-shot animations by replacing the image src", async () => {
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { image, host } = elements();
    const runtime = createCalicoMotionRuntime({ image, host, manifest });

    runtime.apply({ state: "happy" });
    const firstSrc = activeSpriteSrc(image, host);
    runtime.apply({ state: "happy" });

    expect(activeSpriteSrc(image, host)).not.toBe(firstSrc);
  });

  it("keeps replay image URLs bounded during long-running motion loops", async () => {
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { image, host } = elements();
    const runtime = createCalicoMotionRuntime({ image, host, manifest });
    const seenSrcs = new Set<string>();

    for (let index = 0; index < 100; index += 1) {
      runtime.apply({ state: "happy" });
      seenSrcs.add(activeSpriteSrc(image, host));
    }

    expect(seenSrcs.size).toBeLessThanOrEqual(2);
    expect([...seenSrcs].every((src) => src.startsWith("/calico/calico-happy.apng?replay="))).toBe(
      true
    );
  });

  it("keeps replay image URLs bounded across mixed replay states", async () => {
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { image, host } = elements();
    const runtime = createCalicoMotionRuntime({ image, host, manifest });
    const seenByFile = new Map<string, Set<string>>();
    const states = ["happy", "react-left"] as const;

    for (let index = 0; index < 100; index += 1) {
      runtime.apply({ state: states[index % states.length] });
      const src = activeSpriteSrc(image, host);
      const file = src.split("?")[0];
      if (!seenByFile.has(file)) {
        seenByFile.set(file, new Set());
      }
      seenByFile.get(file)?.add(src);
    }

    expect(seenByFile.get("/calico/calico-happy.apng")?.size).toBeLessThanOrEqual(2);
    expect(seenByFile.get("/calico/calico-react-left.apng")?.size).toBeLessThanOrEqual(2);
  });

  it("resets to the manifest default state even during a minimum display window", async () => {
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { image, host } = elements();
    const runtime = createCalicoMotionRuntime({ image, host, manifest });

    runtime.apply({ state: "error" });
    runtime.reset();

    expect(host.dataset.motionState).toBe("idle-follow");
    expect(image.getAttribute("src")).toBe("/calico/calico-idle-follow.svg");
    expect(activeActionImage(host)).toBeNull();
  });

  it("resets to the default state when a replay image fails to load", async () => {
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { image, host } = elements();
    const runtime = createCalicoMotionRuntime({ image, host, manifest });

    runtime.apply({ state: "happy" });
    activeActionImage(host)?.dispatchEvent(new Event("error"));

    expect(host.dataset.motionState).toBe("idle-follow");
    expect(image.getAttribute("src")).toBe("/calico/calico-idle-follow.svg");
    expect(activeActionImage(host)).toBeNull();
  });

  it("does not loop fallback handling when the default image errors", async () => {
    const { createCalicoMotionRuntime } = await loadRuntime();
    const { image, host } = elements();
    const runtime = createCalicoMotionRuntime({ image, host, manifest });

    runtime.reset();
    image.dispatchEvent(new Event("error"));

    expect(host.dataset.motionState).toBe("idle-follow");
    expect(image.getAttribute("src")).toBe("/calico/calico-idle-follow.svg");
  });

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
});
