import MarkdownIt from "markdown-it";
import hljs from "highlight.js/lib/common";
import DOMPurify from "dompurify";
import { convertFileSrc } from "@tauri-apps/api/core";
import katex from "katex";
import "katex/dist/katex.min.css";
import { partirWikilink } from "./enlaces";

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

// Enlaces entre notas: `[[Título]]`, `[[Título|texto]]`, `[[Título#sección]]`.
// Si no hay ninguna nota con ese nombre, el enlace sale atenuado (un clic la crea).
let existe: (destino: string) => boolean = () => true;

/** Cómo saber si un destino `[[…]]` lleva a una nota (lo pone la app con las notas actuales). */
export function ponerResolutor(f: (destino: string) => boolean) {
  existe = f;
}

const escAttr = (s: string) => s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);

md.inline.ruler.before("link", "wikilink", (state, silent) => {
  const { src, pos } = state;
  if (src.charCodeAt(pos) !== 0x5b || src.charCodeAt(pos + 1) !== 0x5b) return false;
  const fin = src.indexOf("]]", pos + 2);
  if (fin < 0) return false;
  const dentro = src.slice(pos + 2, fin);
  if (!dentro.trim() || /[[\]\n]/.test(dentro)) return false;
  const { destino, texto } = partirWikilink(dentro);
  if (!destino) return false;
  if (!silent) {
    const t = state.push("html_inline", "", 0);
    const rota = existe(destino) ? "" : " rota";
    const titulo = rota ? "Crear esta nota" : destino;
    t.content = `<a href="#" class="wikilink${rota}" data-wikilink="${escAttr(destino)}" title="${escAttr(titulo)}">${escAttr(texto)}</a>`;
  }
  state.pos = fin + 2;
  return true;
});

// Imágenes relativas (`assets/captura.png`): se resuelven contra la carpeta de la nota abierta.
let carpeta: string | null = null;

/** Carpeta absoluta de la nota que se está mostrando (`null` = sin resolver rutas relativas). */
export function ponerCarpeta(dir: string | null) {
  carpeta = dir;
}

const imagenPorDefecto = md.renderer.rules.image!;
md.renderer.rules.image = (tokens, idx, opciones, env, self) => {
  const t = tokens[idx];
  const src = String(t.attrGet("src") ?? "");
  const relativa = src && !/^[a-z][a-z0-9+.-]*:/i.test(src) && !src.startsWith("/") && !src.startsWith("#");
  if (carpeta && relativa) {
    let ruta = src;
    try {
      ruta = decodeURI(src);
    } catch {
      /* se usa tal cual */
    }
    t.attrSet("src", convertFileSrc(`${carpeta}/${ruta}`));
  }
  return imagenPorDefecto(tokens, idx, opciones, env, self);
};

// ------------------------------------------------------------------ fórmulas (KaTeX)
// `$x^2$` en línea y `$$…$$` en bloque. `$` seguido de espacio o cifra tras el cierre no cuenta
// (así «cuesta $5 y $10» sigue siendo texto).

function formula(tex: string, bloque: boolean): string {
  try {
    return katex.renderToString(tex, { displayMode: bloque, throwOnError: false, output: "html", trust: false, strict: "ignore" });
  } catch {
    return `<code>${md.utils.escapeHtml(tex)}</code>`;
  }
}

md.inline.ruler.after("escape", "formula", (state, silent) => {
  const { src, pos } = state;
  if (src.charCodeAt(pos) !== 0x24 /* $ */ || src.charCodeAt(pos + 1) === 0x24) return false;
  const siguiente = src[pos + 1];
  if (!siguiente || /\s/.test(siguiente)) return false;
  let fin = pos + 1;
  while ((fin = src.indexOf("$", fin)) !== -1) {
    if (src[fin - 1] !== "\\" && !/\s/.test(src[fin - 1]) && !/\d/.test(src[fin + 1] ?? "")) break;
    fin++;
  }
  if (fin === -1 || fin === pos + 1) return false;
  if (!silent) {
    const t = state.push("html_inline", "", 0);
    t.content = formula(src.slice(pos + 1, fin), false);
  }
  state.pos = fin + 1;
  return true;
});

md.block.ruler.after("blockquote", "formula_bloque", (state, inicio, finLinea, silent) => {
  const desde = state.bMarks[inicio] + state.tShift[inicio];
  const primera = state.src.slice(desde, state.eMarks[inicio]);
  if (!primera.startsWith("$$")) return false;
  let tex: string;
  let ultima = inicio;
  const enUna = primera.length > 4 && primera.trimEnd().endsWith("$$");
  if (enUna) {
    tex = primera.trimEnd().slice(2, -2);
  } else {
    const lineas = [primera.slice(2)];
    let cerrada = false;
    for (let l = inicio + 1; l < finLinea; l++) {
      const texto = state.src.slice(state.bMarks[l] + state.tShift[l], state.eMarks[l]);
      ultima = l;
      if (texto.trimEnd().endsWith("$$")) {
        lineas.push(texto.trimEnd().slice(0, -2));
        cerrada = true;
        break;
      }
      lineas.push(texto);
    }
    if (!cerrada) return false;
    tex = lineas.join("\n");
  }
  if (silent) return true;
  const t = state.push("html_block", "", 0);
  t.content = `<div class="formula">${formula(tex.trim(), true)}</div>\n`;
  t.map = [inicio, ultima + 1];
  state.line = ultima + 1;
  return true;
});

// ------------------------------------------------------------------ diagramas (Mermaid)
// El bloque ```mermaid sale como marcador; `dibujarDiagramas` carga Mermaid solo si hace falta y
// cambia cada marcador por su SVG. Los SVG quedan en caché por fuente y tema, para que al
// volver a pintar la vista previa (cada tecla) salgan al instante y sin parpadeo.

const diagramas = new Map<string, string>();
const oscuro = () => {
  const m = /\d+/g;
  const [r, g, b] = (getComputedStyle(document.body).backgroundColor.match(m) ?? ["255", "255", "255"]).map(Number);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b < 128;
};
const claveDiagrama = (fuente: string) => `${oscuro() ? "o" : "c"}|${fuente}`;

const fencePorDefecto = md.renderer.rules.fence!;
md.renderer.rules.fence = (tokens, idx, opciones, env, self) => {
  const t = tokens[idx];
  if (t.info.trim().toLowerCase() !== "mermaid") return fencePorDefecto(tokens, idx, opciones, env, self);
  const hecho = diagramas.get(claveDiagrama(t.content));
  if (hecho) return `<div class="diagrama" data-hecho="1">${hecho}</div>`;
  // Codificada: DOMPurify quita atributos con `-->` (las flechas de Mermaid).
  return `<div class="diagrama" data-mermaid="${encodeURIComponent(t.content)}"><pre>${md.utils.escapeHtml(t.content)}</pre></div>`;
};

let mermaidListo: Promise<typeof import("mermaid").default> | null = null;
let temaMermaid = "";
let contador = 0;

/** Dibuja los diagramas Mermaid pendientes dentro de `contenedor` (tras pintar la vista previa). */
export async function dibujarDiagramas(contenedor: HTMLElement) {
  const pendientes = [...contenedor.querySelectorAll<HTMLElement>(".diagrama[data-mermaid]")];
  if (!pendientes.length) return;
  mermaidListo ??= import("mermaid").then((m) => m.default);
  const mermaid = await mermaidListo;
  const tema = oscuro() ? "dark" : "neutral";
  if (tema !== temaMermaid) {
    temaMermaid = tema;
    mermaid.initialize({
      startOnLoad: false,
      securityLevel: "strict",
      theme: tema,
      htmlLabels: false,
      flowchart: { htmlLabels: false },
      fontFamily: '"Inter Variable", system-ui, sans-serif',
    });
  }
  for (const el of pendientes) {
    let fuente = "";
    try {
      fuente = decodeURIComponent(el.dataset.mermaid ?? "");
    } catch {
      continue;
    }
    const clave = claveDiagrama(fuente);
    try {
      let svg = diagramas.get(clave);
      if (!svg) {
        const r = await mermaid.render(`diagrama-${++contador}`, fuente);
        svg = DOMPurify.sanitize(r.svg, { USE_PROFILES: { svg: true, svgFilters: true }, ADD_TAGS: ["style"] });
        diagramas.set(clave, svg);
      }
      if (!el.isConnected) continue;
      el.innerHTML = svg;
      el.removeAttribute("data-mermaid");
      el.dataset.hecho = "1";
    } catch (e) {
      el.classList.add("diagrama-error");
      el.title = e instanceof Error ? e.message : String(e);
    }
  }
}

export function renderizar(markdown: string): string {
  return DOMPurify.sanitize(md.render(markdown), { ADD_ATTR: ["target"], ADD_TAGS: ["style"] });
}
