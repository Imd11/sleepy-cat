import { describe, expect, it } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SettingsPanel } from "./SettingsPanel";
import type { AppLanguage, PromptInsertionMode, Settings } from "../shared/settingsStore";

describe("settings panel", () => {
  const mockSettings: Settings = {
    version: 1,
    blacklistedApps: [
      { bundleId: "com.example.app", name: "Example App" }
    ],
    overlayPlacement: { buttonOffset: null, buttonPosition: null },
    floatingButton: { visible: true },
    promptInsertion: { mode: "paste_and_submit" },
    language: "zh-CN",
  };

  function renderPanel(
    settings: Settings = mockSettings,
    onPromptInsertionModeChange: (mode: PromptInsertionMode) => void = () => {},
    onLanguageChange: (language: AppLanguage) => void = () => {}
  ) {
    render(
      <SettingsPanel
        settings={settings}
        onLanguageChange={onLanguageChange}
        onPromptInsertionModeChange={onPromptInsertionModeChange}
      />
    );
  }

  it("does not render hidden apps settings", () => {
    renderPanel();

    expect(screen.queryByText("隐藏应用")).toBeNull();
    expect(screen.queryByText("暂无隐藏应用")).toBeNull();
    expect(screen.queryByText("Example App")).toBeNull();
  });

  it("renders prompt insertion behavior controls", () => {
    renderPanel();

    expect(screen.getByRole("button", { name: "只填入输入框" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "填入并发送" }).getAttribute(
      "aria-pressed"
    )).toBe("true");
  });

  it("changes prompt insertion mode", () => {
    let selectedMode: PromptInsertionMode | null = null;
    renderPanel(mockSettings, (mode) => { selectedMode = mode; });

    fireEvent.click(screen.getByRole("button", { name: "只填入输入框" }));

    expect(selectedMode).toBe("paste_only");
  });

  it("renders language selection", () => {
    renderPanel();

    expect(screen.getByRole("heading", { name: "语言" })).toBeTruthy();
    expect(screen.getByLabelText("界面语言")).toBeTruthy();
  });

  it("renders language selection as a right-aligned settings row", () => {
    renderPanel();

    const languageSelect = screen.getByLabelText("界面语言");
    const row = languageSelect.closest(".settings-row");

    expect(row).toBeTruthy();
    expect(row?.querySelector(".settings-row-main")).toBeTruthy();
    expect(row?.querySelector(".settings-row-control")?.contains(languageSelect)).toBe(true);
  });

  it("does not render instructional settings descriptions", () => {
    renderPanel();

    expect(screen.queryByText("控制 Calico 如何填入提示词。")).toBeNull();
    expect(screen.queryByText("选择应用界面使用的语言。")).toBeNull();
    expect(screen.queryByText("选择点击提示词后，只填入输入框，还是填入并发送。")).toBeNull();
    expect(screen.queryByText("在这些应用中隐藏小猫。")).toBeNull();
  });

  it("renders prompt click behavior as a compact settings row", () => {
    renderPanel();

    const selectedButton = screen.getByRole("button", { name: "填入并发送" });
    const row = selectedButton.closest(".settings-row");

    expect(row).toBeTruthy();
    expect(row?.querySelector(".settings-row-control")?.contains(selectedButton)).toBe(true);
  });

  it("changes language", () => {
    let selectedLanguage: AppLanguage | null = null;
    renderPanel(mockSettings, () => {}, (language) => { selectedLanguage = language; });

    fireEvent.change(screen.getByLabelText("界面语言"), {
      target: { value: "en-US" },
    });

    expect(selectedLanguage).toBe("en-US");
  });

  it("renders English labels when English is selected", () => {
    renderPanel({ ...mockSettings, language: "en-US" });

    expect(screen.getByRole("heading", { name: "Settings" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Insert + Send" }).getAttribute(
      "aria-pressed"
    )).toBe("true");
  });
});
