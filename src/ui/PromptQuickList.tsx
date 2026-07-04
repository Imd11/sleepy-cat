import { useEffect, useRef, useState } from "react";
import type { PromptContainer } from "../shared/promptTypes";
import {
  getPromptContainerBodies,
  getPromptContainerPreviewLines,
} from "../shared/promptTypes";
import type { Messages } from "../shared/i18n";

interface PromptQuickListProps {
  prompts: PromptContainer[];
  messages: Messages["quickList"];
  groupMeta: Messages["manager"]["groupMeta"];
  onSelect: (prompt: PromptContainer) => void;
  submittingPromptId?: string | null;
  hoverResetKey?: number;
}

type HoverPreviewState = {
  promptId: string;
  left: number;
  top: number;
  width: number;
  placement: "above" | "below";
};

type HoverPreviewAnchor = {
  prompt: PromptContainer;
  target: HTMLElement;
};

const HOVER_PREVIEW_MIN_USEFUL_SPACE = 120;
const HOVER_PREVIEW_GAP = 8;
const HOVER_PREVIEW_MARGIN = 10;
const HOVER_PREVIEW_DELAY_MS = 1500;
function clamp(value: number, min: number, max: number): number {
  if (max < min) return min;
  return Math.min(Math.max(value, min), max);
}

export function PromptQuickList({
  prompts,
  messages,
  groupMeta,
  onSelect,
  submittingPromptId = null,
  hoverResetKey = 0,
}: PromptQuickListProps) {
  const [hoverPreview, setHoverPreview] = useState<HoverPreviewState | null>(null);
  const hoverPreviewTimerRef = useRef<number | null>(null);
  const hoverPreviewAnchorRef = useRef<HoverPreviewAnchor | null>(null);
  const hoveredPrompt = prompts.find((prompt) => prompt.id === hoverPreview?.promptId) ?? null;

  useEffect(() => {
    return () => {
      clearHoverPreviewTimer();
    };
  }, []);

  useEffect(() => {
    hideHoverPreview();
  }, [hoverResetKey]);

  function clearHoverPreviewTimer() {
    if (hoverPreviewTimerRef.current !== null) {
      window.clearTimeout(hoverPreviewTimerRef.current);
      hoverPreviewTimerRef.current = null;
    }
  }

  function createHoverPreviewState(
    prompt: PromptContainer,
    target: HTMLElement
  ): HoverPreviewState {
    const shell = target.closest(".prompt-quick-shell") as HTMLElement | null;
    const shellRect = shell?.getBoundingClientRect();
    const targetRect = target.getBoundingClientRect();
    const shellWidth = Math.max(
      shellRect?.width ?? 0,
      shell?.clientWidth ?? 0,
      targetRect.width + (HOVER_PREVIEW_MARGIN * 2)
    );
    const shellHeight = Math.max(
      shellRect?.height ?? 0,
      shell?.clientHeight ?? 0,
      320
    );
    const localLeft = shellRect ? targetRect.left - shellRect.left : target.offsetLeft;
    const localTop = shellRect ? targetRect.top - shellRect.top : target.offsetTop;
    const width = Math.min(
      targetRect.width,
      shellWidth - (HOVER_PREVIEW_MARGIN * 2)
    );
    const maxLeft = Math.max(
      HOVER_PREVIEW_MARGIN,
      shellWidth - width - HOVER_PREVIEW_MARGIN
    );
    const targetBottom = localTop + targetRect.height;
    const availableBelow = shellHeight - targetBottom - HOVER_PREVIEW_GAP - HOVER_PREVIEW_MARGIN;
    const availableAbove = localTop - HOVER_PREVIEW_GAP - HOVER_PREVIEW_MARGIN;
    const placement =
      availableAbove >= HOVER_PREVIEW_MIN_USEFUL_SPACE || availableAbove >= availableBelow
        ? "above"
        : "below";
    const top = placement === "above"
      ? Math.max(HOVER_PREVIEW_MARGIN, localTop - HOVER_PREVIEW_GAP)
      : Math.min(
        targetBottom + HOVER_PREVIEW_GAP,
        Math.max(HOVER_PREVIEW_MARGIN, shellHeight - HOVER_PREVIEW_MARGIN)
      );

    return {
      promptId: prompt.id,
      left: clamp(localLeft, HOVER_PREVIEW_MARGIN, maxLeft),
      top,
      width,
      placement,
    };
  }

  function scheduleHoverPreview(
    prompt: PromptContainer,
    target: HTMLElement
  ) {
    clearHoverPreviewTimer();
    hoverPreviewAnchorRef.current = {
      prompt,
      target,
    };
    hoverPreviewTimerRef.current = window.setTimeout(() => {
      hoverPreviewTimerRef.current = null;
      const anchor = hoverPreviewAnchorRef.current;
      if (!anchor || !document.contains(anchor.target)) return;
      setHoverPreview(createHoverPreviewState(
        anchor.prompt,
        anchor.target
      ));
    }, HOVER_PREVIEW_DELAY_MS);
  }

  function hideHoverPreview() {
    clearHoverPreviewTimer();
    hoverPreviewAnchorRef.current = null;
    setHoverPreview(null);
  }

  function selectPrompt(prompt: PromptContainer) {
    hideHoverPreview();
    onSelect(prompt);
  }

  return (
    <div className="prompt-quick-shell">
      <div
        className="prompt-quick-list"
        role="listbox"
        aria-label={messages.ariaLabel}
        onScroll={hideHoverPreview}
      >
        {prompts.length === 0 ? (
          <div className="prompt-quick-empty">
            <strong>{messages.noPromptsTitle}</strong>
            <span>{messages.noPromptsDescription}</span>
          </div>
        ) : (
          prompts.map((prompt) => (
            <button
              key={prompt.id}
              className={`prompt-quick-item ${
                prompt.type === "group" ? "prompt-quick-item-group" : ""
              }`}
              type="button"
              role="option"
              aria-selected="false"
              disabled={submittingPromptId === prompt.id}
              onMouseEnter={(event) => scheduleHoverPreview(prompt, event.currentTarget)}
              onMouseLeave={hideHoverPreview}
              onBlur={hideHoverPreview}
              onClick={() => selectPrompt(prompt)}
            >
              <span className="prompt-quick-title-row">
                <span className="prompt-quick-title">{prompt.title}</span>
                {prompt.type === "group" ? (
                  <span className="prompt-quick-meta">
                    {groupMeta(getPromptContainerBodies(prompt).length, prompt.intervalMs)}
                  </span>
                ) : null}
              </span>
              <span className="prompt-quick-preview-lines">
                {getPromptContainerPreviewLines(prompt).map((line) => (
                  <span className="prompt-quick-preview-line" key={line}>
                    {line}
                  </span>
                ))}
              </span>
            </button>
          ))
        )}
      </div>
      {hoveredPrompt && hoverPreview ? (
        <PromptHoverPreview
          prompt={hoveredPrompt}
          left={hoverPreview.left}
          top={hoverPreview.top}
          width={hoverPreview.width}
          placement={hoverPreview.placement}
        />
      ) : null}
    </div>
  );
}

function PromptHoverPreview({
  prompt,
  left,
  top,
  width,
  placement,
}: {
  prompt: PromptContainer;
  left: number;
  top: number;
  width: number;
  placement: "above" | "below";
}) {
  const bodies = getPromptContainerBodies(prompt);

  return (
    <aside
      className={`prompt-hover-preview prompt-hover-preview-floating ${
        placement === "above" ? "is-above" : "is-below"
      }`}
      role="tooltip"
      style={{ left, top, width }}
    >
      <div className="prompt-hover-preview-body">
        {prompt.type === "group" ? (
          bodies.map((body, index) => (
            <p key={`${index}-${body}`}>
              {index + 1}. {body}
            </p>
          ))
        ) : (
          <p>{bodies[0] ?? ""}</p>
        )}
      </div>
    </aside>
  );
}
