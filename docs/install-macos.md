# Installing Prompt Picker (macOS Developer Build)

## Opening the App

1. Locate the app bundle at:
   ```
   /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app
   ```

2. Option-click the app and select "Open" to launch (bypasses gatekeeper on first run)

## Granting Accessibility Permission

1. Open System Settings > Privacy & Security > Accessibility
2. Find "Prompt Picker" in the list
3. Enable the toggle

## Finding the Menu Bar Icon

The app appears in the menu bar with a "P" icon. Look in the top-right corner of your screen.

## Using Prompt Picker

1. **With Codex App**: Open Codex and click into any text input field. A small "P" button should appear. Click it to see your prompts.

2. **Fallback Mode**: When no input detection is available, the app shows a floating mini button. Click it to paste to your current cursor position.

## Importing/Exporting Prompts

1. Open the Prompt Manager from the popover footer
2. Use "Import" to load a `.json` prompt library
3. Use "Export" to save your prompts to a `.json` file

## Blacklist Settings

To blacklist an app (hide Prompt Picker overlay):
1. Right-click the menu bar icon > Settings
2. Add apps to the blacklist

## Known Limitations

- **v1 acceptance target is Codex App only**. Other desktop apps may work but are not guaranteed.
- **Button is a system floating window**, not embedded inside Codex App's UI
- **Windows support** is not implemented in v1
- **Accessibility detection** requires macOS permission and may need Rosetta on Apple Silicon Macs for full functionality