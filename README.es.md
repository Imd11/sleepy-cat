# Prompt Drawer

**Leer en:** [English](README.md) | [简体中文](README.zh-CN.md) | [हिन्दी](README.hi.md) | **Español** | [العربية](README.ar.md)

Prompt Drawer es una biblioteca local de prompts de escritorio creada para Codex. Mantiene un botón flotante cerca del campo de entrada activo de Codex, abre un selector compacto e inserta el prompt seleccionado donde estás trabajando.

La aplicación está construida con Tauri, React y Rust. Los datos de prompts se almacenan localmente en el equipo del usuario.

## Funciones

- Botón flotante de prompts con una lista compacta.
- Administrador local de prompts para prompts individuales y secuencias agrupadas.
- Soporte de categorías para organizar colecciones de prompts.
- Modos de inserción: solo pegar y pegar y enviar.
- Importación y exportación de bibliotecas de prompts como JSON.
- Modo opcional de enlace y sincronización para mantener sincronizado un archivo JSON elegido con las ediciones hechas en la app.
- Almacenamiento local-first; los datos de prompts no se suben a ningún servidor.
- Empaquetado como app de barra de menús en macOS con firma Developer ID y notarización.
- Construcción del instalador de Windows mediante GitHub Actions.

## Descarga

La versión más reciente está disponible en GitHub:

https://github.com/Imd11/prompt-drawer/releases/latest

Paquetes disponibles actualmente:

- DMG para macOS Apple Silicon
- Instalador Windows x64

En macOS, Prompt Drawer requiere permiso de Accesibilidad para pegar texto y enviarlo en otras aplicaciones.

## Bibliotecas de prompts de ejemplo

Este repositorio incluye dos bibliotecas de prompts de ejemplo:

- `examples/prompts/prompts-zh.json`
- `examples/prompts/prompts-en.json`

Contienen un conjunto de prompts para un flujo de trabajo de desarrollo, con planificación, ejecución, revisión, depuración y publicación.

Para usar una de ellas:

1. Abre Prompt Drawer.
2. Ve al administrador de prompts.
3. Haz clic en Import.
4. Selecciona uno de los archivos JSON desde `examples/prompts/`.
5. Elige importarlo como copia interna de la app o enlazar y sincronizar el archivo JSON seleccionado.

Importar como copia reemplaza la biblioteca interna actual, así que exporta tus prompts actuales primero si quieres conservar una copia de seguridad. Si eliges enlazar y sincronizar, Prompt Drawer guarda la ruta del archivo seleccionado y escribe las futuras ediciones hechas en la app de vuelta en ese JSON. La app no escanea tu escritorio ni elige automáticamente un archivo de prompts.

## Datos locales

Prompt Drawer almacena los datos del usuario localmente.

En macOS, los prompts se almacenan en:

```text
~/Library/Application Support/local.promptpicker.dev/prompts.json
```

La configuración se almacena junto a ellos:

```text
~/Library/Application Support/local.promptpicker.dev/settings.json
```

Exportar prompts crea una copia de seguridad JSON separada. No cambia la ubicación de almacenamiento predeterminada de la app.

Cuando importas un JSON, Prompt Drawer usa el `prompts.json` interno por defecto. Enlazar y sincronizar es una opción explícita por archivo importado, y se puede desactivar desde el administrador de prompts sin eliminar el JSON externo.

## Desarrollo

Instalar dependencias:

```bash
npm install
```

Ejecutar el servidor de desarrollo del frontend:

```bash
npm run dev
```

Ejecutar pruebas:

```bash
npm test
```

Construir el frontend:

```bash
npm run build
```

Construir la app Tauri:

```bash
npm run tauri -- build
```

## Build de release para macOS

La configuración de Tauri está preparada para firma Developer ID. Para una release pública de macOS, construye, notariza y aplica staple al DMG:

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

Verificar que Gatekeeper lo acepta:

```bash
spctl --assess --type open --context context:primary-signature --verbose=4 \
  "src-tauri/target/release/bundle/dmg/Prompt Drawer_<version>_aarch64.dmg"
```

## Build de release para Windows

El repositorio incluye un workflow de GitHub Actions:

```text
.github/workflows/build-windows.yml
```

Ejecútalo desde GitHub Actions para producir el artifact del instalador NSIS de Windows.

## Stack tecnológico

- Tauri 2
- Rust 2021
- React 19
- TypeScript
- Vite
- Vitest

## Licencia

MIT
