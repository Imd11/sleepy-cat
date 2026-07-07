const REPLAY_SLOT_COUNT = 2;
const ACTION_SPRITE_CLASS = "calico-action-sprite";

export function createCalicoMotionRuntime({ image, host, manifest, now = () => Date.now() }) {
  let currentPriority = 0;
  let minUntil = 0;
  let autoReturnTimer = 0;
  let replayCounter = 0;
  let actionImage = null;

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

  function applyRenderMetadataTo(target, entry) {
    target.style.setProperty("--calico-scale", String(entry.scale ?? 1));
    target.style.setProperty("--calico-offset-x", `${entry.offsetX ?? 0}px`);
    target.style.setProperty("--calico-offset-y", `${entry.offsetY ?? 0}px`);
  }

  function defaultEntry() {
    return entryFor(manifest.defaultState);
  }

  function releaseActionImage() {
    if (!actionImage) return;
    actionImage.removeAttribute("src");
    actionImage.remove();
    actionImage = null;
    image.hidden = false;
  }

  function resetToDefaultAfterError() {
    const defaultState = manifest.defaultState;
    const entry = defaultEntry();
    if (!entry?.file) return;
    window.clearTimeout(autoReturnTimer);
    currentPriority = 0;
    minUntil = 0;
    host.dataset.motionState = defaultState;
    releaseActionImage();
    image.setAttribute("src", entry.file);
    applyRenderMetadataTo(image, entry);
  }

  function handleActionImageError() {
    resetToDefaultAfterError();
  }

  function createActionImage() {
    releaseActionImage();
    const target = document.createElement("img");
    actionImage = target;
    actionImage.className = `${image.className} ${ACTION_SPRITE_CLASS}`.trim();
    actionImage.alt = "";
    actionImage.draggable = false;
    actionImage.setAttribute("aria-hidden", "true");
    actionImage.addEventListener("error", handleActionImageError, { once: true });
    actionImage.addEventListener(
      "load",
      () => {
        if (actionImage === target && target.isConnected) image.hidden = true;
      },
      { once: true }
    );
    host.appendChild(actionImage);
    return actionImage;
  }

  function setImageSource(state, entry) {
    if (!entry?.file) return;
    if (state === manifest.defaultState) {
      releaseActionImage();
      image.setAttribute("src", entry.file);
      applyRenderMetadataTo(image, entry);
      return;
    }

    const target = createActionImage();
    target.setAttribute("src", entry.replay ? replaySourceFor(entry) : entry.file);
    applyRenderMetadataTo(target, entry);
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
    setImageSource(state, entry);

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
