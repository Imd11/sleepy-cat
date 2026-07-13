import { readFileSync } from "fs";
import { describe, expect, it } from "vitest";

describe("quick picker layout styles", () => {
  const css = readFileSync("src/styles.css", "utf8");
  const rule = (selector: string) => {
    const match = css.match(new RegExp(`${selector.replace(".", "\\.")}\\s*{[^}]*}`));
    return match?.[0] ?? "";
  };

  it("does not render the old popover bottom triangle", () => {
    expect(css).not.toContain(".popover-window::after");
  });

  it("keeps tabs fixed above the scrollable prompt list", () => {
    expect(css).toContain(".popover-window");
    expect(css).toContain("display: flex");
    expect(css).toContain(".prompt-category-tabs");
    expect(css).toContain(".prompt-quick-list");
    expect(css).toContain("overflow-y: auto");
  });

  it("keeps group cards to three visual rows without flex compression", () => {
    const itemRule = rule(".prompt-quick-item");
    const groupRule = rule(".prompt-quick-item-group");
    const titleRowRule = rule(".prompt-quick-title-row");
    const titleRule = rule(".prompt-quick-title");

    expect(itemRule).toContain("flex: 0 0 auto");
    expect(groupRule).toContain("height: 84px");
    expect(groupRule).toContain("overflow: hidden");
    expect(titleRowRule).toContain("flex-wrap: nowrap");
    expect(titleRule).toContain("text-overflow: ellipsis");
    expect(titleRule).toContain("white-space: nowrap");
    expect(css).toContain(".prompt-quick-title-row .prompt-quick-meta");
  });

  it("gives prompt cards immediate hover and pressed feedback", () => {
    const hoverRule = rule(".prompt-quick-item.is-hovered");
    const activeRule = rule(".prompt-quick-item:active");

    expect(css).not.toContain(".prompt-quick-item:hover {");
    expect(hoverRule).toContain("background: #e7eef8");
    expect(hoverRule).toContain("border-color: #7f93ad");
    expect(hoverRule).toContain("rgba(15, 23, 42, 0.14)");
    expect(activeRule).toContain("background: #dce6f2");
  });

  it("keeps the rounded popover panel flush with the native popover window", () => {
    const rootRule = rule(".popover-root");
    const windowRule = rule(".popover-window");

    expect(css).toContain("--pp-popover-window-padding: 0px");
    expect(rootRule).toContain("padding: 0");
    expect(windowRule).toContain("width: 100%");
    expect(windowRule).toContain("height: 100%");
    expect(windowRule).toContain("box-shadow: none");
    expect(windowRule).not.toContain("box-shadow: var(--pp-shadow-popover)");
    expect(windowRule).not.toContain("width: 100vw");
    expect(windowRule).not.toContain("min-height: 100vh");
  });
});
