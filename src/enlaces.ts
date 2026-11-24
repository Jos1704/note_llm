// Enlaces entre notas `[[Título]]`: la misma resolución que el backend (`relaciones.rs`), para
// pintar al instante los enlaces que no llevan a ninguna nota y abrir el destino al hacer clic.
import type { NotaResumen, Plantilla } from "./api";

const ACENTOS: Record<string, string> = {
  á: "a", à: "a", ä: "a", â: "a",
  é: "e", è: "e", ë: "e", ê: "e",
  í: "i", ì: "i", ï: "i", î: "i",
  ó: "o", ò: "o", ö: "o", ô: "o",
  ú: "u", ù: "u", ü: "u", û: "u",
  ñ: "n",
};

/** Igual que `store::slug` en Rust. */
export function slug(texto: string): string {
  let out = "";
  for (const ch of texto.trim().toLowerCase()) {
    const c = ACENTOS[ch] ?? ch;
    if (/^[a-z0-9]$/.test(c)) out += c;
    else if (out && !out.endsWith("-")) out += "-";
    if (out.length >= 80) break;
  }
  return out.replace(/-+$/, "");
}

const nombreArchivo = (id: string) => (id.split(/[/:]/).pop() ?? id).replace(/\.md$/, "");

export type Resolutor = (destino: string, desdeProyecto?: string | null) => string | null;

export function crearResolutor(notas: NotaResumen[]): Resolutor {
  const porClave = new Map<string, { id: string; proyecto: string }[]>();
  for (const n of notas) {
    for (const k of new Set([slug(n.titulo), slug(nombreArchivo(n.id))])) {
      if (!k) continue;
      const lista = porClave.get(k) ?? [];
      lista.push({ id: n.id, proyecto: n.proyecto });
      porClave.set(k, lista);
    }
  }
  return (destino, desde) => {
    const c = porClave.get(slug(destino));
    if (!c) return null;
    return (c.find((x) => x.proyecto === desde) ?? c[0]).id;
  };
}

/** Parte un `[[destino#sección|texto]]` (sin corchetes). */
export function partirWikilink(dentro: string): { destino: string; texto: string } {
  const [objetivo, alias] = dentro.split("|", 2);
  const destino = objetivo.split("#")[0].trim();
  return { destino, texto: (alias ?? objetivo).trim() };
}

// ------------------------------------------------------------------ plantillas

const pad = (n: number) => String(n).padStart(2, "0");

export function fechaHoy(d = new Date()): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/**
 * Rellena las variables de una plantilla. `{{cursor}}` se quita y su posición se devuelve en
 * `cursor` (o `null` si no está).
 */
export function rellenar(texto: string, vars: { proyecto: string; titulo: string }): { texto: string; cursor: number | null } {
  const d = new Date();
  const valores: Record<string, string> = {
    fecha: fechaHoy(d),
    hora: `${pad(d.getHours())}:${pad(d.getMinutes())}`,
    proyecto: vars.proyecto,
    titulo: vars.titulo,
  };
  const lleno = texto.replace(/\{\{\s*(fecha|hora|proyecto|titulo)\s*\}\}/g, (_, k: string) => valores[k]);
  const i = lleno.search(/\{\{\s*cursor\s*\}\}/);
  if (i < 0) return { texto: lleno, cursor: null };
  return { texto: lleno.replace(/\{\{\s*cursor\s*\}\}/, ""), cursor: i };
}

/** Título que tendrá una nota creada con la plantilla (vacío si la plantilla no trae `title`). */
export function tituloDePlantilla(p: Plantilla, proyecto: string): string {
  return p.titulo ? rellenar(p.titulo, { proyecto, titulo: "" }).texto.trim() : "";
}
