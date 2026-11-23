# Notas

App de escritorio para tomar notas en **Markdown**, organizadas por **proyectos** y
**etiquetas de colores**. Hecha con [Tauri](https://tauri.app) (Rust) + CodeMirror.
Funciona en Linux y macOS. Las notas son archivos `.md` normales: puedes abrirlas con
cualquier editor.

- Barra lateral: todas las notas, proyectos (en el disco y en este equipo) y etiquetas.
- Lista de notas con filtro (busca en títulos, etiquetas y contenido).
- Editor con resaltado y vista previa: **editar / dividido / vista** (`Ctrl+E`).
- Guardado automático. Las casillas `- [ ]` se marcan desde la vista previa.

## Dónde viven las notas

Igual que en Syntara, hay dos lugares y la app usa los dos a la vez:

| Lugar | Carpeta | Para qué |
|---|---|---|
| **Disco** | la que elijas en el disco externo, p. ej. `/run/media/jos/DISCO/Notas` o `/Volumes/DISCO/Notas` | notas que llevas entre la Mac y Linux |
| **Este equipo** | `~/Notas` | notas que solo quieres en esta computadora |

- **Configurar el disco**: barra lateral → *Disco* → **Usar un disco externo…** y elige la
  carpeta del disco (por ejemplo, la raíz del disco). Si esa carpeta no es ya una
  biblioteca de notas, se usa `<carpeta>/Notas` para no mezclarla con lo demás (como la
  carpeta de Syntara). Cada equipo recuerda su propia ruta, así que en la Mac y en Linux
  la eliges una vez en cada uno.
- **Si el disco no está conectado**, sus proyectos se ocultan y aparece «no conectado».
  La app no crea nada en esa ruta; al conectarlo, las notas aparecen solas.
- **Mover**: menú `⋯` de un proyecto → *Mover al disco* / *Mover a este equipo* (se lleva
  todas sus notas). Una nota suelta se mueve cambiando su proyecto en el selector de la
  nota.
- Una nota nueva sin proyecto va a la **Bandeja** del disco si está conectado; si no, a la
  de este equipo.
- Para usar el disco en los dos sistemas, formatéalo en **exFAT**. Los archivos `._*` y
  `.DS_Store` que crea macOS se ignoran.

### Estructura

```
Notas/
├── .md-notes            marca la carpeta como biblioteca
├── .etiquetas.toml      colores de las etiquetas
├── .papelera/           lo que eliminas (se puede recuperar a mano)
└── trabajo/             un proyecto = una carpeta
    ├── .proyecto.toml   nombre visible del proyecto
    └── mi-nota.md
```

Cada nota lleva un front matter con el título y las etiquetas (compatible con Obsidian y
otros editores). El nombre del archivo se toma del título:

```markdown
---
title: "Mi nota"
tags: [trabajo, rust]
---

Contenido en Markdown…
```

## Nota rápida

- Dentro de la app: barra lateral → **Nota rápida**, o `Ctrl+Shift+N`.
- Desde fuera: `md-notes --rapida` abre solo el modal (ventana pequeña flotante). Si la app ya
  está abierta, la misma instancia lo muestra; si no, el proceso termina al guardar o cerrar.
- Omarchy: el módulo `~/.config/omarchy/bar/modules/NotaRapida.qml` pone un botón en la barra
  (clic: nota rápida · clic derecho: abrir Notas). Se registra en `~/.config/omarchy/shell.json`:

  ```json
  { "id": "nota-rapida", "source": "~/.config/omarchy/bar/modules/NotaRapida.qml" }
  ```

## Agentes de IA (MCP)

`md-notes --mcp` arranca un servidor [MCP](https://modelcontextprotocol.io) por stdio (sin
ventanas), para que los agentes lean y dejen contexto en tus proyectos fuera del chat. No
hace falta tener la app abierta: trabaja sobre los mismos archivos, y la app muestra los
cambios al volver a enfocarla.

| Herramienta | Qué hace |
|---|---|
| `listar_proyectos` | proyectos con su id, lugar y número de notas |
| `contexto_proyecto` | todo el contexto de un proyecto: «Contexto», «Bitácora» y el resto de notas |
| `crear_proyecto` | crea el proyecto si no existe (con una nota «Contexto» inicial opcional) |
| `registrar` | agrega una entrada fechada a la «Bitácora» del proyecto |
| `crear_nota` / `actualizar_nota` | notas nuevas; reemplazar o anexar al cuerpo de una existente |
| `leer_nota` / `listar_notas` / `buscar` | lectura y búsqueda |

Los proyectos se pueden nombrar por su nombre visible («Mi app») o por su id
(`equipo:mi-app`). Un proyecto nuevo va al disco si está conectado; si no, a este equipo.

Registrarlo en cada agente (tras `scripts/install.sh`; en la Mac, el binario está en
`/Applications/Notas.app/Contents/MacOS/md-notes`):

```sh
claude mcp add --scope user notas -- ~/.local/bin/md-notes --mcp   # Claude Code
codex mcp add notas -- ~/.local/bin/md-notes --mcp                 # Codex
```

opencode (`~/.config/opencode/opencode.json`):

```json
"mcp": { "notas": { "type": "local", "command": ["/home/<tú>/.local/bin/md-notes", "--mcp"] } }
```

Cursor, Claude Desktop y otros usan el formato habitual:

```json
{ "mcpServers": { "notas": { "command": "/home/<tú>/.local/bin/md-notes", "args": ["--mcp"] } } }
```

Para que un agente lo use sin pedírselo, agrega a su `CLAUDE.md` / `AGENTS.md`:
*«Al empezar, llama a `contexto_proyecto` de Notas con el nombre de este proyecto; al
terminar, deja lo hecho y lo pendiente con `registrar`.»*

## Instalar

```sh
scripts/install.sh   # Linux: ~/.local/bin + menú de aplicaciones · macOS: /Applications/Notas.app
```

En la Mac necesitas Rust (`rustup`), Node y las Xcode Command Line Tools
(`xcode-select --install`).

## Desarrollo

```sh
npm install
npm run tauri dev                      # app con recarga en caliente
cd src-tauri && cargo test             # pruebas del backend
```

Variables útiles para probar sin tocar tus notas: `MD_NOTES_DIR` (carpeta de este
equipo), `MD_NOTES_DISCO` (carpeta del disco) y `MD_NOTES_CONFIG` (carpeta de ajustes).

| Atajo | Acción |
|---|---|
| `Ctrl+N` | nueva nota |
| `Ctrl+E` | editar / dividido / vista |
| `Ctrl+K` | filtrar notas |
| `Ctrl+S` | guardar ahora |
