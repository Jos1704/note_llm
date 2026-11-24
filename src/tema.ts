// Tema «Sereno»: fuentes incluidas en la app + paleta (Bruma, Lavanda, Marea, EVA-01, Vidrio) y modo
// (sistema, claro, oscuro). EVA-01 y Vidrio además cambian formas y transparencias (ver estilos.css).
// Lo comparten la ventana principal y la de nota rápida (mismo almacenamiento local).
import "@fontsource-variable/inter";
import "@fontsource-variable/literata/opsz.css";
import "@fontsource-variable/literata/opsz-italic.css";
import "@fontsource-variable/jetbrains-mono";
import "@fontsource-variable/oswald";

export type Paleta = "bruma" | "lavanda" | "marea" | "eva" | "vidrio";
export type ModoTema = "auto" | "light" | "dark";

/** Colores de muestra: fondo claro, acento, fondo oscuro (los mismos que en estilos.css). */
export const PALETAS: { valor: Paleta; nombre: string; muestra: [string, string, string] }[] = [
  { valor: "bruma", nombre: "Bruma", muestra: ["#f6f7f4", "#3f6b5b", "#171c1a"] },
  { valor: "lavanda", nombre: "Lavanda", muestra: ["#f7f6fa", "#5e5596", "#1a1820"] },
  { valor: "marea", nombre: "Marea", muestra: ["#f4f7f8", "#2f6680", "#161b1f"] },
  { valor: "eva", nombre: "EVA-01", muestra: ["#7b4fd6", "#9cff3a", "#ff7a1a"] },
  { valor: "vidrio", nombre: "Vidrio", muestra: ["#7fd1b8", "#8fb8ff", "#c58bf0"] },
];

export const MODOS: { valor: ModoTema; nombre: string }[] = [
  { valor: "auto", nombre: "Sistema" },
  { valor: "light", nombre: "Claro" },
  { valor: "dark", nombre: "Oscuro" },
];

function leer(clave: string): string | null {
  try {
    return localStorage.getItem(clave);
  } catch {
    return null;
  }
}

function guardar(clave: string, valor: string | null) {
  try {
    if (valor === null) localStorage.removeItem(clave);
    else localStorage.setItem(clave, valor);
  } catch {
    /* sin almacenamiento: el tema dura solo esta sesión */
  }
}

export function temaActual(): { paleta: Paleta; modo: ModoTema } {
  const p = leer("paleta");
  const m = leer("tema");
  return {
    paleta: PALETAS.some((x) => x.valor === p) ? (p as Paleta) : "bruma",
    modo: m === "light" || m === "dark" ? m : "auto",
  };
}

export function aplicarTema(paleta: Paleta, modo: ModoTema) {
  const raiz = document.documentElement;
  raiz.dataset.paleta = paleta;
  if (modo === "auto") delete raiz.dataset.theme;
  else raiz.dataset.theme = modo;
}

export function cambiarTema(paleta: Paleta, modo: ModoTema) {
  guardar("paleta", paleta);
  guardar("tema", modo === "auto" ? null : modo);
  aplicarTema(paleta, modo);
}

// Al cargar: aplicar lo guardado antes de pintar nada.
{
  const { paleta, modo } = temaActual();
  aplicarTema(paleta, modo);
}
