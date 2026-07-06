# Autosend Target Recovery Verification Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the review finding that autosend target recovery lacked an auditable acceptance record after implementation.

**Architecture:** Keep application code unchanged unless verification exposes a concrete defect. Create a QA record that ties the final plan to the current code state, rerun the focused automated checks, build the app, and document which real-app manual scenarios were completed or remain user-owned because they involve active third-party accounts.

**Tech Stack:** Markdown QA docs, Git, npm/Vitest, Cargo tests, Tauri macOS build.

---

### Task 1: Create The Autosend Target Recovery QA Record

**Files:**
- Create: `docs/qa/2026-07-07-autosend-target-recovery.md`

**Step 1: Create the QA document**

Add this initial document:

```markdown
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
```

**Step 2: Confirm the document exists**

Run:

```bash
test -f docs/qa/2026-07-07-autosend-target-recovery.md
```

Expected: PASS with exit code 0.

**Step 3: Commit**

Do not commit build artifacts.

```bash
git add docs/qa/2026-07-07-autosend-target-recovery.md
git commit -m "docs: add autosend target recovery qa record"
```

---

### Task 2: Rerun Focused Automated Verification And Record It

**Files:**
- Modify: `docs/qa/2026-07-07-autosend-target-recovery.md`

**Step 1: Run focused frontend verification**

Run:

```bash
npm test -- src/overlay/overlayHtml.test.ts
```

Expected: PASS. This verifies the overlay opens the prompt list only after target capture.

**Step 2: Run Rust verification**

Run:

```bash
cd src-tauri
cargo test --lib
```

Expected: PASS. This covers the autosend state machine, Prompt Picker-only recovery, third-app copy-only safety, group prompt recovery, and macOS recovery source guard.

**Step 3: Update the QA document**

Replace `Pending.` under `## Automated Verification` with the commands and observed pass counts.

**Step 4: Commit**

```bash
git add docs/qa/2026-07-07-autosend-target-recovery.md
git commit -m "docs: record autosend target recovery automated verification"
```

---

### Task 3: Rerun Build Verification And Record It

**Files:**
- Modify: `docs/qa/2026-07-07-autosend-target-recovery.md`

**Step 1: Run frontend production build**

Run:

```bash
npm run build
```

Expected: PASS.

**Step 2: Run Tauri macOS build**

Run:

```bash
npm run tauri -- build
```

Expected: PASS and produce:

```text
src-tauri/target/release/bundle/macos/Prompt Picker.app
src-tauri/target/release/bundle/dmg/Prompt Picker_1.0.4_aarch64.dmg
```

If notarization is skipped because Apple notarization environment variables are missing, record that as a build-environment limitation, not an autosend implementation failure.

**Step 3: Update the QA document**

Replace `Pending.` under `## Build Verification` with the commands, output summary, app path, dmg path, and any notarization caveat.

**Step 4: Commit**

```bash
git add docs/qa/2026-07-07-autosend-target-recovery.md
git commit -m "docs: record autosend target recovery build verification"
```

---

### Task 4: Record Real-App Manual Verification Boundary

**Files:**
- Modify: `docs/qa/2026-07-07-autosend-target-recovery.md`

**Step 1: Check whether safe manual execution is possible**

Before running any real-app scenario, confirm there is a safe scratch target where sending text will not affect a real user/account conversation.

Use this safety rule:

```text
Do not send text into WeChat, Claude, Codex, or any third-party account unless the user explicitly provides a scratch input/conversation and confirms it is safe.
```

**Step 2: Update real-app verification table**

If safe manual execution is not possible in this agent session, keep the real-app rows as not executed and record why:

```markdown
Manual real-app tests were not executed by the agent because they require interacting with active third-party application windows/accounts. This is intentionally left as user-owned verification to avoid sending text into the wrong conversation or account.
```

If the user provides safe scratch windows later, update each row with Pass/Fail and notes.

**Step 3: Update acceptance notes**

Set acceptance notes to:

```markdown
Code-level implementation and automated verification are complete. Final product acceptance still requires user-side real-app manual verification for Codex, Claude, WeChat, and third-app switching.
```

**Step 4: Commit**

```bash
git add docs/qa/2026-07-07-autosend-target-recovery.md
git commit -m "docs: record autosend target recovery manual verification boundary"
```

---

### Task 5: Push The Verification Record

**Files:**
- Git history only.

**Step 1: Confirm only intentional docs changes are committed**

Run:

```bash
git status --short
git log --oneline -6
```

Expected: QA record commits exist. Existing build artifacts may remain dirty but must not be staged.

**Step 2: Push**

Run:

```bash
git push origin main
```

Expected: push succeeds.

---

## User-Visible Result

After this verification fix, the project has an auditable QA record for autosend target recovery. The implementation can be considered code-complete and automatically verified, while real-app acceptance remains explicitly gated on safe user-side manual testing instead of being falsely marked as passed.
