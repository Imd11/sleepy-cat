export function createCalicoMotionRuntime({ image, host, manifest, now = () => Date.now() }) {
  let currentPriority = 0;
  let minUntil = 0;
  let autoReturnTimer = 0;
  let replayCounter = 0;

  function stateFor(requestedState) {
    if (requestedState && manifest.states[requestedState]) return requestedState;
    return manifest.defaultState;
  }

  function entryFor(state) {
    return manifest.states[state] || manifest.states[manifest.defaultState];
  }

  function setImageSource(entry) {
    if (!entry?.file) return;
    if (entry.replay) {
      replayCounter += 1;
      image.setAttribute("src", `${entry.file}?replay=${replayCounter}`);
      return;
    }
    image.setAttribute("src", entry.file);
  }

  function applyRenderMetadata(entry) {
    image.style.setProperty("--calico-scale", String(entry.scale ?? 1));
    image.style.setProperty("--calico-offset-x", `${entry.offsetX ?? 0}px`);
    image.style.setProperty("--calico-offset-y", `${entry.offsetY ?? 0}px`);
  }

  function apply(payload = {}) {
    const state = stateFor(payload.state);
    const entry = entryFor(state);
    if (!entry?.file) return false;

    const priority = Number.isFinite(payload.priority) ? payload.priority : entry.priority;
    if (!payload.force && now() < minUntil && priority < currentPriority) return false;

    window.clearTimeout(autoReturnTimer);
    currentPriority = priority;
    minUntil = now() + (entry.minMs || 0);
    host.dataset.motionState = state;
    setImageSource(entry);
    applyRenderMetadata(entry);

    const durationMs = payload.durationMs ?? entry.durationMs;
    if (durationMs > 0) {
      autoReturnTimer = window.setTimeout(reset, durationMs);
    }
    return true;
  }

  function reset() {
    return apply({ state: manifest.defaultState, priority: 0, force: true });
  }

  return { apply, reset };
}
