import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  getFrontmostApp,
  getCurrentInputTarget,
  showPromptButton,
  hidePromptButton,
  hidePromptPopover,
} from "../platform/platformApi";
import type { FrontmostApp } from "../platform/platformApi";

interface InputTarget {
  frame: { x: number; y: number; width: number; height: number };
  window_frame: { x: number; y: number; width: number; height: number };
  button_position: [number, number];
  click_point?: [number, number];
  app: FrontmostApp | null;
}

type PromptButtonPosition = {
  x: number;
  y: number;
};

type OverlayPlacement = {
  buttonOffset?: { x: number; y: number } | null;
  buttonPosition?: { x: number; y: number } | null;
};

interface InputTargetPollingOptions {
  onBasePositionChange?: (position: [number, number]) => void;
  onButtonDragEnd?: (
    position: PromptButtonPosition,
    basePosition: [number, number] | null
  ) => void;
}

const DEFAULT_BUTTON_POSITION: [number, number] = [960, 700];

function savedButtonPosition(
  overlayPlacement: OverlayPlacement
): [number, number] | null {
  const position = overlayPlacement.buttonPosition;
  if (!position) return null;
  if (!Number.isFinite(position.x) || !Number.isFinite(position.y)) return null;
  return [position.x, position.y];
}

export function useInputTargetPolling(
  blacklist: string[] = [],
  overlayPlacement: OverlayPlacement = { buttonOffset: null, buttonPosition: null },
  options: InputTargetPollingOptions = {},
  floatingButtonVisible = true
) {
  const [target, setTarget] = useState<InputTarget | null>(null);
  const [showAttached, setShowAttached] = useState(false);
  const lastTargetAppRef = useRef<FrontmostApp | null>(null);
  const lastButtonPositionRef = useRef<[number, number] | null>(DEFAULT_BUTTON_POSITION);
  const lastTargetAtRef = useRef(0);
  const currentBasePositionRef = useRef<[number, number] | null>(null);
  const draggingRef = useRef(false);
  const autosendPausedRef = useRef(false);
  const timeoutRef = useRef<number | null>(null);
  const generationRef = useRef(0);

  useEffect(() => {
    let active = true;
    let unlistenDragStarted: (() => void) | undefined;
    let unlistenDragEnded: (() => void) | undefined;
    let unlistenAutosendActivity: (() => void) | undefined;

    listen("prompt-button-drag-started", () => {
      draggingRef.current = true;
    })
      .then((unlisten) => {
        if (active) {
          unlistenDragStarted = unlisten;
        } else {
          unlisten();
        }
      })
      .catch(() => {});

    listen<PromptButtonPosition>("prompt-button-drag-ended", (event) => {
      draggingRef.current = false;
      const position = event.payload;
      if (
        !position ||
        !Number.isFinite(position.x) ||
        !Number.isFinite(position.y)
      ) {
        return;
      }
      lastButtonPositionRef.current = [position.x, position.y];
      lastTargetAtRef.current = Date.now();
      options.onButtonDragEnd?.(position, currentBasePositionRef.current);
    })
      .then((unlisten) => {
        if (active) {
          unlistenDragEnded = unlisten;
        } else {
          unlisten();
        }
      })
      .catch(() => {});

    listen<{ active?: boolean }>("prompt-autosend-activity", (event) => {
      autosendPausedRef.current = event.payload?.active === true;
      if (!autosendPausedRef.current) {
        lastTargetAtRef.current = Date.now();
      }
    })
      .then((unlisten) => {
        if (active) {
          unlistenAutosendActivity = unlisten;
        } else {
          unlisten();
        }
      })
      .catch(() => {});

    return () => {
      active = false;
      unlistenDragStarted?.();
      unlistenDragEnded?.();
      unlistenAutosendActivity?.();
    };
  }, [options.onButtonDragEnd]);

  useEffect(() => {
    const generation = generationRef.current + 1;
    generationRef.current = generation;
    let cancelled = false;

    const isCurrent = () => !cancelled && generationRef.current === generation;

    const clearScheduledPoll = () => {
      if (timeoutRef.current) {
        window.clearTimeout(timeoutRef.current);
        timeoutRef.current = null;
      }
    };

    const schedulePoll = (delay: number) => {
      if (!isCurrent()) return;
      clearScheduledPoll();
      timeoutRef.current = window.setTimeout(() => {
        timeoutRef.current = null;
        if (!isCurrent()) return;
        void poll();
      }, delay);
    };

    const poll = async () => {
      if (!isCurrent()) return;

      if (!floatingButtonVisible) {
        setTarget(null);
        setShowAttached(false);
        await hidePromptButton();
        await hidePromptPopover();
        return;
      }

      if (draggingRef.current) {
        schedulePoll(250);
        return;
      }

      if (autosendPausedRef.current) {
        schedulePoll(500);
        return;
      }

      try {
        const fixedPosition = savedButtonPosition(overlayPlacement);
        if (fixedPosition) {
          lastButtonPositionRef.current = fixedPosition;
        }
        const displayPosition = lastButtonPositionRef.current ?? DEFAULT_BUTTON_POSITION;
        const app = await getFrontmostApp();
        if (!isCurrent() || autosendPausedRef.current) return;

        const inputTarget = (await getCurrentInputTarget()) as InputTarget | null;
        if (!isCurrent() || autosendPausedRef.current) return;

        if (inputTarget && app) {
          setTarget(inputTarget);
          setShowAttached(false);
          lastTargetAppRef.current = inputTarget.app;
          const [x, y] = inputTarget.button_position;
          currentBasePositionRef.current = [x, y];
          lastTargetAtRef.current = Date.now();
          options.onBasePositionChange?.(displayPosition);
          await showPromptButton(displayPosition[0], displayPosition[1]);
          schedulePoll(1000 + Math.random() * 1000);
          return;
        } else if (
          app &&
          app.name === "Prompt Picker" &&
          lastTargetAtRef.current > 0 &&
          lastButtonPositionRef.current
        ) {
          // Grace period during overlay self-interaction
          const recentlyHadTarget = Date.now() - lastTargetAtRef.current < 3000;
          if (recentlyHadTarget) {
            const [x, y] = displayPosition;
            await showPromptButton(x, y);
            schedulePoll(500 + Math.random() * 500);
            return;
          }
        }

        // No input target and no recent target — show at default/fallback position
        // This ensures the button appears even on first run before any target is detected
        currentBasePositionRef.current = null;
        if (floatingButtonVisible && lastButtonPositionRef.current) {
          const [x, y] = displayPosition;
          await showPromptButton(x, y);
          setShowAttached(false);
        }

        schedulePoll(1000 + Math.random() * 1000);
      } catch {
        schedulePoll(2000);
      }
    };

    if (!floatingButtonVisible) {
      hidePromptButton().catch(console.error);
      hidePromptPopover().catch(console.error);
      return;
    }

    poll();

    return () => {
      cancelled = true;
      clearScheduledPoll();
    };
  }, [
    blacklist.join("\u0000"),
    overlayPlacement.buttonPosition
      ? `${overlayPlacement.buttonPosition.x}:${overlayPlacement.buttonPosition.y}`
      : "none",
    floatingButtonVisible,
  ]);

  return { target, showAttached };
}
