# Autosend Review Test Alignment Fix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Resolve the review finding that one macOS autosend test still encodes the old "legacy activating paste is absent" framing instead of the restored captured-target autosend contract.

**Architecture:** Do not change autosend runtime behavior. Replace the stale test in `src-tauri/src/platform/macos.rs` with positive assertions that the production activating sender exists, delegates submit-key handling, and does not interpolate prompt bodies into AppleScript.

**Tech Stack:** Rust, Cargo unit tests, Tauri macOS backend.

---

### Task 1: Replace The Stale Anti-Legacy Test

**Files:**
- Modify: `src-tauri/src/platform/macos.rs`

**Step 1: Locate the stale test**

Find:

```rust
#[test]
fn legacy_activating_paste_script_is_not_present() {
```

This test name and framing are no longer aligned with the restored captured-target autosend plan.

**Step 2: Replace it with a positive contract test**

Replace it with:

```rust
#[test]
fn activating_clipboard_sender_is_available_without_prompt_body_scripting() {
    let source = include_str!("macos.rs");
    let start = source
        .find("pub fn paste_prompt_and_submit_to_app_clipboard_with_copier")
        .expect("activating sender should exist");
    let end = source[start..]
        .find("#[allow(dead_code)]")
        .expect("next legacy helper should follow activating sender");
    let sender_source = &source[start..start + end];

    assert!(sender_source.contains("recover_target_app_for_autosend"));
    assert!(sender_source.contains("post_focus_preserving_submit_key"));
    assert!(!sender_source.contains("keystroke \"{body}\""));
    assert!(!sender_source.contains("keystroke \"Test body\""));
}
```

Keep the exact assertions flexible if formatting requires line wrapping, but preserve the behavior:

- Assert the restored activating clipboard sender exists.
- Assert it uses target recovery and submit-key handling.
- Assert it does not script literal prompt body text.

**Step 3: Run the focused test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib activating_clipboard_sender_is_available_without_prompt_body_scripting
```

Expected: one test passes.

**Step 4: Run full Rust backend tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

Expected: all backend unit tests pass.

**Step 5: Commit and push**

Run:

```bash
git add src-tauri/src/platform/macos.rs docs/plans/2026-07-07-autosend-review-test-alignment-fix.md
git commit -m "test: align autosend activating sender assertion"
git push origin main
```

Expected: `origin/main` contains the new fix commit.
