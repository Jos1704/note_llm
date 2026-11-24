// Prefijos de la captura rápida: `#etiqueta` y `@proyecto` en el título o en la primera línea
// del texto se convierten en etiquetas y proyecto, y se quitan del texto.
// `# Título` (con espacio) sigue siendo un encabezado de Markdown, no una etiqueta.
import type { Proyecto } from "./api";
import { normalizarEtiqueta } from "./api";

interface Encontrados {
  etiquetas: string[];
  /** `undefined`: no había `@proyecto`; `null`: la Bandeja. */
  proyecto: string | null | undefined;
}

const TOKEN = /(^|\s)([#@])([\p{L}_][\p{L}\p{N}_\-/.]*)/gu;
const TOKEN_FINAL = /(^|\s)([#@])([\p{L}\p{N}_\-/.]*)$/u;

const sinAcentos = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

/** Forma corta de un nombre de proyecto para escribirlo tras `@` («Mi app» → `mi-app`). */
export const slugProyecto = (s: string) =>
  sinAcentos(s)
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");

const esBandeja = (p: Proyecto) => p.id.endsWith(":bandeja");

/** Proyecto por nombre visible o por nombre de carpeta, sin importar mayúsculas ni acentos. */
export function buscarProyecto(nombre: string, proyectos: Proyecto[]): Proyecto | undefined {
  const s = slugProyecto(nombre);
  if (!s) return undefined;
  return (
    proyectos.find((p) => slugProyecto(p.nombre) === s) ??
    proyectos.find((p) => slugProyecto(p.id.split(":")[1] ?? "") === s)
  );
}

/** Extrae los prefijos de una línea. Los `@` que no son un proyecto se dejan en el texto. */
function extraerDeLinea(linea: string, proyectos: Proyecto[], r: Encontrados): string {
  return linea
    .replace(TOKEN, (todo, espacio: string, signo: string, nombre: string) => {
      if (signo === "#") {
        const e = normalizarEtiqueta(nombre);
        if (!e) return todo;
        if (!r.etiquetas.includes(e)) r.etiquetas.push(e);
        return espacio;
      }
      const p = buscarProyecto(nombre, proyectos);
      if (!p) return todo;
      r.proyecto = esBandeja(p) ? null : p.id;
      return espacio;
    })
    .replace(/[ \t]{2,}/g, " ")
    .trim();
}

/** Aplica los prefijos del título y de la primera línea no vacía del texto.
 * `proyecto` es `undefined` si no había `@proyecto` (se usa el elegido en el selector). */
export function extraerPrefijos(
  titulo: string,
  cuerpo: string,
  proyectos: Proyecto[],
): { titulo: string; cuerpo: string; etiquetas: string[]; proyecto: string | null | undefined } {
  const r: Encontrados = { etiquetas: [], proyecto: undefined };
  const t = extraerDeLinea(titulo, proyectos, r);
  const lineas = cuerpo.split("\n");
  const i = lineas.findIndex((l) => l.trim());
  if (i >= 0 && !/^\s*(!\[|<|```|#\s)/.test(lineas[i])) {
    const limpia = extraerDeLinea(lineas[i], proyectos, r);
    if (limpia) lineas[i] = limpia;
    else {
      lineas.splice(i, 1);
      if (i < lineas.length && !lineas[i].trim()) lineas.splice(i, 1); // línea en blanco que quedaba
    }
  }
  return { titulo: t, cuerpo: lineas.join("\n"), etiquetas: r.etiquetas, proyecto: r.proyecto };
}

// ------------------------------------------------------------------ autocompletado

export interface FuentePrefijos {
  proyectos: Proyecto[];
  etiquetas: string[];
}

type Campo = HTMLInputElement | HTMLTextAreaElement;

/**
 * Sugerencias de `#etiqueta` / `@proyecto` mientras se escribe en `campo` (solo en la primera
 * línea, que es donde se aplican). Se muestran como fichas en `caja`; `Tab` acepta la primera.
 * Devuelve una función que indica si hay sugerencias visibles (para no cerrar con `Esc`).
 */
export function autocompletarPrefijos(campo: Campo, caja: HTMLElement, fuente: () => FuentePrefijos): () => boolean {
  let opciones: { mostrar: string; insertar: string }[] = [];
  let inicioToken = 0;

  const ocultar = () => {
    opciones = [];
    caja.hidden = true;
    caja.replaceChildren();
  };

  const aceptar = (i: number) => {
    const o = opciones[i];
    if (!o) return;
    const fin = campo.selectionStart ?? campo.value.length;
    campo.value = campo.value.slice(0, inicioToken) + o.insertar + " " + campo.value.slice(fin);
    const pos = inicioToken + o.insertar.length + 1;
    campo.setSelectionRange(pos, pos);
    campo.dispatchEvent(new Event("input", { bubbles: true }));
    ocultar();
    campo.focus();
  };

  const actualizar = () => {
    const pos = campo.selectionStart ?? 0;
    const antes = campo.value.slice(0, pos);
    const primera = campo.value.search(/\S/);
    const finPrimera = primera < 0 ? Infinity : campo.value.indexOf("\n", primera);
    if (campo instanceof HTMLTextAreaElement && finPrimera >= 0 && pos > finPrimera) return ocultar();
    const m = TOKEN_FINAL.exec(antes);
    if (!m) return ocultar();
    inicioToken = pos - m[2].length - m[3].length;
    const consulta = sinAcentos(m[3]);
    const { proyectos, etiquetas } = fuente();
    if (m[2] === "#") {
      opciones = etiquetas
        .filter((e) => sinAcentos(e).startsWith(consulta))
        .slice(0, 6)
        .map((e) => ({ mostrar: `#${e}`, insertar: `#${e}` }));
    } else {
      opciones = proyectos
        .filter((p) => !esBandeja(p))
        .filter((p) => slugProyecto(p.nombre).startsWith(slugProyecto(consulta)) || sinAcentos(p.nombre).includes(consulta))
        .slice(0, 6)
        .map((p) => ({ mostrar: `@${p.nombre}`, insertar: `@${slugProyecto(p.nombre)}` }));
    }
    if (!opciones.length) return ocultar();
    caja.replaceChildren(
      ...opciones.map((o, i) => {
        const b = document.createElement("button");
        b.type = "button";
        b.className = "sugerencia";
        b.textContent = o.mostrar;
        b.addEventListener("mousedown", (e) => {
          e.preventDefault(); // no quitar el foco al campo
          aceptar(i);
        });
        return b;
      }),
    );
    caja.hidden = false;
  };

  campo.addEventListener("input", actualizar);
  campo.addEventListener("click", actualizar);
  campo.addEventListener("blur", () => setTimeout(ocultar, 100));
  campo.addEventListener("keydown", (e) => {
    const ke = e as KeyboardEvent;
    if (!opciones.length) return;
    if (ke.key === "Tab") {
      ke.preventDefault();
      aceptar(0);
    } else if (ke.key === "Escape") {
      ke.preventDefault();
      ke.stopPropagation();
      ocultar();
    }
  });
  ocultar();
  return () => opciones.length > 0;
}
