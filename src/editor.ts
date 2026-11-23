import { EditorView, keymap } from "@codemirror/view";
import { EditorState, type Extension } from "@codemirror/state";
import { basicSetup } from "codemirror";
import { indentWithTab } from "@codemirror/commands";
import { autocompletion } from "@codemirror/autocomplete";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";
import { floatingToolbar, pegarEnlace, slashCommands } from "./editor-tools";

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
}

export function crearEditor(padre: HTMLElement, alCambiar: () => void, alDesplazar: (fraccion: number) => void): Editor {
  const extensiones: Extension[] = [
    basicSetup,
    keymap.of([indentWithTab]),
    markdown({ base: markdownLanguage, codeLanguages: languages }),
    syntaxHighlighting(resaltado),
    autocompletion({ override: [slashCommands()] }),
    floatingToolbar(),
    pegarEnlace(),
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
    },
    texto: () => view.state.doc.toString(),
    alternarTarea(linea) {
      if (linea < 0 || linea >= view.state.doc.lines) return;
      const l = view.state.doc.line(linea + 1);
      const m = /\[([ xX])\]/.exec(l.text);
      if (!m) return;
      const desde = l.from + m.index + 1;
      view.dispatch({ changes: { from: desde, to: desde + 1, insert: m[1] === " " ? "x" : " " } });
    },
  };
}
