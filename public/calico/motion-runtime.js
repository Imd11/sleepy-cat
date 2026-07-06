const REPLAY_SLOT_COUNT = 2;

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

  function replaySourceFor(entry) {
    replayCounter = (replayCounter + 1) % REPLAY_SLOT_COUNT;
    return `${entry.file}?replay=${replayCounter}`;
  }

  function setImageSource(entry) {
    if (!entry?.file) return;
    if (entry.replay) {
      image.setAttribute("src", replaySourceFor(entry));
      return;
    }
    image.setAttribute("src", entry.file);
  }

  function applyRenderMetadata(entry) {
    image.style.setProperty("--calico-scale", String(entry.scale ?? 1));
    image.style.setProperty("--calico-offset-x", `${entry.offsetX ?? 0}px`);
    image.style.setProperty("--calico-offset-y", `${entry.offsetY ?? 0}px`);
  }

  function defaultEntry() {
    return entryFor(manifest.defaultState);
  }

  function resetToDefaultAfterError() {
    const defaultState = manifest.defaultState;
    const entry = defaultEntry();
    if (!entry?.file) return;
    window.clearTimeout(autoReturnTimer);
    currentPriority = 0;
    minUntil = 0;
    host.dataset.motionState = defaultState;
    image.setAttribute("src", entry.file);
    applyRenderMetadata(entry);
  }

  image.addEventListener?.("error", () => {
    if (host.dataset.motionState === manifest.defaultState) return;
    resetToDefaultAfterError();
  });

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
