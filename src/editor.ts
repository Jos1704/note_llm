import { Decoration, EditorView, ViewPlugin, keymap, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import { Compartment, EditorSelection, EditorState, RangeSetBuilder, type Extension } from "@codemirror/state";
import { basicSetup } from "codemirror";
import { indentWithTab } from "@codemirror/commands";
import { autocompletion, type Completion } from "@codemirror/autocomplete";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";
import { floatingToolbar, pegarEnlace, slashCommands, wikilinkCompletion, wikilinks, type DestinoEnlace } from "./editor-tools";

const resaltado = HighlightStyle.define([
  { tag: t.heading1, color: "var(--h1)", fontWeight: "700", fontSize: "1.45em" },
  { tag: t.heading2, color: "var(--h2)", fontWeight: "700", fontSize: "1.25em" },
  { tag: t.heading3, color: "var(--h3)", fontWeight: "700", fontSize: "1.1em" },
  { tag: [t.heading4, t.heading5, t.heading6], color: "var(--h3)", fontWeight: "700" },
  { tag: t.strong, fontWeight: "700" },
  { tag: t.emphasis, fontStyle: "italic" },
  { tag: t.strikethrough, textDecoration: "line-through" },
  { tag: [t.link, t.url], color: "var(--enlace)" },
  { tag: t.monospace, color: "var(--codigo)" },
  { tag: t.quote, color: "var(--suave)", fontStyle: "italic" },
  { tag: [t.processingInstruction, t.meta, t.contentSeparator], color: "var(--marca)" },
  { tag: t.list, color: "var(--h2)" },
  // Código dentro de bloques ```lenguaje
  { tag: [t.keyword, t.modifier, t.operatorKeyword], color: "var(--sx-clave)" },
  { tag: [t.string, t.special(t.string), t.regexp], color: "var(--sx-texto)" },
  { tag: [t.number, t.bool, t.atom], color: "var(--sx-numero)" },
  { tag: [t.comment, t.lineComment, t.blockComment], color: "var(--suave)", fontStyle: "italic" },
  { tag: [t.function(t.variableName), t.function(t.propertyName)], color: "var(--sx-funcion)" },
  { tag: [t.typeName, t.className], color: "var(--sx-tipo)" },
  { tag: [t.propertyName, t.attributeName], color: "var(--sx-prop)" },
]);

const tema = EditorView.theme({
  "&": { height: "100%", backgroundColor: "transparent", color: "var(--texto)" },
  "&.cm-focused": { outline: "none" },
  // Renglón fijo (--renglon) para que la pauta de la hoja coincida con las líneas de texto.
  ".cm-scroller": { fontFamily: "var(--mono)", lineHeight: "var(--renglon)", fontSize: "13.5px" },
  ".cm-content": { padding: "var(--renglon) 0 40vh", caretColor: "var(--acento)" },
  ".cm-line": { padding: "0 20px 0 6px" },
  ".cm-gutters": { backgroundColor: "transparent", border: "none", color: "var(--suave-2)" },
  ".cm-activeLine": { backgroundColor: "var(--linea-activa)" },
  ".cm-activeLineGutter": { backgroundColor: "transparent", color: "var(--suave)" },
  ".cm-cursor": { borderLeftColor: "var(--acento)", borderLeftWidth: "2px" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
    backgroundColor: "var(--seleccion) !important",
  },
  ".cm-foldGutter span": { color: "var(--suave-2)" },
  ".cm-panels": { backgroundColor: "var(--bg-lista)", color: "var(--texto)" },
  ".cm-searchMatch": { backgroundColor: "var(--busqueda)" },
  ".cm-tooltip": {
    backgroundColor: "var(--bg-campo)",
    color: "var(--texto)",
    border: "1px solid var(--borde)",
    borderRadius: "10px",
    boxShadow: "var(--sombra)",
    overflow: "hidden",
  },
  ".cm-tooltip.cm-tooltip-autocomplete > ul": { fontFamily: "var(--sans)", maxHeight: "18em", padding: "4px" },
  ".cm-tooltip.cm-tooltip-autocomplete > ul > li": { padding: "5px 10px", borderRadius: "6px", lineHeight: "1.4" },
  ".cm-tooltip-autocomplete ul li[aria-selected]": { backgroundColor: "var(--acento)", color: "var(--acento-texto)" },
  ".cm-completionDetail": { opacity: "0.6", marginLeft: "10px", fontStyle: "normal", fontFamily: "var(--mono)" },
  ".cm-completionIcon": { display: "none" },
});

export interface Editor {
  view: EditorView;
  /** Reemplaza el documento (al abrir otra nota) sin mezclar el historial de deshacer. */
  poner(texto: string, soloLectura?: boolean): void;
  texto(): string;
  /** Marca o desmarca la casilla `[ ]`/`[x]` de la línea (base 0). */
  alternarTarea(linea: number): void;
  /** Lleva el cursor al inicio de la línea (base 0) y la centra en pantalla. */
  irALinea(linea: number): void;
  /** Pone el cursor en una posición del texto. */
  cursorEn(pos: number): void;
  /** Activa o quita la atenuación del modo enfoque. */
  enfoque(activo: boolean): void;
}

/** Modo enfoque: atenúa las líneas fuera del párrafo donde está el cursor. */
const atenuada = Decoration.line({ class: "cm-atenuada" });
const parrafoActual = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = this.calcular(view);
    }
    update(u: ViewUpdate) {
      if (u.docChanged || u.selectionSet || u.viewportChanged) this.decorations = this.calcular(u.view);
    }
    calcular(view: EditorView): DecorationSet {
      const doc = view.state.doc;
      const actual = doc.lineAt(view.state.selection.main.head).number;
      // Párrafo: líneas seguidas con texto alrededor del cursor (una línea vacía lo corta).
      let desde = actual;
      let hasta = actual;
      if (doc.line(actual).text.trim()) {
        while (desde > 1 && doc.line(desde - 1).text.trim()) desde--;
        while (hasta < doc.lines && doc.line(hasta + 1).text.trim()) hasta++;
      }
      const b = new RangeSetBuilder<Decoration>();
      for (const { from, to } of view.visibleRanges) {
        for (let n = doc.lineAt(from).number; n <= doc.lineAt(to).number; n++) {
          if (n < desde || n > hasta) b.add(doc.line(n).from, doc.line(n).from, atenuada);
        }
      }
      return b.finish();
    }
  },
  { decorations: (p) => p.decorations },
);

export interface OpcionesEditor {
  /** Notas a las que se puede enlazar con `[[`. */
  destinos: () => DestinoEnlace[];
  /** ⌘/Ctrl + clic en un `[[…]]` (recibe lo que hay entre corchetes). */
  abrirEnlace: (dentro: string) => void;
  /** Opciones extra del menú `/` (plantillas). */
  extraSlash: () => Promise<Completion[]>;
  /** Archivos pegados o arrastrados: los guarda y devuelve el Markdown de cada uno. */
  adjuntar: (archivos: File[]) => Promise<string[]>;
}

/** Pegar o soltar archivos en el editor: se guardan como adjuntos y se inserta su Markdown. */
function archivosSoltados(adjuntar: OpcionesEditor["adjuntar"]): Extension {
  const manejar = (view: EditorView, archivos: File[], pos: number) => {
    adjuntar(archivos)
      .then((trozos) => {
        if (!trozos.length) return;
        const p = Math.min(pos, view.state.doc.length);
        const antes = view.state.doc.sliceString(Math.max(0, p - 2), p);
        const pre = p === 0 || antes.endsWith("\n\n") ? "" : antes.endsWith("\n") ? "\n" : "\n\n";
        const texto = `${pre}${trozos.join("\n\n")}\n\n`;
        view.dispatch({ changes: { from: p, insert: texto }, selection: { anchor: p + texto.length }, userEvent: "input.paste" });
        view.focus();
      })
      .catch(() => {});
  };
  return EditorView.domEventHandlers({
    paste(event, view) {
      const archivos = [...(event.clipboardData?.files ?? [])];
      if (!archivos.length || view.state.facet(EditorState.readOnly)) return false;
      event.preventDefault();
      manejar(view, archivos, view.state.selection.main.head);
      return true;
    },
    dragover(event) {
      if (event.dataTransfer?.types.includes("Files")) {
        event.preventDefault();
        return true;
      }
      return false;
    },
    drop(event, view) {
      const archivos = [...(event.dataTransfer?.files ?? [])];
      if (!archivos.length || view.state.facet(EditorState.readOnly)) return false;
      event.preventDefault();
      const pos = view.posAtCoords({ x: event.clientX, y: event.clientY }) ?? view.state.selection.main.head;
      manejar(view, archivos, view.state.doc.lineAt(pos).to);
      return true;
    },
  });
}

export function crearEditor(
  padre: HTMLElement,
  alCambiar: () => void,
  alDesplazar: (fraccion: number) => void,
  opciones: OpcionesEditor,
): Editor {
  const compartimentoEnfoque = new Compartment();
  let enfoqueActivo = false;
  const extensiones: Extension[] = [
    compartimentoEnfoque.of([]),
    basicSetup,
    keymap.of([indentWithTab]),
    markdown({ base: markdownLanguage, codeLanguages: languages }),
    syntaxHighlighting(resaltado),
    autocompletion({ override: [slashCommands(opciones.extraSlash), wikilinkCompletion(opciones.destinos)] }),
    wikilinks(opciones.abrirEnlace),
    floatingToolbar(),
    pegarEnlace(),
    archivosSoltados(opciones.adjuntar),
    EditorView.lineWrapping,
    tema,
    EditorView.updateListener.of((u) => {
      if (u.docChanged) alCambiar();
    }),
    EditorView.domEventHandlers({
      scroll(_e, view) {
        const s = view.scrollDOM;
        const max = s.scrollHeight - s.clientHeight;
        alDesplazar(max > 0 ? s.scrollTop / max : 0);
      },
    }),
  ];

  const view = new EditorView({ parent: padre, state: EditorState.create({ extensions: extensiones }) });

  return {
    view,
    poner(texto, soloLectura = false) {
      const ext = soloLectura ? [...extensiones, EditorState.readOnly.of(true), EditorView.editable.of(false)] : extensiones;
      view.setState(EditorState.create({ doc: texto, extensions: ext }));
      if (enfoqueActivo) view.dispatch({ effects: compartimentoEnfoque.reconfigure(parrafoActual) });
    },
    texto: () => view.state.doc.toString(),
    enfoque(activo) {
      enfoqueActivo = activo;
      view.dispatch({ effects: compartimentoEnfoque.reconfigure(activo ? parrafoActual : []) });
    },
    alternarTarea(linea) {
      if (linea < 0 || linea >= view.state.doc.lines) return;
      const l = view.state.doc.line(linea + 1);
      const m = /\[([ xX])\]/.exec(l.text);
      if (!m) return;
      const desde = l.from + m.index + 1;
      view.dispatch({ changes: { from: desde, to: desde + 1, insert: m[1] === " " ? "x" : " " } });
    },
    irALinea(linea) {
      const n = Math.min(Math.max(linea, 0), view.state.doc.lines - 1);
      const l = view.state.doc.line(n + 1);
      view.dispatch({ selection: EditorSelection.single(l.from), effects: EditorView.scrollIntoView(l.from, { y: "center" }) });
      view.focus();
    },
    cursorEn(pos) {
      const p = Math.min(Math.max(pos, 0), view.state.doc.length);
      view.dispatch({ selection: EditorSelection.single(p), effects: EditorView.scrollIntoView(p, { y: "center" }) });
      view.focus();
    },
  };
}
