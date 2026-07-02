#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_PATH="$ROOT_DIR/src-tauri/target/release/bundle/macos/Prompt Picker.app"
DMG_PATH="$ROOT_DIR/src-tauri/target/release/bundle/dmg/Prompt Picker_1.0.0_aarch64.dmg"
BUNDLE_IDENTIFIER="local.promptpicker.dev"
SIGNING_IDENTITY="${PROMPT_PICKER_CODESIGN_IDENTITY:-}"

if [[ ! -d "$APP_PATH" ]]; then
  echo "Missing app bundle: $APP_PATH" >&2
  exit 1
fi

if [[ -z "$SIGNING_IDENTITY" ]]; then
  SIGNING_IDENTITY="$(
    security find-identity -v -p codesigning 2>/dev/null \
      | awk -F '"' '/Apple Development:/ { print $2; exit }'
  )"
fi

if [[ -z "$SIGNING_IDENTITY" ]]; then
  SIGNING_IDENTITY="-"
  echo "No Apple Development signing identity found; using ad-hoc signing." >&2
else
  echo "Using signing identity: $SIGNING_IDENTITY" >&2
fi

codesign --force --deep --sign "$SIGNING_IDENTITY" --identifier "$BUNDLE_IDENTIFIER" "$APP_PATH"
codesign --verify --deep --strict --verbose=2 "$APP_PATH"

if command -v hdiutil >/dev/null 2>&1; then
  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "$tmp_dir"' EXIT
  ditto "$APP_PATH" "$tmp_dir/Prompt Picker.app"
  mkdir -p "$(dirname "$DMG_PATH")"
  hdiutil create -volname "Prompt Picker" -srcfolder "$tmp_dir" -ov -format UDZO "$DMG_PATH"
fi

codesign -dv --verbose=4 "$APP_PATH" 2>&1 | sed -n '1,24p'
