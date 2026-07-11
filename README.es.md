# Prompt Drawer

![Demostración de Prompt Drawer](docs/prompt-drawer-demo.gif)

**Leer en:** [English](README.md) | [简体中文](README.zh-CN.md) | [हिन्दी](README.hi.md) | **Español** | [العربية](README.ar.md)

Todos merecen una biblioteca personal de prompts.

Prompt Drawer es una biblioteca local de prompts para Codex, Cursor y CLI. Selecciona un prompt guardado y Prompt Drawer lo introduce en el campo activo y lo envía en una sola acción. Sin copiar, pegar ni pulsar Return repetidamente. Cambia a **Solo insertar** cuando quieras revisar el contenido antes de enviarlo.

Crea grupos de prompts para enviar una secuencia de prompts en orden.

## Uso

1. Crea prompts individuales, grupos de prompts y categorías en tu biblioteca.
2. Pon el foco en el campo de entrada donde quieras trabajar.
3. Abre Prompt Drawer y elige un prompt o un grupo.

## Descarga

Descarga el DMG más reciente para macOS Apple Silicon o el instalador x64 para Windows desde [GitHub Releases](https://github.com/Imd11/prompt-drawer/releases/latest).

En macOS, se necesita permiso de Accesibilidad para insertar y enviar prompts en las aplicaciones compatibles.

## Tu biblioteca de prompts

Prompt Drawer guarda tu biblioteca localmente y nunca sube el contenido de los prompts a un servidor. Importa o exporta bibliotecas JSON cuando necesites una copia de seguridad o quieras mover tus prompts.

Hay bibliotecas de ejemplo en:

- `examples/prompts/prompts-zh.json`
- `examples/prompts/prompts-en.json`

## Desarrollo

```bash
npm install
npm test
npm run tauri -- build
```

## Licencia

MIT
