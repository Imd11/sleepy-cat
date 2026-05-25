import { describe, expect, it } from "vitest";
import { getPromptPreview, normalizePromptTitle } from "./promptTypes";

describe("prompt model helpers", () => {
  it("trims prompt titles", () => {
    expect(normalizePromptTitle("  Code Review  ")).toBe("Code Review");
  });

  it("creates a compact preview from the prompt body", () => {
    expect(getPromptPreview("Line one\n\nLine two is longer", 18)).toBe("Line one Line two...");
  });
});