import { EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { EditorSelection, EditorState, type Extension } from "@codemirror/state";
import { type Completion, type CompletionContext, type CompletionSource } from "@codemirror/autocomplete";
import { api } from "./api";

// ------------------------------------------------------------------ slash

function aplicar(texto: string, cursor: number): Completion["apply"] {
  return (view, _completion, from, to) => {
    view.dispatch({
      changes: { from, to, insert: texto },
      selection: EditorSelection.single(from + cursor),
    });
  };
}

const opciones: Completion[] = [
  { label: "Encabezado 1", detail: "H1", type: "keyword", apply: "# " },
  { label: "Encabezado 2", detail: "H2", type: "keyword", apply: "## " },
  { label: "Encabezado 3", detail: "H3", type: "keyword", apply: "### " },
  { label: "Encabezado 4", detail: "H4", type: "keyword", apply: "#### " },
  { label: "Encabezado 5", detail: "H5", type: "keyword", apply: "##### " },
  { label: "Encabezado 6", detail: "H6", type: "keyword", apply: "###### " },
  { label: "Alerta: Nota", detail: "NOTE", type: "text", apply: aplicar("> [!NOTE]\n> ", "> [!NOTE]\n> ".length) },
  { label: "Alerta: Sugerencia", detail: "TIP", type: "text", apply: aplicar("> [!TIP]\n> ", "> [!TIP]\n> ".length) },
  { label: "Alerta: Importante", detail: "IMPORTANT", type: "text", apply: aplicar("> [!IMPORTANT]\n> ", "> [!IMPORTANT]\n> ".length) },
  { label: "Alerta: Aviso", detail: "WARNING", type: "text", apply: aplicar("> [!WARNING]\n> ", "> [!WARNING]\n> ".length) },
  { label: "Alerta: Precaución", detail: "CAUTION", type: "text", apply: aplicar("> [!CAUTION]\n> ", "> [!CAUTION]\n> ".length) },
  { label: "Bloque de código", detail: "```", type: "text", apply: aplicar("```\n\n```", 4) },
  { label: "Lista con viñetas", detail: "-", type: "keyword", apply: "- " },
  { label: "Lista numerada", detail: "1.", type: "keyword", apply: "1. " },
  { label: "Tarea", detail: "[ ]", type: "keyword", apply: "- [ ] " },
  { label: "Cita", detail: ">", type: "keyword", apply: "> " },
  { label: "Enlace", detail: "[]()", type: "text", apply: aplicar("[](url)", 1) },
  { label: "Regla horizontal", detail: "---", type: "keyword", apply: "---" },
];

/** Menú de inserciones que se abre al escribir `/` al inicio de una línea. */
export function slashCommands(): CompletionSource {
  return (ctx: CompletionContext) => {
    const { state, pos } = ctx;
    if (state.facet(EditorState.readOnly)) return null;
    const line = state.doc.lineAt(pos);
    const antes = line.text.slice(0, pos - line.from);
    if (!antes.startsWith("/")) return null;
    const q = antes.slice(1).toLowerCase();
    const options = opciones.filter((o) => o.label.toLowerCase().includes(q) || o.detail?.includes(q));
    if (!options.length) return null;
    // `filter: false`: el filtrado ya se hizo arriba (CodeMirror compararía «/…» con las etiquetas y no quedaría nada).
    return { from: line.from, options, filter: false };
  };
}

// ------------------------------------------------------------------ barra flotante

const botonesToolbar = `
  <button data-fmt="negrita" title="Negrita"><b>B</b></button>
  <button data-fmt="cursiva" title="Cursiva"><i>I</i></button>
  <button data-fmt="tachado" title="Tachado"><s>S</s></button>
  <button data-fmt="resaltado" title="Resaltar (==texto==)"><mark>A</mark></button>
  <button data-fmt="codigo" title="Código"><code>{ }</code></button>
  <button data-fmt="enlace" title="Enlace">🔗</button>
  <span class="sep"></span>
  <button data-fmt="h1" title="Encabezado 1">H1</button>
  <button data-fmt="h2" title="Encabezado 2">H2</button>
  <button data-fmt="h3" title="Encabezado 3">H3</button>
  <span class="sep"></span>
  <button data-fmt="cita" title="Cita">❝</button>
  <button data-fmt="alerta" title="Alerta">!</button>`;

type Posicion = { left: number; top: number; bottom: number; limite: number } | null;

class ToolbarPlugin {
  private el: HTMLDivElement;
  private readonly alDesplazar = () => this.programar();

  constructor(private view: EditorView) {
    this.el = document.createElement("div");
    this.el.className = "editor-toolbar";
    this.el.hidden = true;
    this.el.innerHTML = botonesToolbar;
    document.body.append(this.el);
    this.el.addEventListener("mousedown", (e) => e.preventDefault());
    this.el.addEventListener("click", (e) => {
      const b = (e.target as HTMLElement).closest<HTMLElement>("[data-fmt]");
      if (b) this.aplicar(b.dataset.fmt!);
    });
    view.scrollDOM.addEventListener("scroll", this.alDesplazar, { passive: true });
  }

  update(u: ViewUpdate) {
    if (u.selectionSet || u.docChanged || u.focusChanged || u.geometryChanged) this.programar();
  }

  destroy() {
    this.view.scrollDOM.removeEventListener("scroll", this.alDesplazar);
    this.el.remove();
  }

  /** CodeMirror no deja leer el layout durante `update`: se mide en su ciclo de medición. */
  private programar() {
    this.view.requestMeasure<Posicion>({
      key: this,
      read: (view) => this.medir(view),
      write: (pos) => this.colocar(pos),
    });
  }

  private medir(view: EditorView): Posicion {
    if (view.state.facet(EditorState.readOnly)) return null;
    const sel = view.state.selection.main;
    // Solo con texto seleccionado, el editor enfocado y visible (no en modo «solo vista»).
    if (sel.empty || !view.hasFocus || !view.dom.offsetParent) return null;
    const desde = view.coordsAtPos(sel.from);
    const hasta = view.coordsAtPos(sel.to);
    const caja = view.scrollDOM.getBoundingClientRect();
    if (!desde || !hasta || desde.top < caja.top || desde.top > caja.bottom) return null;
    return { left: (desde.left + hasta.left) / 2, top: desde.top, bottom: desde.bottom, limite: caja.top };
  }

  private colocar(pos: Posicion) {
    if (!pos) {
      this.el.hidden = true;
      return;
    }
    this.el.hidden = false;
    const ancho = this.el.offsetWidth || 320;
    const alto = this.el.offsetHeight || 38;
    this.el.style.left = `${Math.max(8, Math.min(pos.left - ancho / 2, innerWidth - ancho - 8))}px`;
    // Encima de la selección; si no cabe dentro del editor, debajo.
    const arriba = pos.top - alto - 10;
    this.el.style.top = `${arriba >= pos.limite ? arriba : pos.bottom + 10}px`;
  }

  private aplicar(fmt: string) {
    switch (fmt) {
      case "negrita":
        return this.envolver("**", "**");
      case "cursiva":
        return this.envolver("*", "*");
      case "tachado":
        return this.envolver("~~", "~~");
      case "codigo":
        return this.envolver("`", "`");
      case "resaltado":
        return this.envolver("==", "==");
      case "enlace":
        return this.enlace();
      case "h1":
        return this.prefijar("# ");
      case "h2":
        return this.prefijar("## ");
      case "h3":
        return this.prefijar("### ");
      case "cita":
        return this.prefijar("> ");
      case "alerta":
        return this.alerta();
    }
  }

  private envolver(antes: string, despues: string) {
    const sel = this.view.state.selection.main;
    if (sel.empty) return;
    const texto = this.view.state.sliceDoc(sel.from, sel.to);
    this.view.dispatch({ changes: { from: sel.from, to: sel.to, insert: antes + texto + despues } });
  }

  private prefijar(prefijo: string) {
    const sel = this.view.state.selection.main;
    const inicio = this.view.state.doc.lineAt(sel.from).from;
    const fin = this.view.state.doc.lineAt(sel.to).to;
    const lineas = this.view.state.sliceDoc(inicio, fin).split("\n");
    this.view.dispatch({ changes: { from: inicio, to: fin, insert: lineas.map((l) => prefijo + l).join("\n") } });
  }

  private enlace() {
    const sel = this.view.state.selection.main;
    const vacio = sel.empty;
    const texto = this.view.state.sliceDoc(sel.from, sel.to);
    this.view.dispatch({ changes: { from: sel.from, to: sel.to, insert: vacio ? "[](url)" : `[${texto}](url)` } });
  }

  private alerta() {
    const sel = this.view.state.selection.main;
    const vacio = sel.empty;
    const cuerpo = vacio ? "" : this.view.state.sliceDoc(sel.from, sel.to).split("\n").join("\n> ");
    this.view.dispatch({ changes: { from: sel.from, to: sel.to, insert: `> [!NOTE]\n> ${cuerpo}` } });
  }
}

export function floatingToolbar(): Extension {
  return ViewPlugin.fromClass(ToolbarPlugin);
}

// ------------------------------------------------------------------ pegado de enlaces

function esUrl(t: string): boolean {
  return /^https?:\/\/\S+$/i.test(t) || /^mailto:\S+$/i.test(t);
}

function insertarTexto(view: EditorView, texto: string) {
  const sel = view.state.selection.main;
  view.dispatch({ changes: { from: sel.from, to: sel.to, insert: texto } });
  view.focus();
}

function mostrarMenuEnlace(view: EditorView, url: string) {
  const sel = view.state.selection.main;
  const etiqueta = sel.empty ? url : view.state.sliceDoc(sel.from, sel.to);

  const menu = document.createElement("div");
  menu.className = "editor-menu";

  const btnTexto = document.createElement("button");
  btnTexto.innerHTML = "<span>Pegar como texto</span>";
  btnTexto.onclick = () => {
    insertarTexto(view, url);
    cerrar();
  };

  const btnEnlace = document.createElement("button");
  btnEnlace.innerHTML = "<span>Pegar como enlace</span>";
  btnEnlace.onclick = () => {
    insertarTexto(view, `[${etiqueta}](${url})`);
    cerrar();
  };

  const btnTitulo = document.createElement("button");
  btnTitulo.innerHTML = "<span>Cargando título…</span>";
  btnTitulo.onclick = () => {
    const t = btnTitulo.dataset.titulo?.trim();
    insertarTexto(view, `[${t || url}](${url})`);
    cerrar();
  };

  menu.append(btnTexto, btnEnlace, btnTitulo);

  const cerrar = () => {
    menu.remove();
    document.removeEventListener("mousedown", fuera, true);
    document.removeEventListener("keydown", tecla, true);
  };
  const fuera = (e: MouseEvent) => {
    if (!menu.contains(e.target as Node)) cerrar();
  };
  const tecla = (e: KeyboardEvent) => {
    if (e.key === "Escape") cerrar();
  };

  document.addEventListener("mousedown", fuera, true);
  document.addEventListener("keydown", tecla, true);
  menu.addEventListener("mousedown", (e) => e.preventDefault());

  const coords = view.coordsAtPos(sel.from);
  if (coords) {
    menu.style.left = `${Math.min(coords.left, innerWidth - 250)}px`;
    menu.style.top = `${coords.bottom + 6}px`;
  }
  document.body.append(menu);

  // El título se trae en segundo plano; la opción se actualiza cuando llega.
  api.obtenerTituloUrl(url)
    .then((t) => {
      btnTitulo.dataset.titulo = t;
      const span = btnTitulo.querySelector("span");
      if (span) span.textContent = t.trim() ? `Enlace: ${t.trim()}` : "Enlace con título";
    })
    .catch(() => {
      const span = btnTitulo.querySelector("span");
      if (span) span.textContent = "Enlace con título";
    });
}

export function pegarEnlace(): Extension {
  return EditorView.domEventHandlers({
    paste(event, view) {
      if (view.state.facet(EditorState.readOnly)) return false;
      const url = event.clipboardData?.getData("text/plain")?.trim();
      if (!url || !esUrl(url)) return false;
      event.preventDefault();
      mostrarMenuEnlace(view, url);
      return true;
    },
  });
}
