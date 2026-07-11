# Prompt Drawer

**اقرأ هذا باللغة:** [English](README.md) | [简体中文](README.zh-CN.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | **العربية**

Prompt Drawer هو مكتبة prompts محلية لسطح المكتب، مصممة لـ Codex. يحتفظ التطبيق بزر عائم قرب حقل إدخال Codex النشط، ويفتح لوحة prompts مدمجة، ثم يدرج prompt الذي تختاره حيث تعمل.

التطبيق مبني باستخدام Tauri و React و Rust. يتم تخزين بيانات prompts محلياً على جهاز المستخدم.

## الميزات

- زر prompts عائم مع قائمة prompts مدمجة.
- مدير prompts محلي يدعم prompts مفردة وتسلسلات prompts مجمعة.
- دعم التصنيفات لتنظيم مجموعات prompts.
- وضعا إدراج: اللصق فقط، واللصق ثم الإرسال.
- استيراد وتصدير مكتبات prompts بصيغة JSON.
- وضع اختياري للربط والمزامنة للحفاظ على ملف JSON تختاره متزامناً مع التعديلات التي تجريها داخل التطبيق.
- تخزين محلي أولاً؛ لا يتم رفع بيانات prompts إلى خادم.
- حزمة تطبيق شريط القوائم على macOS مع توقيع Developer ID والتوثيق notarization.
- بناء مثبت Windows عبر GitHub Actions.

## التنزيل

أحدث إصدار متاح على GitHub:

https://github.com/Imd11/prompt-drawer/releases/latest

الحزم المتاحة حالياً:

- ملف DMG لأجهزة macOS Apple Silicon
- مثبت Windows x64

على macOS، يحتاج Prompt Drawer إلى إذن Accessibility حتى يستطيع لصق النص وإرساله داخل تطبيقات أخرى.

## مكتبات prompts تجريبية

يتضمن هذا المستودع مكتبتين تجريبيتين من prompts:

- `examples/prompts/prompts-zh.json`
- `examples/prompts/prompts-en.json`

تحتويان على مجموعة prompts لسير عمل تطويري يشمل التخطيط والتنفيذ والمراجعة وتصحيح الأخطاء والإصدار.

لاستخدام إحداهما:

1. افتح Prompt Drawer.
2. انتقل إلى مدير prompts.
3. اضغط Import.
4. اختر أحد ملفات JSON من `examples/prompts/`.
5. اختر استيراده كنسخة داخلية للتطبيق أو ربط ملف JSON المحدد ومزامنته.

الاستيراد كنسخة يستبدل مكتبة prompts الداخلية الحالية، لذلك صدّر prompts الحالية أولاً إذا كنت تريد الاحتفاظ بنسخة احتياطية. إذا اخترت الربط والمزامنة، يحفظ Prompt Drawer مسار الملف المحدد ويكتب أي تعديلات لاحقة تجريها داخل التطبيق إلى ملف JSON نفسه. لا يفحص التطبيق سطح المكتب ولا يختار ملف prompts تلقائياً.

## البيانات المحلية

يخزن Prompt Drawer بيانات المستخدم محلياً.

على macOS، يتم تخزين prompts هنا:

```text
~/Library/Application Support/local.promptpicker.dev/prompts.json
```

وتخزن الإعدادات بجانبها:

```text
~/Library/Application Support/local.promptpicker.dev/settings.json
```

تصدير prompts ينشئ نسخة احتياطية منفصلة بصيغة JSON. هذا لا يغير موقع التخزين الافتراضي للتطبيق.

عند استيراد ملف JSON، يستخدم Prompt Drawer ملف `prompts.json` الداخلي بشكل افتراضي. الربط والمزامنة خيار صريح لكل ملف مستورد، ويمكن إزالته من مدير prompts دون حذف ملف JSON الخارجي.

## التطوير

تثبيت الاعتماديات:

```bash
npm install
```

تشغيل خادم تطوير الواجهة:

```bash
npm run dev
```

تشغيل الاختبارات:

```bash
npm test
```

بناء الواجهة:

```bash
npm run build
```

بناء تطبيق Tauri:

```bash
npm run tauri -- build
```

## بناء إصدار macOS

تم إعداد Tauri للتوقيع باستخدام Developer ID. لإصدار macOS عام، ابن DMG ثم وثقه notarize ثم طبق staple:

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

تحقق من قبول Gatekeeper:

```bash
spctl --assess --type open --context context:primary-signature --verbose=4 \
  "src-tauri/target/release/bundle/dmg/Prompt Drawer_<version>_aarch64.dmg"
```

## بناء إصدار Windows

يتضمن المستودع workflow في GitHub Actions:

```text
.github/workflows/build-windows.yml
```

شغله من GitHub Actions لإنتاج artifact مثبت Windows NSIS.

## التقنيات

- Tauri 2
- Rust 2021
- React 19
- TypeScript
- Vite
- Vitest

## الترخيص

MIT
