# Bundled Prompt Library Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Give new users the maintained Chinese prompt library on first launch while preserving every existing personal library, then release a fresh SemVer version for macOS and Windows.

**Architecture:** Embed `examples/prompts/prompts-zh.json` into the Rust executable with `include_str!`. During native startup, before prompt windows are initialized, create the AppData `prompts.json` only with exclusive `create_new` semantics. Existing files, including intentionally empty libraries and invalid/unreadable files, are never opened for writing by initialization. Normal editing/import/export continues to use the existing local storage.

**Tech Stack:** Tauri 2, Rust standard filesystem APIs, React/TypeScript, Vitest, GitHub Actions, Apple Developer ID and notarytool.

## Design decisions

- Seed from a compiled resource, so first launch works offline and does not fetch GitHub.
- Use native startup rather than frontend empty-list detection: an existing empty library remains empty, upgrades do not replace content, and no frontend migration or new filesystem capabilities are required.
- Preserve the full current library: four categories, 50 containers, 57 prompt bodies, and two dividers. Users can edit, delete, create, import, and export normally.
- Select v1.4.0 as a backward-compatible feature release after the previously built v1.3.5. Check local artifacts, remote tags, Releases, and Actions before building; never rebuild a completed package under an already-used version.

## Task 1: Native initialization

**Files:** Create `src-tauri/src/prompt_seed.rs`; modify `src-tauri/src/lib.rs`.

1. Add filesystem tests for first creation, nested AppData directory creation, byte-for-byte preservation of an existing personal library, preservation of an existing empty/invalid file, user-cleared library across restarts, and filesystem error propagation.
2. Run the new tests with `rustc --edition=2021 --test src-tauri/src/prompt_seed.rs -o /private/tmp/sleepy-cat-prompt-seed-tests` and execute the binary. Confirm first-creation assertions fail before implementing initialization.
3. Implement exclusive file creation. Return `Ok(false)` for `AlreadyExists`, write the embedded JSON only to a newly created file, and report genuine I/O failures.
4. Call initialization from Tauri setup using the same AppData path as the existing frontend storage. Seed errors are reported without replacing user content.
5. Re-run native tests, run the full frontend test suite, and verify the embedded JSON imports through the actual prompt store without losing categories, containers, bodies, or dividers.

## Task 2: Documentation and version

**Files:** `README.md`, `README.zh-CN.md`, `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`.

1. Document first-run defaults, normal personal editing/import/export, and preservation on upgrade.
2. Set all application/package versions to v1.4.0 without changing dependency versions.
3. Run frontend tests, TypeScript/Vite build, native tests, and `git diff --check`. Review the diff and commit only intentional source/docs/version changes.
4. Push the commit to main, create a v1.4.0 tag, and build both platforms from that exact commit.

## Task 3: Release

1. Dispatch the existing Windows build workflow against the tag; inspect its result, download the NSIS artifact, and calculate SHA-256.
2. Build the macOS Apple Silicon app once for the new version, sign with the installed Developer ID Application identity and hardened runtime, and verify its signature.
3. Submit to Apple notarization, staple the accepted app, create/sign/notarize/staple its DMG, and verify with codesign, stapler, and Gatekeeper.
4. Apple currently rejects notarization with HTTP 403 for a missing/expired team agreement. The account holder is temporarily unable to resolve it. Preserve the signed Mac build for later notarization; do not publish it as a notarized download or rebuild under the same version.
5. Publish the verified Windows installer and checksums to the tagged GitHub Release, clearly state Mac availability if notarization remains blocked, and retain the local signed Mac artifacts and diagnostics.
6. Verify remote main/tag/Release assets and hashes, then fast-forward the original checkout without disturbing its existing `.gitignore` and generated-build changes.
