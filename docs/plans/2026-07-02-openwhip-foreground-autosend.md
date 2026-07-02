# OpenWhip Foreground Autosend Execution Plan

Goal: Make prompt selection use the same focus model as OpenWhip source code: keep the previous application as the keyboard target, refocus with Cmd+Tab only when Prompt Picker is frontmost, paste the selected prompt, and press Enter without blocking permission dialogs.

Source finding:
- OpenWhip creates a focusless, always-on-top overlay.
- It calls Cmd+Tab when the overlay is spawned, not by activating a recorded bundle id.
- Its macOS macro sends `System Events` keystrokes to the currently focused app.
- It does not inspect input coordinates, AX focused elements, or target bundle ids for sending.
- Failures are logged, not shown as blocking dialogs.

Tasks:

1. Add regression coverage for non-blocking prompt selection.
   - Verify prompt click hides the popover before autosend.
   - Verify autosend failures log without `dialog.message`.
   - Verify no blocking dialog capability remains.

2. Replace autosend primary path with foreground keyboard sending.
   - Add a macOS foreground paste+enter script that does not activate a bundle id.
   - Copy prompt text to clipboard before sending keys.
   - If Prompt Picker is frontmost, run the OpenWhip-style Cmd+Tab refocus first.
   - Do not require a recorded last input target before attempting autosend.
   - Keep existing targeted helpers only as legacy/fallback helpers.

3. Keep user experience non-blocking.
   - Remove the frontend blocking dialog call from prompt selection.
   - Keep errors in logs only.
   - Keep popover hiding behavior so the user returns to the target app quickly.

4. Verify and package.
   - Run targeted tests for the changed behavior.
   - Run full frontend tests.
   - Run Rust format and full Rust lib tests.
   - Run frontend build.
   - Build the Tauri app and DMG.
   - Restart the local Prompt Picker app.

5. Commit and push.
   - Stage only source/config/plan files.
   - Do not commit generated `dist`, `node_modules`, `target`, `.app`, or `.dmg` artifacts.
   - Push to GitHub `main`.
