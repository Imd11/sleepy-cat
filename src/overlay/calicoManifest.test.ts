import { existsSync, readFileSync } from "fs";
import { describe, expect, it } from "vitest";

type CalicoState = {
  file: string;
  priority: number;
  durationMs: number;
  minMs: number;
  replay: boolean;
  scale: number;
  offsetX: number;
  offsetY: number;
};

type CalicoManifest = {
  schemaVersion: number;
  assetSource: string;
  defaultState: string;
  phase1States: string[];
  reservedStates: string[];
  states: Record<string, CalicoState>;
};

type IdleDirectorModule = {
  IDLE_MOTION_POOL: Array<{ state: string; weights: Record<string, number> }>;
};

const phase1States = [
  "idle-follow",
  "idle",
  "thinking",
  "working-typing",
  "working-conducting",
  "working-juggling",
  "working-building",
  "working-carrying",
  "working-sweeping",
  "notification",
  "error",
  "happy",
  "react-drag",
];

const reservedStates = [
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
];

const calicoNativeWindowSize = 288;
const calicoHitAreaSize = 132;
const calicoSpriteSize = 126;

function readManifest(): CalicoManifest {
  return JSON.parse(readFileSync("public/calico/manifest.json", "utf8"));
}

async function loadIdleDirector() {
  // @ts-expect-error public overlay module is intentionally outside the src build graph.
  return (await import("../../public/calico/idle-director.js")) as IdleDirectorModule;
}

describe("Calico manifest", () => {
  it("declares the Phase 1 motion states separately from later risky states", () => {
    const manifest = readManifest();

    expect(manifest.schemaVersion).toBe(1);
    expect(manifest.assetSource).toBe("authorized upstream");
    expect(manifest.defaultState).toBe("idle-follow");
    expect(manifest.phase1States).toEqual(phase1States);
    expect(manifest.reservedStates).toEqual(reservedStates);
    expect(Object.keys(manifest.states).sort()).toEqual(
      [...phase1States, ...reservedStates].sort()
    );
  });

  it("ships every declared Calico state with rendering metadata", () => {
    const manifest = readManifest();

    for (const [stateName, state] of Object.entries(manifest.states)) {
      expect(state.file, stateName).toMatch(/^\/calico\/calico-.+\.(apng|svg)$/);
      expect(existsSync(`public${state.file}`), stateName).toBe(true);
      expect(typeof state.priority, stateName).toBe("number");
      expect(typeof state.durationMs, stateName).toBe("number");
      expect(typeof state.minMs, stateName).toBe("number");
      expect(typeof state.replay, stateName).toBe("boolean");
      expect(typeof state.scale, stateName).toBe("number");
      expect(typeof state.offsetX, stateName).toBe("number");
      expect(typeof state.offsetY, stateName).toBe("number");
    }
  });

  it("does not reintroduce paper-plane assets", () => {
    const manifest = readManifest();
    const files = Object.values(manifest.states).map((state) => state.file);

    expect(files).not.toContain("/calico/paper-plane.svg");
    expect(existsSync("public/calico/paper-plane.svg")).toBe(false);
  });

  it("ships every idle director state and hover response asset", async () => {
    const manifest = readManifest();
    const { IDLE_MOTION_POOL } = await loadIdleDirector();

    for (const { state } of IDLE_MOTION_POOL) {
      expect(manifest.states[state], state).toBeDefined();
      expect(existsSync(`public${manifest.states[state].file}`), state).toBe(true);
    }
    expect(manifest.states.waking).toBeDefined();
    expect(manifest.states["mini-happy"]).toBeDefined();
    expect(existsSync(`public${manifest.states.waking.file}`)).toBe(true);
    expect(existsSync(`public${manifest.states["mini-happy"].file}`)).toBe(true);
  });

  it("keeps deep idle states on non-replay assets", async () => {
    const manifest = readManifest();
    const { IDLE_MOTION_POOL } = await loadIdleDirector();
    const deepIdleStates = IDLE_MOTION_POOL.filter((entry) => (entry.weights.deepIdle ?? 0) > 0)
      .map((entry) => entry.state);

    expect(deepIdleStates.length).toBeGreaterThan(0);
    for (const stateName of deepIdleStates) {
      expect(manifest.states[stateName].replay, stateName).toBe(false);
    }
  });

  it("keeps every rendered Calico motion inside the native transparent window", () => {
    const manifest = readManifest();
    const center = calicoNativeWindowSize / 2;

    for (const [stateName, state] of Object.entries(manifest.states)) {
      const renderedSize = calicoSpriteSize * state.scale;
      const left = center - renderedSize / 2 + state.offsetX;
      const top = center - renderedSize / 2 + state.offsetY;
      const right = left + renderedSize;
      const bottom = top + renderedSize;

      expect(left, `${stateName} left edge`).toBeGreaterThanOrEqual(0);
      expect(top, `${stateName} top edge`).toBeGreaterThanOrEqual(0);
      expect(right, `${stateName} right edge`).toBeLessThanOrEqual(calicoNativeWindowSize);
      expect(bottom, `${stateName} bottom edge`).toBeLessThanOrEqual(calicoNativeWindowSize);
    }

    expect(calicoHitAreaSize).toBe(132);
  });
});
