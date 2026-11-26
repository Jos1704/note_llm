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

### Widget del notch (macOS)

Con la app abierta, pasa el ratón por el notch o pulsa **`⌥Space`** desde cualquier app: se
despliega un panel para capturar una nota.

- Con el atajo, el cursor ya está en el texto; con el ratón, haz clic para escribir.
- `⌘↩` guarda (en la Bandeja o el proyecto elegido; recuerda el último) y te devuelve a la app
  en la que estabas. `Esc` (o el atajo otra vez) cierra.
- **Prefijos:** `#etiqueta` y `@proyecto` en el título o en la primera línea etiquetan la nota y
  la mandan a ese proyecto (se quitan del texto). Mientras escribes salen sugerencias; `Tab`
  acepta la primera. `# Título` con espacio sigue siendo un encabezado.
- **Botones de captura** (abajo a la izquierda):
  - *Portapapeles*: pega texto o imagen.
  - *Pestaña*: inserta `[título](url)` de la pestaña activa (Safari, Chrome, Arc, Brave, Edge,
    Vivaldi, Opera). macOS pide permiso de **Automatización** la primera vez.
  - *Selección*: el texto seleccionado en la app en la que estabas. Requiere permiso de
    **Accesibilidad** (Ajustes del Sistema → Privacidad y seguridad → Accesibilidad → Notas).
  - *Captura*: eliges una zona de la pantalla; se guarda la imagen y su texto (OCR de macOS, sin
    conexión) en un bloque plegable. macOS pide permiso de **Grabación de pantalla** la primera vez.
- También puedes pegar imágenes con `⌘V`. Las imágenes se guardan en `assets/` junto a la nota.
- Si sacas el ratón sin haber escrito, se pliega solo. Si cierras con texto sin guardar, el
  borrador se conserva para la próxima vez.
- En pantallas sin notch (monitor externo, tapa cerrada) la zona activa es una franja fina
  arriba al centro.
- **Segundo plano:** `⌘Q` o cerrar la ventana solo la ocultan; la app sale del Dock y el notch y
  el atajo siguen funcionando. Para volver: icono de Notas en la barra de menús → *Abrir Notas*
  (o abrir la app otra vez). Para cerrarla del todo: `⌥⌘Q`, o *Salir de Notas* en ese icono.

### Panel en Linux (Wayland)

En Linux no hay notch ni la app puede colocarse sola arriba de la pantalla, así que el panel es una
configuración de [Quickshell](https://quickshell.org) (`linux/quickshell/notas/shell.qml`, que
`scripts/install.sh` copia a `~/.config/quickshell/notas/`). Funciona aunque Notas esté cerrada.

- Pasa el ratón por el **borde superior, al centro** de la pantalla y baja el panel (en cada
  monitor). Si sacas el ratón sin escribir, se pliega.
- `Ctrl+↩` guarda, `Esc` cierra. Mismos prefijos `#etiqueta` / `@proyecto`, con sugerencias
  mientras escribes (`Tab` acepta la primera), y botones de portapapeles, selección (primaria) y
  captura (`grim` + `slurp`, OCR con `tesseract`). `Ctrl+V` en el texto también pega imágenes.
- Usa la paleta y el modo de la app: la app los copia a `~/.config/md-notes/tema.json` y el panel
  los cambia al momento. En modo *Sistema* sigue a `gsettings … color-scheme`.
- En Omarchy, `scripts/install.sh` también instala el botón de la barra
  (`~/.config/omarchy/bar/modules/NotaRapida.qml`): clic abre el panel, clic derecho la app.
- En Hyprland con Omarchy (`~/.config/hypr/autostart.lua` y `bindings.lua`; `Alt+Space` es del
  dictado):

  ```lua
  o.exec_on_start("qs -c notas -d")
  o.bind("SUPER + ALT + N", "Nota rápida", "~/.local/bin/md-notes --capturar")
  ```

  Con la configuración clásica (`hyprland.conf`): `exec-once = qs -c notas -d` y
  `bind = SUPER ALT, N, exec, md-notes --capturar`.

  `md-notes --capturar` abre el panel en el monitor activo si está corriendo y, si no, la ventana
  de nota rápida.
- Guarda con la línea de comandos, que también sirve para scripts: `md-notes --proyectos`,
  `--etiquetas`, `md-notes --guardar-rapida [--titulo T] [--proyecto ID] -- texto`, `--portapapeles`,
  `--seleccion`, `--captura-pantalla` (todas responden JSON).

**Cambiar el atajo:** en `~/Library/Application Support/md-notes/ajustes.json` (Linux:
`~/.config/md-notes/ajustes.json`) añade `"atajo": "ctrl+alt+n"` y reinicia la app.
`md-notes --capturar` hace lo mismo que el atajo (útil para Raycast o, en Hyprland, un `bind`,
ya que Wayland no permite atajos globales a las apps). En Linux abre la ventana de nota rápida
(misma captura de prefijos, portapapeles, selección primaria y captura con `grim`/`slurp`; OCR si
está instalado `tesseract`).

## Enlaces, tareas y plantillas

- **Enlaces entre notas:** escribe `[[` y elige una nota (también `[[Nota|texto]]` y
  `[[Nota#sección]]`). En la vista previa son enlaces; en el editor, `⌘`‑clic. Si la nota no
  existe se muestra tenue y, al abrirlo, se crea. Bajo cada nota, **Enlazan aquí** lista las
  notas que la mencionan, con la línea en contexto.
- Si cambias el título de una nota enlazada, la app ofrece actualizar los `[[…]]` de las demás.
- **Tareas:** cualquier `- [ ] …` de cualquier nota aparece en **Tareas** (barra lateral), por
  proyecto, con filtros Pendientes, Hoy, Atrasadas, Sin fecha y Hechas. Fecha opcional con
  `📅 2026-11-30` o `@2026-11-30`. Clic en el círculo la marca; clic en el texto abre la nota en
  esa línea.
- **Plantillas:** botón junto a «Nueva nota» o `/` en el editor. Son archivos `.md` en
  `.plantillas/` (en `~/Notas` para todos los proyectos, o dentro de un proyecto solo para él).
  La primera vez se crean *Reunión*, *Bug* e *Idea*. Admiten `title` y `tags` en el front matter y
  las variables `{{fecha}}`, `{{hora}}`, `{{proyecto}}`, `{{titulo}}` y `{{cursor}}`.

## Notas parecidas y grafo

- **Notas parecidas:** al pie de cada nota, junto a «Enlazan aquí», salen las notas que hablan de
  lo mismo aunque no estén enlazadas, con las palabras que comparten. Se calcula en local
  (TF‑IDF: estadística de palabras, sin IA ni red) y solo se relee lo que cambió.
- **Grafo** (barra lateral): mapa de toda la biblioteca. Los proyectos son los nodos grandes y sus
  notas orbitan alrededor; las líneas entre proyectos son más gruesas cuanto más se relacionan.
  - Líneas: enlaces `[[…]]` (sólidas), etiqueta compartida (punteadas, del color de la etiqueta),
    notas parecidas (finas) y nota → proyecto (tenues). Cada tipo se activa o desactiva arriba.
  - Filtros por lugar, fecha de modificación y etiqueta; el buscador centra el nodo.
  - Rueda o pellizco: zoom (alejado solo se ven los proyectos). Arrastrar: mover el mapa o un
    nodo. Clic: resalta sus vecinos y, en una nota, muestra una ficha. Doble clic: abre la nota.
    `Esc` cierra. Las posiciones se recuerdan.

## Editor

- **Adjuntos:** pega (`⌘V`) o arrastra imágenes y archivos al editor. Se guardan en `assets/`
  junto a la nota (sin pisar nada) y se inserta `![…](assets/…)` o `[…](assets/…)`. En la vista
  previa, un clic en un archivo enlazado lo abre con su app. Al mover una nota a otro proyecto
  sus adjuntos van con ella (se copian si otra nota también los usa) y al borrarla, los que
  nadie más usa van a la papelera.
- **Fórmulas:** `$e^{i\pi}+1=0$` en línea y `$$ … $$` en bloque (KaTeX, sin conexión). Un `$`
  seguido de espacio o cifra no cuenta, así que «$5 o $10» queda como texto.
- **Diagramas:** bloques ` ```mermaid ` se dibujan en la vista previa con el tema de la app.
  Mermaid solo se carga si la nota tiene algún diagrama.
- **Modo enfoque** (`⇧⌘F` o el botón de la barra): oculta la barra lateral y la lista, centra el
  texto y atenúa todo menos el párrafo en el que escribes. `Esc` sale.
- **Versiones** (botón del reloj): instantáneas automáticas al editar (como mucho cada 10
  minutos) y al cambiar de nota. Muestra las diferencias lado a lado con el texto actual y
  permite restaurar (el texto actual se guarda antes, así que se puede deshacer). Se conservan
  todas las de 24 h, una por día durante 30 días y una por semana después, en
  `.historial/` de cada lugar (sin git, funciona también en el disco).

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
