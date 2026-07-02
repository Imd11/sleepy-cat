# OpenWhip Foreground Autosend Execution Plan

Goal: Make prompt selection use the same focus model as OpenWhip source code: keep the previous application as the keyboard target, refocus with Cmd+Tab only when Prompt Picker is frontmost, paste the selected prompt, and press Enter without blocking permission dialogs.

Source finding:
- OpenWhip creates a focusless, always-on-top overlay.
- It calls Cmd+Tab when the overlay is spawned, not by activating a recorded bundle id.
- Its macOS macro sends `System Events` keystrokes to the currently focused app.
- It does not inspect input coordinates, AX focused elements, or target bundle ids for sending.
- Failures are logged, not shown as blocking dialogs.

Risk controls:
- This flow cannot bypass macOS Accessibility permission. If Accessibility is not granted, the app can copy the prompt to the clipboard but cannot reliably send `Cmd+V` or `Enter`.
- Foreground sending is intentionally less strict than the previous last-target gate. It improves Codex/WeChat reliability, but if the user triggers it while no text field has focus, macOS may deliver `Cmd+V` or `Enter` to the wrong UI. This is acceptable only because the user explicitly asked for OpenWhip-style behavior and non-blocking UX.
- The prompt body must be copied to the clipboard before any keyboard automation so failed autosend still leaves a manual fallback.
- Cmd+Tab must be conditional. Run it only when Prompt Picker is actually frontmost; unconditional Cmd+Tab can move away from the real target because the Calico/popover windows are intended to be non-activating.
- After Cmd+Tab, leave enough time for macOS to settle before sending paste/enter. If real-world testing shows missed inserts, increase this delay rather than reintroducing bundle activation.
- Keep the older target-app helpers available as legacy fallback utilities, but do not use them as the primary autosend path because `activate bundle id` does not guarantee that the previous text field still has focus.
- Do not reintroduce blocking permission dialogs in prompt selection. If user-facing permission guidance is needed later, put it in the main settings/status surface, not in the prompt-click path.

Impact assessment:
- Prompt selection behavior changes from "requires recorded last target" to "uses current foreground keyboard target." This is intended.
- Plain paste-to-last-target remains target-based and still protects against missing target.
- Prompt management, import/export, menu bar controls, Calico visibility, and popover positioning are not part of this change and should not be edited.
- The plan is not a pure Pareto improvement in the strict theoretical sense because it trades some safety against wrong-focus paste for much better reliability and a less disruptive UX. For this personal utility, it is a pragmatic Pareto-style improvement for the requested workflow because it improves the primary Codex/WeChat use case without intentionally degrading existing management or visibility features.

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
   - Inspect packaged binary strings for `foreground-paste-and-submit` and absence of the old blocking permission string.

5. Commit and push.
   - Stage only source/config/plan files.
   - Do not commit generated `dist`, `node_modules`, `target`, `.app`, or `.dmg` artifacts.
   - Push to GitHub `main`.

Go / no-go conclusion:
- Go only if the implementation keeps the prompt-click path non-blocking, copies to clipboard before sending keys, avoids mandatory last-target lookup for autosend, conditionally Cmd+Tabs only when Prompt Picker is frontmost, and passes the verification commands above.
- No-go if the implementation reintroduces blocking dialogs, blindly Cmd+Tabs on every prompt click, removes the manual clipboard fallback, or touches unrelated app management/UI behavior.
