# Autosend Target Recovery QA

Date: 2026-07-07
Plan: `docs/plans/2026-07-07-autosend-target-recovery.md`
Review fix plan: `docs/plans/2026-07-07-autosend-target-recovery-verification.md`

## Scope

This record closes the acceptance gap found in the post-implementation review for autosend target recovery.

The implementation goal is:

- Capture the target before opening Prompt Picker UI.
- Preserve pure focus-preserving paste/submit when the original target remains frontmost.
- Recover only when Prompt Picker itself became frontmost.
- Copy only when another non-target app is frontmost.
- Apply the same rule to single prompts and prompt groups.

## Automated Verification

Pending.

## Build Verification

Pending.

## Real-App Manual Verification

The following scenarios require real foreground apps and may send text into active user accounts. They must not be marked as passed unless they are actually performed in a safe scratch conversation/input:

| Scenario | Status | Notes |
|---|---|---|
| Codex visible, input not manually focused, choose prompt | Pending user-safe manual test | Requires active Codex UI. |
| Claude input focused once, choose prompt | Pending user-safe manual test | Requires active Claude UI. |
| WeChat chat input focused once, choose prompt | Pending user-safe manual test | Requires safe scratch chat or user confirmation. |
| Start from Codex, switch to third app before selecting prompt | Pending user-safe manual test | Must confirm copy-only/no wrong-target send. |

## Acceptance Notes

Pending.
