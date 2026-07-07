# Autosend Never-Key Recovery QA

Date: 2026-07-07

## Scope

This QA covers the autosend focus preservation work from
`docs/plans/2026-07-07-autosend-never-key-recovery.md`.

The verified changes are:

- Prompt button and prompt popover windows are created hidden and non-focusable before their first non-activating show path.
- Autosend focus diagnostics are available behind `PROMPT_PICKER_FOCUS_DIAGNOSTICS`.
- Recovery click points are selected with generic rules instead of Codex-specific fallback logic.
- Existing safety behavior is retained: when the target cannot be restored, the app copies only and does not send.

## Automated Verification

Passed:

- `cargo fmt --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml recovery_click_point --lib`
- `cargo test --manifest-path src-tauri/Cargo.toml fallback_click_point --lib`
- `cargo test --manifest-path src-tauri/Cargo.toml last_input_target --lib`
- `cargo test --manifest-path src-tauri/Cargo.toml autosend --lib`
- `cargo test --manifest-path src-tauri/Cargo.toml macos_panels --lib`
- `cargo test --manifest-path src-tauri/Cargo.toml windows --lib`
- `cargo test --manifest-path src-tauri/Cargo.toml --lib`
- `npm test`
- `npm run build`
- `git diff --check -- src-tauri/src/lib.rs src-tauri/src/platform/macos.rs src-tauri/src/platform/mod.rs src-tauri/src/platform/unsupported.rs`

Automated result summary:

- Rust lib tests: 169 passed.
- Vitest files: 23 passed.
- Vitest tests: 284 passed.
- Frontend production build completed successfully.

## Source Checks

Passed:

- No `allows_fallback_click_point` function remains.
- No legacy `paste_prompt_to_app`, `paste_prompt_to_last_target`, `pastePromptToApp`, `pastePromptToLastTarget`, or `paste_to_app_script` references were found in `src` or `src-tauri/src`.
- Codex bundle-id references remain only in unit tests and script fixtures, not in production fallback selection logic.

## Manual Diagnostic Verification

Run command:

`PROMPT_PICKER_FOCUS_DIAGNOSTICS=1 npm run tauri -- dev`

Temporary diagnostic setup:

- The local `local.promptpicker.dev` settings and prompt library were backed up before testing.
- The app was temporarily configured with a single paste-only diagnostic prompt:
  `PROMPT_PICKER_DIAGNOSTIC_TEST`.
- The original `settings.json` and `prompts.json` were restored after testing.

Captured diagnostic evidence:

```text
prompt-picker-panel label=prompt-button class=NSKVONotifying_TaoWindow action=ManagedTauriRuntime can_become_key=false can_become_main=false
```

| Scenario | prompt-button key? | prompt-popover key? | Classification | Recovery Used | Outcome | Notes |
|---|---:|---:|---|---:|---|---|
| Codex normal autosend | false | NOT RUN | NOT RUN | NOT RUN | NOT RUN | Computer Use refuses to operate `com.openai.codex`, so this row could not be executed safely by the agent. |
| Claude normal autosend | false | NOT OBSERVED | NOT OBSERVED | NOT OBSERVED | NOT RUN | Claude input was focused and the pet opened the popover. Available automation could not reliably trigger the prompt row's real `onClick`; the popover closed without changing the clipboard or producing autosend diagnostics. |
| WeChat normal autosend | false | NOT RUN | NOT RUN | NOT RUN | NOT RUN | Not executed after prompt-row selection could not be triggered reliably; avoiding unsafe interaction with an active chat. |
| Focus-break safety | n/a | n/a | NOT RUN | NOT RUN | NOT RUN | Not executed because a trustworthy prompt selection event could not be produced from automation. |

Manual diagnostic matrix result: **FAIL / NOT READY**.

The only trusted live diagnostic evidence from this run is that the prompt button window is a Tao-managed runtime window but reports `can_become_key=false` and `can_become_main=false`. The prompt popover and autosend classification rows were not observed, so this QA does not claim Codex, Claude, WeChat, or focus-break acceptance.

## Notes

The Tao/Wry class guard was not removed. The plan made this conditional on diagnostics proving the runtime-managed window class still steals focus after the show-ordering fix. This run produced the opposite evidence for the prompt button (`can_become_key=false`) and did not produce enough popover/autosend evidence to justify changing the guard.

## Final Acceptance Status

Automated verification: PASS

Manual diagnostic matrix: FAIL / NOT READY

Task 4 required: unknown; no trigger evidence was produced

Task 4 executed: no

Acceptance recommendation: NEEDS FIX / MANUAL DIAGNOSTIC REQUIRED
