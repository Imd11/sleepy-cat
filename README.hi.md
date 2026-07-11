# Prompt Drawer

**इसे इस भाषा में पढ़ें:** [English](README.md) | [简体中文](README.zh-CN.md) | **हिन्दी** | [Español](README.es.md) | [العربية](README.ar.md)

Prompt Drawer Codex के लिए बनाया गया local desktop prompt library है। यह सक्रिय Codex input के पास एक floating button रखता है, एक compact prompt picker खोलता है, और चुने हुए prompt को आपके काम की जगह पर insert करता है।

यह app Tauri, React, और Rust से बना है। Prompt data user की अपनी machine पर local रूप से store होता है।

## Features

- Compact prompt list के साथ floating prompt button।
- Single prompts और grouped prompt sequences के लिए local prompt manager।
- Prompt collections को organize करने के लिए category support।
- Paste-only और paste-and-submit insertion modes।
- Prompt libraries को JSON के रूप में import और export करना।
- चुनी हुई JSON file को app में किए गए edits के साथ sync रखने के लिए optional link-and-sync mode।
- Local-first storage; prompt data किसी server पर upload नहीं होता।
- macOS menu bar app packaging, Developer ID signing और notarization के साथ।
- GitHub Actions के जरिए Windows installer build।

## Download

Latest release GitHub पर उपलब्ध है:

https://github.com/Imd11/prompt-drawer/releases/latest

Current packaged builds:

- macOS Apple Silicon DMG
- Windows x64 installer

macOS पर Prompt Drawer को दूसरे apps में text paste और submit करने के लिए Accessibility permission चाहिए।

## Example Prompt Libraries

इस repository में दो example prompt libraries शामिल हैं:

- `examples/prompts/prompts-zh.json`
- `examples/prompts/prompts-en.json`

इनमें planning, execution, review, debugging, और release prompts के साथ एक development workflow prompt set है।

इनमें से किसी एक को इस्तेमाल करने के लिए:

1. Prompt Drawer खोलें।
2. Prompt manager में जाएँ।
3. Import पर click करें।
4. `examples/prompts/` से कोई JSON file चुनें।
5. इसे app की internal copy के रूप में import करें, या selected JSON file को link and sync करें।

Copy के रूप में import करने से current internal prompt library replace हो जाती है, इसलिए अगर आप backup रखना चाहते हैं तो पहले अपने current prompts export कर लें। अगर आप link and sync चुनते हैं, तो Prompt Drawer selected file path store करता है और app में आगे किए गए prompt edits वापस उसी JSON file में लिखता है। App आपके Desktop को scan नहीं करता और अपने आप कोई prompt file नहीं चुनता।

## Local Data

Prompt Drawer user data को local रूप से store करता है।

macOS पर prompts यहाँ store होते हैं:

```text
~/Library/Application Support/local.promptpicker.dev/prompts.json
```

Settings उसी जगह store होती हैं:

```text
~/Library/Application Support/local.promptpicker.dev/settings.json
```

Prompts export करने से अलग JSON backup बनता है। इससे app की default storage location नहीं बदलती।

जब आप JSON file import करते हैं, Prompt Drawer default रूप से internal `prompts.json` इस्तेमाल करता है। Link and sync हर imported file के लिए opt-in है, और इसे prompt manager से external JSON file delete किए बिना हटाया जा सकता है।

## Development

Dependencies install करें:

```bash
npm install
```

Frontend development server चलाएँ:

```bash
npm run dev
```

Tests चलाएँ:

```bash
npm test
```

Frontend build करें:

```bash
npm run build
```

Tauri app build करें:

```bash
npm run tauri -- build
```

## macOS Release Build

Tauri config Developer ID signing के लिए set up है। Public macOS release के लिए DMG को build, notarize, और staple करें:

```bash
npm run tauri -- build --bundles dmg
xcrun notarytool submit "src-tauri/target/release/bundle/dmg/Prompt Drawer_<version>_aarch64.dmg" \
  --key /path/to/AuthKey_<KEY_ID>.p8 \
  --key-id <KEY_ID> \
  --issuer <ISSUER_ID> \
  --wait
xcrun stapler staple "src-tauri/target/release/bundle/dmg/Prompt Drawer_<version>_aarch64.dmg"
xcrun stapler validate "src-tauri/target/release/bundle/dmg/Prompt Drawer_<version>_aarch64.dmg"
```

Gatekeeper acceptance verify करें:

```bash
spctl --assess --type open --context context:primary-signature --verbose=4 \
  "src-tauri/target/release/bundle/dmg/Prompt Drawer_<version>_aarch64.dmg"
```

## Windows Release Build

Repository में यह GitHub Actions workflow शामिल है:

```text
.github/workflows/build-windows.yml
```

Windows NSIS installer artifact बनाने के लिए इसे GitHub Actions से run करें।

## Tech Stack

- Tauri 2
- Rust 2021
- React 19
- TypeScript
- Vite
- Vitest

## License

MIT
