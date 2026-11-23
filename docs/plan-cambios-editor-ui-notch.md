# Plan: md-notes — editor, UI estilo Apple, apertura de .md y notch

> Copia del plan aprobado (para retomar si se agota el contexto).

## Contexto

- Stack: Tauri v2 (Rust) + frontend TypeScript vanilla con Vite. Editor CodeMirror 6, vista previa markdown-it + DOMPurify + highlight.js.
- Estado: variables de módulo en `src/main.ts` (`estado`, `vista`, `actual`, `sucio`, `modo`), sin framework. Backend es la fuente de verdad (`api.estado()`).
- Backend: `src-tauri/src/lib.rs` (comandos), `biblioteca.rs`, `store.rs`, `frontmatter.rs`. Plugins `dialog` y `opener`. No hay tray, multi-ventana ni fetch de red.
- Layout: `#app { grid-template-columns: 236px 310px 1fr }`, estilos en `src/estilos.css`.

## Decisiones

- Abrir `.md` existentes: **solo lectura** (no se guarda ni se mueve).
- Estilo de hoja: **por nota**, en front matter.
- Widget notch: **notch flotante** (píldora arriba) que abre panel de notas rápidas.
- Supuestos: resaltado = `==texto==` → `<mark>`; nota rápida sin proyecto = Bandeja; hoja independiente del tema; valores hoja `blanca|negra|amarilla`, pauta `ninguna|rayas|puntos`.

## Fase 0 — Dependencias

- `package.json`: `@codemirror/lang-javascript`, `json`, `python`, `rust`, `html`, `css`, `sql`, `yaml`, `bash` (para autocompletado dentro de bloques de código).
- `src-tauri/Cargo.toml`: `ureq = { version = "2", default-features = false, features = ["rustls", "gzip"] }` (título de URL).

## Fase 1 — Herramientas del editor

- Nuevo `src/editor-tools.ts` con extensiones CodeMirror:
  - `slashCommands()`: `CompletionSource` activado por `/` en línea vacía (encabezados, alertas GitHub, código, listas, cita, enlace, regla).
  - `floatingToolbar()`: `ViewPlugin` con barra flotante sobre la selección (negrita, cursiva, tachado, código, enlace, H1-H3, cita, alerta, resaltado `==…==`).
  - `pegarEnlace()`: `domEventHandlers({ paste })` detecta URL, menú inline (enlace/texto/enlace con título), título vía `api.obtenerTituloUrl` en segundo plano.
- `src/editor.ts`: añadir las 3 extensiones y `poner(texto, soloLectura=false)` con `EditorState.readOnly.of(true)` + `EditorView.editable.of(false)`.
- `src/vista-previa.ts`: regla `==texto==` → `<mark>`; detección de alertas `[!NOTE|TIP|IMPORTANT|WARNING|CAUTION]`.
- Backend `lib.rs`: comando `obtener_titulo_url(url) -> String` (GET con ureq, extraer `<title>`).
- `src/api.ts`: `obtenerTituloUrl`.

## Fase 2 — Abrir `.md` externos (solo lectura)

- Backend `lib.rs`: struct `ArchivoExterno { ruta, titulo, cuerpo }` + comando `elegir_archivo` (picker `.md`, leer con `frontmatter::parsear`, título front matter → primer `#` → nombre archivo).
- `src/api.ts`: tipo `ArchivoExterno` + `elegirArchivo`.
- `src/main.ts`: estado `lectura: ArchivoExterno | null`; `abrirArchivoExterno()`; botón "Abrir archivo" en sidebar y estado vacío; guardas no-op si `lectura`; `cerrarNota()` limpia `lectura`.

## Fase 3 — Sidebar colapsable

- `src/main.ts`: estado persistido `lateral` = `abierta|iconos|fija`; botones colapsar/pin en `.lateral-cab`; `data-lateral` en `#app`.
- `src/estilos.css`: `#app[data-lateral="iconos"] { grid-template-columns: 52px 310px 1fr }`; ocultar textos en iconos; flyout al hover/clic (panel absoluto 236px) que se recoge solo; pin fija expandido.

## Fase 4 — Restyle Apple + responsive

- `src/estilos.css`: paleta más Apple, radios 10–14px, sombras suaves, `backdrop-filter` moderado, hover suaves.
- Responsive: `<1080px` auto-colapsa sidebar (matchMedia), `<900px` lista más estrecha, `<760px` lista toggle; grid con `clamp()`.

## Fase 5 — Estilo de hoja por nota

- `frontmatter.rs`: `Documento` + `hoja`/`pauta`; parsear y serializar.
- `store.rs`: `Nota` gana `hoja`/`pauta`; `leer`, `guardar_nota`, `crear_nota`, `reescribir_etiquetas`.
- `biblioteca.rs` y `lib.rs`: firma `guardar_nota` con `hoja`/`pauta`.
- `src/api.ts`: `Nota` gana `hoja`/`pauta`; `guardarNota(..., hoja, pauta, ...)`.
- `src/main.ts`: `Actual` gana `hoja`/`pauta`; selectores `#sel-hoja`/`#sel-pauta`; `data-hoja`/`data-pauta` en `#paneles`.
- `src/estilos.css`: fondos de hoja + pauta (gradientes), sobrescribiendo vars locales en `#paneles`.

## Fase 6 — Notch flotante + notas rápidas

- `tauri.conf.json`: `macOSPrivateApi: true`.
- `capabilities/default.json`: windows `["main","notch","rapida"]`.
- `vite.config.ts`: multi-página con `notch.html` y `rapida.html`.
- Nuevos `notch.html`+`src/notch.ts` (píldora, click → `mostrar_rapida`) y `rapida.html`+`src/rapida.ts` (panel nota rápida).
- `lib.rs`: `.setup` crea ventanas `notch` (transparent, always_on_top, skip_taskbar) y `rapida` (oculta); comandos `mostrar_rapida`, `ocultar_rapida`.
- Flujo: crear con `crear_nota` + `guardar_nota`; ventana principal refresca al recuperar foco.

## Resumen de archivos

Crear: `src/editor-tools.ts`, `notch.html`, `src/notch.ts`, `rapida.html`, `src/rapida.ts`.

Modificar frontend: `src/editor.ts`, `src/api.ts`, `src/main.ts`, `src/vista-previa.ts`, `src/estilos.css`, `src/iconos.ts`, `vite.config.ts`, `package.json`.

Modificar backend: `src-tauri/src/lib.rs`, `frontmatter.rs`, `store.rs`, `biblioteca.rs`, `Cargo.toml`, `tauri.conf.json`, `capabilities/default.json`.

## Verificación

- `cargo test` (frontmatter/store), `npm run build`.
- Manual: slash, toolbar, autocompletado código, pegado enlaces; abrir `.md` solo lectura; sidebar colapsar/flyout/pin; responsive; hoja/pauta persistida; notch + nota rápida.

## Progreso (actualizar al retomar)

- [x] Fase 0: dependencias (`package.json` con `@codemirror/lang-*`, `Cargo.toml` con `ureq` y `macos-private-api`).
- [x] Fase 1: herramientas del editor (`editor-tools.ts`, `editor.ts`, `vista-previa.ts`, `obtener_titulo_url`).
- [x] Fase 2: abrir `.md` externos solo lectura (`elegir_archivo` + `main.ts`).
- [x] Fase 3: sidebar colapsable + flyout + pin (`main.ts`, `estilos.css`, `iconos.ts`).
- [x] Fase 4: restyle Apple + responsive (`estilos.css`, `tauri.conf.json`).
- [x] Fase 5: hoja/pauta por nota (backend + frontend + `index.html` + CSS).
- [x] Fase 6: notch flotante + notas rápidas (`notch.html/ts`, `rapida.html/ts`, `lib.rs`, `vite.config.ts`, capabilities).

Verificación hecha: `cargo test` (10 tests ok), `cargo check` ok, `npm run build` ok.

## Revisión 2 (2026-11-13) — correcciones tras probar la app

Fallos encontrados y corregidos:

- **No se podía cambiar la hoja**: `main.ts` buscaba `#meta`, pero el div no tenía `id`; al abrir una nota se lanzaba un error antes de aplicar la hoja. Ahora `index.html` tiene `id="meta"`.
- La pauta se pintaba fija sobre `#paneles` (no se desplazaba con el texto). Ahora va en `.cm-scroller` / `.panel-vista` con `background-attachment: local`, y el editor usa un renglón fijo (`--renglon: 24px`) para que las rayas coincidan con cada línea.
- Sin hoja elegida todo se veía «blanca» aunque el tema fuera oscuro. Nueva opción **Automática** (valor por defecto, no se escribe en el front matter) que sigue el tema.
- **Barra flotante** nunca aparecía: leía `coordsAtPos` dentro de `update()` y CodeMirror desactivaba el plugin. Ahora mide con `requestMeasure`; se oculta sin foco/al salir de vista y se coloca debajo si no cabe arriba.
- **Menú `/`** nunca aparecía: CodeMirror filtraba las opciones con el texto «/». Se devuelve `filter: false`.
- **Alertas `[!NOTE]`** no se renderizaban: la regla corría antes del procesado en línea. Ahora va al final del núcleo (`ruler.push`) y el título sale en su propia línea con icono. Las clases CSS (`alerta-note`…) ya coinciden.
- **Flyout de la barra lateral** desplazaba la lista y el editor de columna (el `absolute` sacaba al panel de la cuadrícula). Ahora el grid usa `grid-template-areas`.
- **Notch no se creaba en Linux/Wayland**: `primary_monitor()` devuelve `None`; se usa el primer monitor disponible.

Cambios de interfaz:

- Selector de hoja/pauta como panel visual (botón paleta en la barra) en lugar de dos `<select>`; nueva pauta **Cuadrícula**.
- `estilos.css` reescrito y ordenado: colores de sistema estilo macOS, radios 6/10/14px, sombras suaves, animaciones cortas, separadores con sangría en la lista, extracto a 2 líneas.
- Menú `/` con los colores del tema (antes, gris claro en modo oscuro).
- Pantallas `<760px`: lista y editor comparten columna con botón «volver».
- Backend: `hoja`/`pauta` se validan antes de escribir; extracto sin marcadores `[!NOTE]` ni `==`. Nuevo test `hoja_y_pauta_se_guardan_y_se_validan`.
- Ventanas del notch con títulos únicos (`md-notes-notch`, `md-notes-rapida`) para reglas de Hyprland.

Pendiente: en Hyprland el notch aparece flotando en el centro (200×200, fondo gris); hacen falta reglas de ventana para fijarlo arriba al centro.

## Revisión 3 (2026-11-13) — nota rápida dentro de la app

- En Hyprland el widget flotante (`notch`) quedaba en medio de la pantalla, encima de todo. **Se deja de crear al arrancar** (`crear_ventanas` eliminado de `lib.rs`).
- Nueva acción **Nota rápida** en la barra lateral (primera fila, también visible en modo iconos) y atajo `Ctrl+Shift+N`. Abre un diálogo dentro de la app (`pedirNotaRapida` en `ui.ts`): título opcional, texto Markdown y proyecto (Bandeja por defecto). `Ctrl+Enter` guarda; si no hay título se usa la primera línea.
- `rapida.html`: el modo oscuro iba antes de las reglas base y quedaba sobrescrito (campos blancos con texto blanco). Corregido.
- `notch.html`, `rapida.html`, `src/notch.ts`, `src/rapida.ts` y los comandos `mostrar_rapida`/`ocultar_rapida` quedan sin uso; se pueden borrar o reutilizar para un botón en la barra del sistema (Waybar).

## Revisión 4 (2026-11-13) — botón en la barra de Omarchy

- `md-notes --rapida`: abre solo una ventana pequeña (`rapida.html`, 560×420, sin bordes, no redimensionable → Hyprland la abre flotante y centrada) con el mismo modal de la app. `Esc`/Cancelar/Guardar la cierran (`cerrar_rapida`).
- `tauri-plugin-single-instance`: si la app ya está abierta, la segunda llamada le pasa los argumentos y termina; la instancia abierta muestra el modal. Sin `--rapida`, enfoca la ventana principal.
- La ventana principal ya no se crea desde `tauri.conf.json` (`"create": false`); `setup` decide qué abrir según los argumentos.
- Lógica compartida: `crearNotaRapida` (`api.ts`) y `opcionesProyecto` (`ui.ts`). Al guardar desde la ventana se emite `notas-cambiaron` y la principal refresca la lista.
- Barra: módulo QML `~/.config/omarchy/bar/modules/NotaRapida.qml` (botón con `BarIconButton`), añadido en `bar.layout.right` tras la bandeja. Nota: tras crear un archivo nuevo en esa carpeta hay que reiniciar el shell (`omarchy restart shell`); Qt guarda en caché el listado de la carpeta.
- Sin uso: `notch.html` y `src/notch.ts`.

## Revisión 5 (2026-11-13) — rediseño «Sereno» con tres temas

- Diseño: lienzo «Notas — rediseño sereno» (artifact). Aplicado a la app.
- Temas: **Bruma** (salvia, por defecto), **Lavanda** y **Marea**, cada uno en claro y oscuro. Se eligen en la barra lateral → botón paleta (paleta + modo Sistema/Claro/Oscuro). Se guardan en `localStorage` (`paleta`, `tema`) y la ventana de nota rápida usa los mismos.
- `src/tema.ts` (nuevo): fuentes incluidas (`@fontsource-variable/inter`, `literata`, `jetbrains-mono`; la CSP no permite Google Fonts) y lógica de paleta/modo.
- `estilos.css` reescrito: cada paleta define solo semillas (`--l-*`, `--d-*`) y los tokens se derivan con `color-mix`. Notas en Literata, interfaz en Inter, editor en JetBrains Mono; renglón de 28px.
- La nota es una tarjeta: la hoja (`data-hoja`/`data-pauta`) se aplica a `.editor`, así barra, título y paneles toman el color de la hoja. Hojas renombradas en la interfaz: Papel, Arena, Pizarra (los valores guardados siguen siendo `blanca`/`amarilla`/`negra`).
- Paleta de etiquetas nueva (tonos suaves) en `biblioteca.rs`; las etiquetas existentes conservan su color.
