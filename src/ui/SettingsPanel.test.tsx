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
    onLanguageChange: (language: AppLanguage) => void = () => {},
    onBack?: () => void
  ) {
    render(
      <SettingsPanel
        settings={settings}
        onLanguageChange={onLanguageChange}
        onPromptInsertionModeChange={onPromptInsertionModeChange}
        onBack={onBack}
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
    expect(screen.getByRole("button", { name: /界面语言.*中文/ })).toBeTruthy();
  });

  it("renders language selection as a custom dropdown row", () => {
    renderPanel();

    const trigger = screen.getByRole("button", { name: /界面语言.*中文/ });
    const row = trigger.closest(".settings-row");

    expect(row).toBeTruthy();
    expect(row?.querySelector(".settings-row-main")).toBeTruthy();
    expect(row?.querySelector(".settings-row-control")?.contains(trigger)).toBe(true);
    expect(screen.queryByRole("combobox")).toBeNull();
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

  it("opens and selects language from the custom dropdown", () => {
    let selectedLanguage: AppLanguage | null = null;
    renderPanel(mockSettings, () => {}, (language) => { selectedLanguage = language; });

    fireEvent.click(screen.getByRole("button", { name: /界面语言.*中文/ }));

    expect(screen.getByRole("listbox", { name: "界面语言" })).toBeTruthy();
    fireEvent.click(screen.getByRole("option", { name: "English" }));

    expect(selectedLanguage).toBe("en-US");
    expect(screen.queryByRole("listbox", { name: "界面语言" })).toBeNull();
  });

  it("closes the language dropdown with Escape", () => {
    renderPanel();

    fireEvent.click(screen.getByRole("button", { name: /界面语言.*中文/ }));
    fireEvent.keyDown(document, { key: "Escape" });

    expect(screen.queryByRole("listbox", { name: "界面语言" })).toBeNull();
  });

  it("closes the language dropdown on outside click", () => {
    renderPanel();

    fireEvent.click(screen.getByRole("button", { name: /界面语言.*中文/ }));
    fireEvent.pointerDown(document.body);

    expect(screen.queryByRole("listbox", { name: "界面语言" })).toBeNull();
  });

  it("renders optional manager back button", () => {
    let wentBack = false;
    renderPanel(mockSettings, () => {}, () => {}, () => { wentBack = true; });

    fireEvent.click(screen.getByRole("button", { name: "返回管理提示词" }));

    expect(wentBack).toBe(true);
  });

  it("renders English labels when English is selected", () => {
    renderPanel({ ...mockSettings, language: "en-US" });

    expect(screen.getByRole("heading", { name: "Settings" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Insert + Send" }).getAttribute(
      "aria-pressed"
    )).toBe("true");
  });
});
