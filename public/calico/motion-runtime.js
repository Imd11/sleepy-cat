export function createCalicoMotionRuntime({
  renderer,
  host,
  manifest,
  sheetManifest,
  now = () => Date.now(),
}) {
  let currentPriority = 0;
  let minUntil = 0;
  let completionProtectedUntil = 0;
  let queuedAfterCompletion = null;
  let autoReturnTimer = 0;
  let disposed = false;

  function stateFor(requestedState) {
    if (requestedState && manifest.states[requestedState]) return requestedState;
    return manifest.defaultState;
  }

  function entryFor(state) {
    return manifest.states[state] || manifest.states[manifest.defaultState];
  }

  function render(state, entry) {
    renderer.setPresentation(entry);
    const sheet = sheetManifest.states[state];
    const operation = sheet
      ? renderer.play(state, sheet, { restart: entry.replay === true })
      : renderer.showBaseline(entry);
    Promise.resolve(operation).catch((error) => {
      console.error(`Failed to render Calico motion: ${state}`, error);
    });
  }

  function completionIsProtected() {
    return now() < completionProtectedUntil;
  }

  function queueAfterProtectedCompletion(payload, state, priority) {
    const candidateIsBaseline = state === manifest.defaultState;
    const queuedIsBaseline = queuedAfterCompletion?.state === manifest.defaultState;
    const shouldReplace = !queuedAfterCompletion
      || (queuedIsBaseline && !candidateIsBaseline)
      || (queuedIsBaseline === candidateIsBaseline && priority >= queuedAfterCompletion.priority);
    if (shouldReplace) {
      queuedAfterCompletion = { payload: { ...payload, state }, state, priority };
    }
    return true;
  }

  function transitionAfter(durationMs, sequence, priority, reason) {
    if (durationMs <= 0) return;
    autoReturnTimer = window.setTimeout(() => {
      completionProtectedUntil = 0;
      const queued = queuedAfterCompletion;
      queuedAfterCompletion = null;
      if (queued) {
        apply({ ...queued.payload, force: true });
        return;
      }
      const [nextState, ...remaining] = sequence;
      if (nextState) {
        apply({
          state: nextState,
          priority,
          reason,
          sequence: remaining,
          force: true,
        });
        return;
      }
      reset();
    }, durationMs);
  }

  function apply(payload = {}) {
    if (disposed) return false;
    const state = stateFor(payload.state);
    const entry = entryFor(state);
    if (!entry) return false;
    if (!sheetManifest.states[state] && !entry.file) return false;

    const priority = Number.isFinite(payload.priority) ? payload.priority : entry.priority;
    if (!payload.force && !payload.interruptProtected && completionIsProtected()) {
      return queueAfterProtectedCompletion(payload, state, priority);
    }
    if (!payload.force && now() < minUntil && priority < currentPriority) return false;

    window.clearTimeout(autoReturnTimer);
    autoReturnTimer = 0;
    completionProtectedUntil = 0;
    queuedAfterCompletion = null;
    currentPriority = priority;
    minUntil = now() + (entry.minMs || 0);
    host.dataset.motionState = state;
    render(state, entry);

    const durationMs = payload.durationMs ?? entry.durationMs;
    const sequence = Array.isArray(payload.sequence)
      ? payload.sequence.filter((candidate) => manifest.states[candidate])
      : [];
    if (entry.completeBeforeTransition === true && durationMs > 0) {
      completionProtectedUntil = now() + durationMs;
    }
    transitionAfter(durationMs, sequence, priority, payload.reason);
    return true;
  }

  function reset() {
    return apply({ state: manifest.defaultState, priority: 0, force: true });
  }

  function requestReset() {
    if (completionIsProtected()) {
      return queueAfterProtectedCompletion({}, manifest.defaultState, 0);
    }
    return reset();
  }

  function suspend(options = { retainFrame: true }) {
    window.clearTimeout(autoReturnTimer);
    autoReturnTimer = 0;
    renderer.suspend(options);
  }

  function resume() {
    return renderer.resume();
  }

  function dispose() {
    if (disposed) return;
    disposed = true;
    window.clearTimeout(autoReturnTimer);
    autoReturnTimer = 0;
    completionProtectedUntil = 0;
    queuedAfterCompletion = null;
    renderer.dispose();
  }

  return { apply, reset, requestReset, suspend, resume, dispose };
}
