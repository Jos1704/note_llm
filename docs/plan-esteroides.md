# Plan: Notas «con esteroides» — captura, organización, editor, IA y grafo

> Plan para revisar antes de implementar. Se trabaja por fases; cada una deja la app usable.

## Decisiones del usuario (2026-11-23)

- Modelo: **solo Haiku 5.5** por defecto.
- Atajo global: **`⌥Space`**.
- Orden: el del plan (1 → 2 → 5.1 → 5.2 → 4 → 3).

## Alcance acordado (2026-11-23)

| Bloque | Entra | Fuera |
|---|---|---|
| Captura | atajo global, capturar lo que tienes delante, captura de pantalla con OCR, prefijos `#etiqueta` / `@proyecto` | notas de voz (Whisper) |
| IA | bandeja inteligente, preguntar a tus notas, resumen por proyecto, acciones sobre la selección, notas relacionadas | modelos locales |
| Organización | wikilinks + backlinks, vista de tareas, plantillas, **grafo con los proyectos como nodos principales** | nota diaria |
| Editor | imágenes y adjuntos, Mermaid + KaTeX, modo enfoque, historial de versiones | — |
| Sincronización | **solo plan** (Fase 6), sin implementar | — |

## Decisiones de base

- **IA en la nube con la API de Claude, llamada desde Rust** (`ureq`, que ya está en el proyecto). Rust no tiene SDK oficial, así que va por HTTP directo a `POST https://api.anthropic.com/v1/messages` con `x-api-key` y `anthropic-version: 2026-10-21`. La clave nunca pasa por el webview.
- **Clave en el llavero del sistema** (crate `keyring`: Llavero en macOS, Secret Service en Linux). Se configura en *Ajustes → IA*.
- **Modelo:** `claude-haiku-5-5` para todo ($0.10 / $0.50 por millón de tokens de entrada / salida), elegible en ajustes (p. ej. `claude-opus-5-5`, $4 / $20, si alguna función necesita más calidad). Haiku no tiene reintento automático en otro modelo cuando rechaza una petición: se muestra el motivo (`stop_reason: "refusal"`).
- **Sin embeddings.** Anthropic no ofrece embeddings y no queremos modelos locales. La «cercanía» entre notas se calcula en local con **TF‑IDF** (estadística de palabras: no es un modelo, no descarga nada, es instantáneo). La IA se usa solo para lo que aporta: decidir relaciones, responder, resumir.
- **Privacidad:** el contenido de las notas se envía a Anthropic solo cuando usas una función de IA. Cada proyecto puede marcarse como **«privado: sin IA»** (`.proyecto.toml`), y entonces nunca se envía.
- Todo lo que se genera se guarda como archivos en la biblioteca (`.relaciones.json`, `.historial/`, `assets/`), así viaja con el disco igual que las notas.

---

## Fase 1 — Captura

**1.1 Atajo global** (`tauri-plugin-global-shortcut`)
- Abre el panel del notch desde cualquier app y le da el foco directamente al texto. Por defecto `⌥Space` (choca con Raycast/Alfred si los usas); configurable en ajustes.
- En Linux, el mismo atajo abre la ventana `--rapida`.
- `notch.rs`: nuevo `abrir_con_foco()`, que reutiliza la lógica de apertura (hoy la dispara solo el hover) y además hace la ventana activa.

**1.2 Prefijos en el texto**
- `#etiqueta` y `@proyecto` en la **primera línea** se convierten en etiquetas y proyecto, y se quitan del cuerpo.
- Autocompletado de proyectos y etiquetas mientras escribes (lista pequeña sobre el textarea).
- Va en `crearNotaRapida` (`api.ts`), así lo usan el notch, la ventana rápida y el modal de la app.

**1.3 Capturar lo que tienes delante** (botones en el pie del panel)
- **Portapapeles** (`tauri-plugin-clipboard-manager`): pega el texto, o la imagen si es una imagen.
- **Pestaña activa del navegador:** AppleScript para Safari, Chrome, Arc y Brave; inserta `[título](url)`. macOS pide permiso de *Automatización* la primera vez.
- **Texto seleccionado en otra app:** API de Accesibilidad (`AXSelectedText`). Requiere permiso de *Accesibilidad*; si no se concede, el botón explica cómo darlo.

**1.4 Captura de pantalla → nota**
- `screencapture -i` (recorte interactivo del sistema) → imagen en `assets/`.
- **OCR con Vision de macOS:** viene con el sistema, sin descargas ni carga apreciable. No es un modelo que tengas que correr tú.
- El texto extraído va bajo la imagen (o en un bloque plegable) y entra en la búsqueda.
- En Linux: `grim`/`slurp` para la captura; OCR solo si `tesseract` está instalado.

## Fase 2 — Organización

**2.1 Índice de enlaces** (backend, base del grafo y de los backlinks)
- `store.rs`/`biblioteca.rs`: al leer las notas se extraen `[[Título]]` y `[[Título|texto]]`, los enlaces Markdown a otras notas y las etiquetas.
- Índice en memoria `id → {salientes, entrantes}`, que se refresca al guardar.
- Comandos `enlaces(id)` y `grafo()`.

**2.2 Wikilinks + backlinks**
- Editor: al escribir `[[` aparece el autocompletado de títulos (nuevo `CompletionSource` en `editor-tools.ts`). Con `⌘`+clic se abre la nota.
- Vista previa: regla de markdown-it que convierte `[[…]]` en enlace interno. Si la nota no existe, el enlace sale atenuado y un clic la crea.
- Panel **«Enlazan aquí»** al pie de la nota, con el contexto de la línea que enlaza.
- Al renombrar una nota, se ofrece actualizar los `[[…]]` que apuntan a ella.

**2.3 Vista «Tareas»**
- Entrada en la barra lateral: todas las `- [ ]` de todas las notas, agrupadas por proyecto.
- Fechas `📅 2026-12-01` o `@2026-12-01`. Filtros: pendientes, hoy, atrasadas, sin fecha.
- Marcar una tarea aquí reescribe la línea en su nota. Se reutiliza la lógica de casillas de la vista previa, pero en el backend, para que también funcione sin abrir la nota.

**2.4 Plantillas**
- Carpeta `.plantillas/` en la biblioteca, más `.plantillas/` por proyecto (la del proyecto tiene prioridad).
- Variables `{{fecha}}`, `{{hora}}`, `{{proyecto}}`, `{{titulo}}`.
- Se usan desde el menú `/` (*Plantilla…*) y al crear una nota (botón ▾ junto a «Nueva nota»).
- Se incluyen de ejemplo: Reunión, Bug, Idea.

## Fase 3 — Editor

**3.1 Imágenes y adjuntos**
- Pegar o arrastrar guarda el archivo en `<proyecto>/assets/<slug>-<n>.<ext>` e inserta `![](assets/…)`.
- La vista previa los resuelve con el protocolo de recursos de Tauri. Hay que ajustar la CSP (`img-src`) y el `assetProtocol.scope`.
- Al borrar o mover una nota, sus adjuntos la acompañan (si ninguna otra nota los usa).

**3.2 Mermaid + KaTeX**
- Bloques ` ```mermaid ` → diagrama. `mermaid` se empaqueta en local por la CSP y se carga solo cuando hace falta (es pesado), con el tema de la app.
- `$…$` y `$$…$$` → KaTeX (`markdown-it-katex` o una regla propia), con sus fuentes empaquetadas.

**3.3 Modo enfoque** (`⌘⇧F`)
- Oculta la barra lateral y la lista, centra el texto (~70 caracteres por línea) y atenúa los párrafos que no tienen el cursor (decoración de CodeMirror).
- `Esc` sale.

**3.4 Historial de versiones**
- Al guardar, si pasaron más de 10 minutos desde la última instantánea (o al cerrar la nota), se copia a `.historial/<id>/<fecha>.md`.
- Se conservan: todas las de las últimas 24 h, una por día durante 30 días y una por semana después.
- Panel **«Versiones»** con diferencias lado a lado (`@codemirror/merge`) y botón «Restaurar».
- Sin git: así funciona igual en el disco exFAT.

## Fase 4 — IA (Claude API)

**4.0 Base**
- `src-tauri/src/ia.rs`: cliente con `ureq`, lectura de la clave del llavero y reintentos con espera creciente en 429/5xx/529.
- Streaming SSE: se leen los eventos `content_block_delta` y se reenvían al webview como eventos de Tauri (`ia:delta`, `ia:fin`, `ia:error`), en un hilo aparte.
- Se comprueba siempre `stop_reason` (`refusal`, `max_tokens`) antes de usar el texto.
- **Caché de prompts:** instrucciones fijas primero y `cache_control` en el bloque de notas. Así preguntas seguidas sobre el mismo proyecto cuestan ~10% en la parte cacheada.
- *Ajustes → IA*: clave, modelo, «probar conexión», gasto acumulado del mes (sumando `usage` de cada respuesta) y proyectos privados.

**4.1 Bandeja inteligente**
- Botón «Organizar Bandeja». Se envía la lista de proyectos y etiquetas existentes más cada nota de la Bandeja (título + primeros ~1500 caracteres).
- **Salida estructurada** (`output_config.format` con esquema JSON): `{nota, proyecto, etiquetas[], confianza, motivo}`.
- Se muestra como lista de sugerencias con casillas: aceptar todas, aceptar una por una o cambiar el proyecto antes de aceptar. **Nunca mueve nada sin tu confirmación.**
- Opción: sugerir al guardar desde el notch (una sola nota, en segundo plano).

**4.2 Preguntar a tus notas** (`⌘⇧K`, panel lateral)
- **Recuperación local:** búsqueda BM25 sobre el índice TF‑IDF (Fase 5.1), más las notas enlazadas a las mejores. Se toman ~15–30 notas, o un proyecto entero si lo eliges como ámbito.
- Las notas se envían como bloques `document` con **citas activadas** (`citations: {enabled: true}`). La respuesta llega por streaming y cada cita es un enlace que abre la nota en el párrafo citado.
- Conversación de seguimiento en el mismo panel. Botón «Guardar como nota».

**4.3 Resumen de proyecto**
- Menú `⋯` del proyecto → «Resumir». Usa «Contexto», «Bitácora» y las notas recientes; periodo: última semana / último mes / todo.
- El resultado se puede guardar como nota «Resumen <fecha>» o anexarse a la Bitácora (la misma que alimentan los agentes vía MCP).

**4.4 Acciones sobre la selección**
- Nuevas entradas en la barra flotante y en el menú `/`: Resumir, Reescribir (más claro / más corto / más formal), Traducir (ES↔EN), Convertir en lista de tareas, Continuar.
- **Streaming dentro del editor:** el texto aparece marcado como propuesta (decoración) y se acepta con `Tab` o se descarta con `Esc`. Todo queda como una sola acción de deshacer.

**4.5 Relaciones sugeridas por IA** (alimentan el grafo)
- Por proyecto, bajo demanda («Descubrir relaciones»): se envían título + extracto de cada nota más los pares candidatos que propone TF‑IDF.
- Claude confirma o descarta cada relación y la etiqueta: «amplía», «contradice», «depende de», «mismo tema».
- Se guarda en `.relaciones.json` del proyecto, con la fecha de modificación de cada nota. Solo se recalcula lo que cambió.

## Fase 5 — Grafo de relaciones

**5.1 Índice de similitud local** (`indice.rs`)
- TF‑IDF con palabras vacías en español e inglés y normalización de acentos. Se recalcula de forma incremental al guardar.
- Da **notas relacionadas** (panel al pie de la nota, junto a «Enlazan aquí») y los candidatos para 4.2 y 4.5.

**5.2 Vista «Grafo»** (entrada en la barra lateral, a pantalla completa)
- **Nodos principales: proyectos.** Grandes, con color propio, y su tamaño según el número de notas. Las notas son nodos pequeños orbitando su proyecto.
- **Aristas por tipo,** con estilo distinto y filtros activables:
  - pertenencia nota → proyecto (tenue);
  - wikilink / enlace (sólida);
  - etiqueta compartida (punteada, del color de la etiqueta);
  - similitud TF‑IDF por encima de un umbral (fina);
  - relación IA de 4.5 (con su etiqueta al pasar el ratón).
- **Aristas proyecto ↔ proyecto** = suma de las relaciones entre sus notas. Grosor según la fuerza: se ve de un vistazo qué proyectos se tocan.
- Interacción:
  - Con zoom alejado se ven solo los proyectos; al acercarse aparecen las notas.
  - Clic en un proyecto: se expande y se resaltan sus vecinos. Clic en una nota: vista previa flotante; doble clic: la abre.
  - Filtros por etiqueta, por lugar (disco / este equipo) y por fecha. Búsqueda que centra el nodo.
- **Técnica:** `d3-force` (empaquetado; la CSP impide CDNs) dibujando en `<canvas>` para que vaya fluido con miles de nodos. Las posiciones se guardan en `localStorage` para que el mapa no se reordene cada vez.
- También una **mini‑vista** por nota: grafo local de 1–2 saltos al pie de la nota.

## Fase 6 — Sincronización (solo plan; se decide cuando elijas la nube)

**Dato clave:** la app ya trata «Disco» como *una carpeta cualquiera*. La forma más barata de sincronizar es **apuntar «Disco» a una carpeta sincronizada en la nube**, sin escribir código de red.

| Opción | Cómo | Mac + Linux | Coste | Notas |
|---|---|---|---|---|
| **Syncthing** | carpeta P2P entre tus equipos | ✅ | gratis | no necesita nube; los dos equipos deben coincidir encendidos alguna vez |
| **Dropbox** | «Disco» → `~/Dropbox/Notas` | ✅ (cliente oficial) | 2 GB gratis | el más fiable en Linux |
| **iCloud Drive** | «Disco» → `~/Library/Mobile Documents/…` | ⚠️ Linux sin cliente oficial | 5 GB gratis | ideal si solo fueran Macs |
| **Google Drive** | carpeta con Drive para escritorio / `rclone` en Linux | ⚠️ `rclone bisync` | 15 GB gratis | en Linux funciona por sondeo, no al instante |
| **Git remoto** (GitHub/Gitea privado) | commit + push automáticos en la app | ✅ | gratis | historial completo; la app tendría que resolver conflictos |
| **S3 / R2 / B2** | sincronización propia contra un bucket | ✅ | céntimos | lo más trabajo: reimplementar la sincronización |

**Lo que habría que añadir en la app sea cual sea la opción:**
1. **Detección de conflictos:** al guardar, si el archivo cambió en disco desde que se abrió (por `mtime` + hash), no se sobrescribe. Se crea `nota (conflicto Mac).md` y se avisa con un panel de diferencias (reutiliza 3.4).
2. **Vigilar cambios externos** (`notify`): refrescar la lista y la nota abierta cuando la sincronización trae cambios, sin esperar a que la ventana recupere el foco.
3. **Ignorar archivos temporales** de cada servicio (`.dropbox`, `*.icloud`, `.stversions`, `.syncthing.*`) en el escaneo, como ya se hace con `._*` y `.DS_Store`.
4. **Varias carpetas «Disco»** (p. ej. disco externo + nube) si quisieras conservar las dos.

**Recomendación provisional:** Syncthing si no quieres depender de una nube, o Dropbox si quieres el camino más simple en Mac + Linux. Las dos funcionan con solo los puntos 1–3.

---

## Orden y tamaño estimado

| Fase | Tamaño | Depende de |
|---|---|---|
| 1 Captura | M | — |
| 2 Organización | L | — |
| 3 Editor | L | — |
| 5.1 Índice TF‑IDF | S | 2.1 |
| 4 IA | L | 5.1 (para 4.2 y 4.5) |
| 5.2 Grafo | L | 2.1, 5.1 (4.5 opcional) |
| 6 Sincronización | — | decisión pendiente |

Orden propuesto: **1 → 2 → 5.1 → 5.2 (grafo sin IA) → 4 → 3**. El grafo ya es útil con enlaces, etiquetas y similitud, y la IA lo enriquece después. El editor (3) es independiente y puede intercalarse.

## Nuevas dependencias

- Rust: `tauri-plugin-global-shortcut`, `tauri-plugin-clipboard-manager`, `keyring`, `notify` (fase 6); `objc2-vision` para el OCR en macOS.
- npm: `d3-force`, `mermaid`, `katex`, `@codemirror/merge`.

## Verificación por fase

- `cargo test`: índice de enlaces, TF‑IDF, prefijos, rotación del historial, parseo de tareas, construcción de peticiones a la IA (sin red: se comprueba el JSON que se enviaría).
- `npm run build`.
- Manual: cada función en la app instalada (`scripts/install.sh`).
- IA: lote pequeño de prueba con tu clave antes de usar la Bandeja completa (cuesta dinero real).

## Progreso

- [x] **Fase 1 — Captura** (2026-11-23)
  - `⌥Space` (configurable con `"atajo"` en `ajustes.json`) y `md-notes --capturar`: panel del notch con foco; otra pulsación lo cierra. Al cerrar se reactiva la app anterior (`NSRunningApplication.activateWithOptions`), no `NSApp.deactivate()` (que elegía otra app).
  - Prefijos `#etiqueta` / `@proyecto` (`src/prefijos.ts`) con autocompletado; aplicados en `crearNotaRapida`, así valen para notch, modal y ventana `--rapida`.
  - `src-tauri/src/captura.rs`: adjuntos temporales (`adjunto:<nombre>` → `assets/` al guardar), portapapeles (NSPasteboard PNG/TIFF/texto; `wl-paste`/`xclip` en Linux), pestaña del navegador (AppleScript; `Info.plist` con `NSAppleEventsUsageDescription`), texto seleccionado (AX; selección primaria en Linux), captura `screencapture -i` + OCR con Vision (`grim`/`slurp` + `tesseract` en Linux).
  - Vista previa: imágenes relativas resueltas con `convertFileSrc` + `assetProtocol` (alcance = raíces de la biblioteca, se amplía al elegir disco).
  - Verificado: atajo abre/cierra y devuelve el foco (prueba con `--capturar`), OCR de Vision, portapapeles texto/imagen, tests de adjuntos y prefijos.
  - **Ojo al probar**: `cargo build` sin `--features tauri/custom-protocol` no incrusta el frontend (las ventanas quedan en blanco). Usar `scripts/install.sh` o esa feature.
- [x] **Fase 2 — Organización** (2026-11-23)
  - `src-tauri/src/relaciones.rs`: wikilinks `[[Nota]]`, `[[Nota|alias]]`, `[[Nota#sección]]` (ignora código), resolución por slug del título o del nombre de archivo con preferencia por el mismo proyecto, backlinks con contexto, renombrado de enlaces y tareas (`- [ ]`, fechas `📅 AAAA-MM-DD` o `@AAAA-MM-DD`) con marcado verificando el texto de la línea.
  - `src-tauri/src/plantillas.rs`: `.plantillas/` en la raíz del lugar o dentro de un proyecto (prioridad: proyecto, mismo lugar, otro lugar). Se siembran Reunión, Bug e Idea la primera vez. Variables `{{fecha}}`, `{{hora}}`, `{{proyecto}}`, `{{titulo}}` y `{{cursor}}`.
  - UI: autocompletado `[[` en el editor, ⌘‑clic para seguir enlaces, enlaces rotos marcados (y crean la nota al abrirlos), panel «Enlazan aquí», vista Tareas (filtros Pendientes/Hoy/Atrasadas/Sin fecha/Hechas, agrupadas por proyecto), botón y comando `/` de plantillas, y al cambiar un título se ofrece actualizar los enlaces.
  - Verificado con 22 tests y una prueba guiada de la UI sobre una biblioteca temporal: contador de tareas, filtros, abrir tarea en su línea, seguir enlace, backlinks, renombrar (actualiza `[[…]]` en otras notas), marcar tarea de una nota cerrada y crear nota desde plantilla.
- [x] **Fase 5.1 — Índice TF‑IDF** (2026-11-24)
  - `src-tauri/src/indice.rs`: incremental por fecha de modificación (solo relee lo cambiado), título ×3 y etiquetas ×2, palabras vacías ES/EN, sin acentos, plurales simples. `relacionadas` (coseno, con los términos que más aportan), `pares` (índice invertido; para el grafo y 4.5) y `buscar` (BM25; para 4.2).
  - UI: «Notas parecidas» en el panel de relaciones (sin repetir las que ya enlazan o están enlazadas).
- [x] **Fase 5.2 — Grafo** (2026-11-24)
  - `src/grafo.ts`: `d3-force` + `<canvas>`, capa a pantalla completa sobre lista y editor. Proyectos como nodos principales (tamaño según notas, color de la paleta), aristas por tipo con filtros, proyecto ↔ proyecto por suma de relaciones, zoom semántico (< 0,55 solo proyectos), ficha de nota, doble clic abre, búsqueda que centra, filtros de lugar/fecha/etiqueta, posiciones en `localStorage`.
  - Backend: comando `grafo` (wikilinks resueltos de toda la biblioteca + pares TF‑IDF).
  - Pendiente: la mini‑vista de 1–2 saltos al pie de cada nota (el panel de relaciones ya cubre lo básico).
- [x] **Linux: panel de Quickshell** (2026-11-24): `linux/quickshell/notas/shell.qml` (hover en el borde superior, IPC `alternar`), comandos `--guardar-rapida`, `--proyectos`, `--portapapeles`, `--seleccion`, `--captura-pantalla` en `src-tauri/src/rapida.rs` (prefijos portados de `prefijos.ts`, con tests). **Sin probar en Linux todavía.**
- [ ] Fase 4 — IA — **en pausa** (2026-11-24): sin clave de la API todavía. El índice TF‑IDF (5.1) ya deja listos `buscar` (BM25, para 4.2) y `pares` (para 4.5).
- [x] **Fase 3 — Editor** (2026-11-24)
  - 3.1 `src-tauri/src/adjuntos.rs`: pegar/soltar en el editor → `assets/<slug>.<ext>` sin pisar (`adjuntar_archivo`, bytes crudos + cabeceras); `abrir_adjunto` (solo dentro de la carpeta de la nota); al mover, los adjuntos acompañan a la nota (copia si los comparte otra; renombra y reescribe la referencia si choca el nombre); al borrar, los huérfanos van a la papelera. `dragDropEnabled: false` en la ventana principal para que el `drop` llegue al webview.
  - 3.2 KaTeX (reglas propias `$…$` / `$$…$$`, salida HTML) y Mermaid (import dinámico, caché por fuente y tema, `htmlLabels: false`). **Ojo:** DOMPurify 3 quita atributos con `-->`; la fuente del diagrama va con `encodeURIComponent`.
  - 3.3 Modo enfoque: `Compartment` con un `ViewPlugin` que atenúa fuera del párrafo; CSS `#app[data-enfoque]`.
  - 3.4 `src-tauri/src/historial.rs`: instantánea del texto anterior al guardar (≥ 10 min), al cambiar/cerrar nota y antes de restaurar; poda 24 h / día / semana ISO; el historial se traslada al renombrar o mover la nota. Panel con `@codemirror/merge` y confirmación en dos pasos (las capas de diálogo no se apilan).
  - Verificado con 33 tests y prueba guiada de la UI: KaTeX en línea y bloque, `$5` como texto, diagrama Mermaid con sus etiquetas, pegar y soltar adjuntos, enfoque con atenuación y `Esc`, panel de versiones y restaurar.
