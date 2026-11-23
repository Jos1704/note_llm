import MarkdownIt from "markdown-it";
import hljs from "highlight.js/lib/common";
import DOMPurify from "dompurify";

const md = new MarkdownIt({
  html: true,
  linkify: true,
  breaks: false,
  highlight(codigo, lenguaje) {
    if (lenguaje && hljs.getLanguage(lenguaje)) {
      try {
        return hljs.highlight(codigo, { language: lenguaje, ignoreIllegals: true }).value;
      } catch {
        /* se usa el escapado por defecto */
      }
    }
    return "";
  },
});

// Listas de tareas: `- [ ] algo` → casilla que se puede marcar desde la vista previa.
// `data-linea` es la línea del elemento en el Markdown, para editarla al hacer clic.
md.core.ruler.push("tareas", (state) => {
  const t = state.tokens;
  for (let i = 2; i < t.length; i++) {
    if (t[i].type !== "inline" || t[i - 1].type !== "paragraph_open" || t[i - 2].type !== "list_item_open") {
      continue;
    }
    const m = /^\[([ xX])\]\s+/.exec(t[i].content);
    const hijos = t[i].children;
    if (!m || !hijos?.length || hijos[0].type !== "text" || !hijos[0].content.startsWith(m[0].trimEnd())) {
      continue;
    }
    hijos[0].content = hijos[0].content.slice(m[0].length).replace(/^\s+/, "");
    const item = t[i - 2];
    item.attrJoin("class", "tarea");
    const casilla = new state.Token("html_inline", "", 0);
    const marcada = m[1] !== " " ? " checked" : "";
    casilla.content = `<input type="checkbox" class="casilla" data-linea="${item.map?.[0] ?? -1}"${marcada}>`;
    hijos.unshift(casilla);
  }
});

// Resaltado de lectura: `==texto==` → `<mark>`.
md.core.ruler.push("resaltado", (state) => {
  for (const token of state.tokens) {
    if (token.type !== "inline" || !token.children) continue;
    const hijos = token.children;
    for (let i = 0; i < hijos.length; i++) {
      const h = hijos[i];
      if (h.type !== "text" || !h.content.includes("==")) continue;
      const partes = h.content.split(/(==[^=\n]+==)/g).filter(Boolean);
      if (partes.length < 2) continue;
      const nuevos = partes.map((p) => {
        if (p.startsWith("==") && p.endsWith("==") && p.length > 4) {
          const t = new state.Token("html_inline", "", 0);
          t.content = `<mark>${md.utils.escapeHtml(p.slice(2, -2))}</mark>`;
          return t;
        }
        const t = new state.Token("text", "", 0);
        t.content = p;
        return t;
      });
      hijos.splice(i, 1, ...nuevos);
      i += nuevos.length - 1;
    }
  }
});

// Alertas de GitHub: `> [!NOTE]` (y variantes) → blockquote con clase y título.
// Va con `push` (al final del núcleo): necesita el texto en línea ya procesado y unido.
md.core.ruler.push("alertas", (state) => {
  const nombres: Record<string, string> = {
    NOTE: "Nota",
    TIP: "Sugerencia",
    IMPORTANT: "Importante",
    WARNING: "Aviso",
    CAUTION: "Precaución",
  };
  const t = state.tokens;
  for (let i = 0; i < t.length; i++) {
    if (t[i].type !== "blockquote_open") continue;
    let tipo = "";
    for (let j = i + 1; j < t.length && t[j].type !== "blockquote_close"; j++) {
      const hijos = t[j].children;
      if (t[j].type !== "inline" || !hijos) continue;
      const c = hijos[0];
      const m = c?.type === "text" ? /^\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]\s*/i.exec(c.content) : null;
      if (m) {
        const clave = m[1].toUpperCase();
        tipo = clave.toLowerCase();
        const titulo = new state.Token("html_inline", "", 0);
        titulo.content = `<span class="alerta-titulo">${nombres[clave]}</span>`;
        c.content = c.content.slice(m[0].length);
        // El salto de línea tras `[!NOTE]` sobra: el título ya es un bloque.
        if (!c.content && hijos[1]?.type === "softbreak") hijos.splice(1, 1);
        hijos.unshift(titulo);
      }
      break;
    }
    if (tipo) t[i].attrJoin("class", `alerta alerta-${tipo}`);
  }
});

export function renderizar(markdown: string): string {
  return DOMPurify.sanitize(md.render(markdown), { ADD_ATTR: ["target"] });
}
