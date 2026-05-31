# Always Visible Floating Button — Original Plan

> **Date:** 2026-05-31
> **Status:** Superseded by repair plan

## Product Contract

The floating blue `Prompts` button must:
- Appear automatically when Prompt Picker launches
- Remain visible when switching to any ordinary app (Finder, browser, Codex, etc.)
- NOT require an input text target to be visible
- NOT disappear when the main window is closed/hidden
- Left-click opens a compact prompt list (no Add/Edit/Delete/Import/Export/Settings/Back)
- Right-click opens a compact controls popover with `Hide Button` and `Open Prompt Picker`
- `Hide Button` hides the floating button
- Re-opening the main window shows `Status: Hidden` with a `Show Floating Button` option
- The button can be restored without using the menu bar or system tray

## Architecture

Three-window Tauri model:
- `main` — client app window (manager + settings)
- `prompt-button` — persistent lightweight overlay (blue `Prompts` button)
- `prompt-popover` — renders either the quick prompt list or button-controls panel

Button visibility controlled by `settings.floatingButton.visible` (persisted), NOT by input-target detection.

## Tasks

### Task 1: Restore plan traceability and test script ✅
- Restore original plan document
- Restore `npm test` script

### Task 2: Restore macOS platform runtime behavior
- Restore `current_input_target()` delegation (was stubbed to `None`)
- Restore `accessibility_status()` (was hardcoded to `false`)
- Restore `frontmost_app()` (was stubbed to `None`)
- Keep existing paste behavior intact

### Task 3: Make the floating button actually appear by default
- Add `DEFAULT_BUTTON_POSITION = [960, 700]`
- Initialize `lastButtonPositionRef` to default position (not `null`)
- When no input target exists, show button at default/fallback position
- Startup creates button via Rust `show_prompt_button(960, 700, ...)`
- Manual hidden state is the only normal hide path

### Task 4: Wire right-click button controls end-to-end
- Register `open_main_window` in `tauri::generate_handler!`
- Add `show_prompt_button_controls_from_button` Rust command
- Wire `public/overlay.html` contextmenu event to call the command
- `Hide Button` calls both settings update AND `hidePromptButton()`
- Static import for `openMainWindow` (no dynamic import warning)

### Task 5: Repair window size, URL mode, and non-activating behavior
- Restore button dimensions: 112×40px
- Restore "Prompts" label in button
- Use compact popover: 280×240px
- Keep non-activating panel config for overlay windows
- Left-click opens `index.html?mode=popover`
- Right-click opens `index.html?mode=button-controls`

### Task 6: Fix tests so they catch the real bugs
- No-target test must assert `showPromptButton` WAS called (not just "not hidden")
- Add overlay HTML wiring test
- Strengthen App controls test with hide + popover calls

### Task 7: Full verification
- `npm test` — PASS
- `npm run build` — PASS
- `cargo test` — PASS
- `cargo fmt -- --check` — PASS
- `npm run tauri build` — PASS
- Manual acceptance test — all 9 checks pass
