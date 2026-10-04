import { afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { getMessages } from "../shared/i18n";
import type { PromptContainer } from "../shared/promptTypes";
import { PromptQuickList } from "./PromptQuickList";
import type { NativePopoverPointerPosition } from "./PromptQuickList";

const prompts: PromptContainer[] = [
  {
    id: "1",
    categoryId: "category-default",
    title: "讨论方案",
    type: "single",
    sendBehavior: "inherit",
    prompts: [
      {
        id: "1-entry",
        body: "使用 brainstorming skill，先和我讨论方案，不要修改代码。",
        order: 0,
      },
    ],
    intervalMs: 700,
    order: 0,
    createdAt: "2026-05-26T00:00:00.000Z",
    updatedAt: "2026-05-26T00:00:00.000Z",
  },
  {
    id: "2",
    categoryId: "category-default",
    title: "修复流程",
    type: "group",
    sendBehavior: "inherit",
    prompts: [
      { id: "2-entry-1", body: "分析根本原因。", order: 0 },
      { id: "2-entry-2", body: "执行修复。", order: 1 },
      { id: "2-entry-3", body: "完成验证。", order: 2 },
    ],
    intervalMs: 700,
    order: 1,
    createdAt: "2026-05-26T00:00:00.000Z",
    updatedAt: "2026-05-26T00:00:00.000Z",
  }
];

const zh = getMessages("zh-CN");
const DOM_POINT = { x: 90, y: 120 };

function baseProps(
  overrides: Partial<ComponentProps<typeof PromptQuickList>> = {}
): ComponentProps<typeof PromptQuickList> {
  return {
    prompts,
    messages: zh.quickList,
    groupMeta: zh.manager.groupMeta,
    onSelect: () => {},
    ...overrides,
  };
}

function renderQuickList(
  props: Partial<ComponentProps<typeof PromptQuickList>> = {}
) {
  return render(<PromptQuickList {...baseProps(props)} />);
}

// Position-aware elementFromPoint: the browser resolves each queried point
// against the live layout, so DOM-known positions and native-reported
// positions can resolve differently.
function mockElementFromPoint(
  resolve: (x: number, y: number) => Element | null
) {
  Object.defineProperty(document, "elementFromPoint", {
    configurable: true,
    value: (x: number, y: number) => resolve(x, y),
  });
  return () => Reflect.deleteProperty(document, "elementFromPoint");
}

function hoverFirstOption() {
  const option = screen.getByRole("option", { name: /讨论方案/i });
  fireEvent.pointerMove(option, {
    clientX: DOM_POINT.x,
    clientY: DOM_POINT.y,
  });
  return option;
}

function revealTooltip() {
  act(() => {
    vi.advanceTimersByTime(1500);
  });
  return screen.getByRole("tooltip");
}

describe("PromptQuickList hover stability (acceptance)", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("keeps the visible preview when a stale native observation misses while the DOM hover is live", () => {
    vi.useFakeTimers();
    const cleanup = mockElementFromPoint((x, y) =>
      x === DOM_POINT.x && y === DOM_POINT.y
        ? screen.getByRole("option", { name: /讨论方案/i })
        : null
    );
    try {
      const { rerender } = renderQuickList();
      const option = hoverFirstOption();
      revealTooltip();

      // A lagging native event carrying a gap position resolves to nothing,
      // yet the pointer never left the item (no pointerleave was received).
      rerender(
        <PromptQuickList
          {...baseProps({ nativePointerPosition: { x: 40, y: 63, inside: true } })}
        />
      );

      expect(screen.getByRole("tooltip")).toBeTruthy();
      expect(option.classList.contains("is-hovered")).toBe(true);
    } finally {
      cleanup();
    }
  });

  it("clears hover immediately on a fresh exit report (real departure contract)", () => {
    vi.useFakeTimers();
    const cleanup = mockElementFromPoint(() =>
      screen.queryByRole("option", { name: /讨论方案/i })
    );
    try {
      const { rerender } = renderQuickList();
      const option = hoverFirstOption();
      revealTooltip();

      // A genuinely new inside:false event stays authoritative.
      rerender(
        <PromptQuickList
          {...baseProps({ nativePointerPosition: { x: 0, y: 0, inside: false } })}
        />
      );

      expect(screen.queryByRole("tooltip")).toBeNull();
      expect(option.classList.contains("is-hovered")).toBe(false);
    } finally {
      cleanup();
    }
  });

  it("no longer cancels a pending preview timer on a stale native miss", () => {
    vi.useFakeTimers();
    const cleanup = mockElementFromPoint((x, y) =>
      x === DOM_POINT.x && y === DOM_POINT.y
        ? screen.getByRole("option", { name: /讨论方案/i })
        : null
    );
    try {
      const { rerender } = renderQuickList();
      hoverFirstOption();
      act(() => {
        vi.advanceTimersByTime(1400);
      });

      rerender(
        <PromptQuickList
          {...baseProps({ nativePointerPosition: { x: 40, y: 63, inside: true } })}
        />
      );
      act(() => {
        vi.advanceTimersByTime(1000);
      });

      expect(screen.getByRole("tooltip")).toBeTruthy();
    } finally {
      cleanup();
    }
  });

  it("does not replay a frozen exit report into a cancel when prompts change", () => {
    vi.useFakeTimers();
    const frozen: NativePopoverPointerPosition = { x: 0, y: 0, inside: false };
    const cleanup = mockElementFromPoint((x, y) =>
      x === DOM_POINT.x && y === DOM_POINT.y
        ? screen.getByRole("option", { name: /讨论方案/i })
        : null
    );
    try {
      // Mount with the frozen report (fresh at mount: no hover to clear).
      const { rerender } = renderQuickList({ nativePointerPosition: frozen });
      const option = hoverFirstOption();
      revealTooltip();

      // Data refresh replays the same observation object.
      rerender(
        <PromptQuickList
          {...baseProps({
            prompts: [...prompts],
            nativePointerPosition: frozen,
          })}
        />
      );

      expect(screen.getByRole("tooltip")).toBeTruthy();
      expect(option.classList.contains("is-hovered")).toBe(true);
    } finally {
      cleanup();
    }
  });

  it("does not re-kill a DOM-recovered hover when data changes replay an earlier miss", () => {
    vi.useFakeTimers();
    const miss: NativePopoverPointerPosition = { x: 0, y: 0, inside: false };
    const cleanup = mockElementFromPoint((x, y) =>
      x === DOM_POINT.x && y === DOM_POINT.y
        ? screen.getByRole("option", { name: /讨论方案/i })
        : null
    );
    try {
      const { rerender } = renderQuickList();

      // Native miss kills once (fresh exit semantics)...
      rerender(<PromptQuickList {...baseProps({ nativePointerPosition: miss })} />);
      // ...DOM movement recovers the hover...
      const option = hoverFirstOption();
      revealTooltip();

      // ...and the later data refresh must not replay the old miss.
      rerender(
        <PromptQuickList
          {...baseProps({
            prompts: [...prompts],
            nativePointerPosition: miss,
          })}
        />
      );

      expect(screen.getByRole("tooltip")).toBeTruthy();
      expect(option.classList.contains("is-hovered")).toBe(true);
    } finally {
      cleanup();
    }
  });

  it("does not replay a once-valid position into a cancel after a layout shift", () => {
    vi.useFakeTimers();
    const nativePosition: NativePopoverPointerPosition = { x: 40, y: 80, inside: true };
    let nativePositionHitsItem = true;
    const cleanup = mockElementFromPoint((x, y) => {
      if (x === DOM_POINT.x && y === DOM_POINT.y) {
        return screen.getByRole("option", { name: /讨论方案/i });
      }
      // The native-recorded point stops resolving after the layout shift
      // (category tabs appear, items move down); the DOM point still hits.
      return nativePositionHitsItem
        ? screen.getByRole("option", { name: /讨论方案/i })
        : null;
    });
    try {
      const { rerender } = renderQuickList();
      const option = hoverFirstOption();

      // Native twin arrives and hits the same item (correcting observation).
      rerender(
        <PromptQuickList {...baseProps({ nativePointerPosition: nativePosition })} />
      );
      revealTooltip();

      // Data update shifts the layout: the once-valid native point now misses.
      nativePositionHitsItem = false;
      rerender(
        <PromptQuickList
          {...baseProps({
            prompts: [...prompts],
            nativePointerPosition: nativePosition,
          })}
        />
      );

      expect(screen.getByRole("tooltip")).toBeTruthy();
      expect(option.classList.contains("is-hovered")).toBe(true);
    } finally {
      cleanup();
    }
  });

  it("still clears hover when the data update removes the hovered item", () => {
    vi.useFakeTimers();
    let itemStillRendered = true;
    const cleanup = mockElementFromPoint((x, y) => {
      if (x !== DOM_POINT.x || y !== DOM_POINT.y || !itemStillRendered) {
        return null;
      }
      return screen.queryByRole("option", { name: /讨论方案/i });
    });
    try {
      const { rerender } = renderQuickList();
      const option = hoverFirstOption();
      revealTooltip();

      // The refresh deletes the hovered entry: its element is gone, so the
      // DOM point no longer corroborates anything and cleanup must proceed.
      itemStillRendered = false;
      rerender(
        <PromptQuickList
          {...baseProps({
            prompts: [prompts[1]],
            nativePointerPosition: { x: 40, y: 80, inside: true },
          })}
        />
      );

      expect(screen.queryByRole("tooltip")).toBeNull();
    } finally {
      cleanup();
    }
  });

  it("clears hover on pointer leave and re-arms on re-entry with movement", () => {
    vi.useFakeTimers();
    renderQuickList();

    const option = hoverFirstOption();
    revealTooltip();

    fireEvent.pointerLeave(option);
    expect(screen.queryByRole("tooltip")).toBeNull();
    expect(option.classList.contains("is-hovered")).toBe(false);

    fireEvent.pointerMove(option, {
      clientX: DOM_POINT.x,
      clientY: DOM_POINT.y,
    });
    revealTooltip();
    expect(screen.getByRole("tooltip")).toBeTruthy();
  });
});
