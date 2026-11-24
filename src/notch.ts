// Widget del notch (macOS). Rust detecta el hover (o el atajo global) y agranda la ventana; aquí
// se anima el panel, se captura la nota y se pide plegar la ventana (`notch_cerrar`) al terminar.
import { aplicarTema, temaActual } from "./tema"; // fuentes + paleta y modo de la app
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { api, crearNotaRapida, imagenAdjunta, type Estado } from "./api";
import { autocompletarPrefijos } from "./prefijos";
import { comoBloque, fuentePrefijos, insertarEnCursor, mensajeError, opcionesProyecto, pegarImagenes } from "./ui";

interface Apertura {
  notchAncho: number;
  notchAlto: number;
  hayNotch: boolean;
  /** Abierto con el atajo: foco directo al texto. */
  foco: boolean;
}

// Si se cambia el tema en la ventana principal, el widget (que sigue cargado) lo toma al momento.
window.addEventListener("storage", (e) => {
  if (e.key === "paleta" || e.key === "tema" || e.key === null) {
    const { paleta, modo } = temaActual();
    aplicarTema(paleta, modo);
  }
});

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const panel = $<HTMLDivElement>("panel");
const form = $<HTMLFormElement>("form");
const titulo = $<HTMLInputElement>("titulo");
const cuerpo = $<HTMLTextAreaElement>("cuerpo");
const proyecto = $<HTMLSelectElement>("proyecto");
const estadoEl = $<HTMLSpanElement>("estado");
const guardar = $<HTMLButtonElement>("guardar");
const sugerencias = $<HTMLDivElement>("sugerencias");
const ayuda = estadoEl.innerHTML;

const BORRADOR = "notch-borrador";
const ULTIMO_PROYECTO = "notch-proyecto";
const RETARDO_CIERRE = 450; // ms tras sacar el ratón
const DURACION_CIERRE = 360; // ms de la animación de plegado

let abierto = false;
let cerrando = false;
/** Hay una captura en curso (diálogo de permisos, selección de pantalla…): no se pliega solo. */
let ocupado = false;
let temporizador: number | undefined;
let estado: Estado | null = null;

function leer(clave: string): string | null {
  try {
    return localStorage.getItem(clave);
  } catch {
    return null;
  }
}
function escribir(clave: string, valor: string | null) {
  try {
    if (valor === null) localStorage.removeItem(clave);
    else localStorage.setItem(clave, valor);
  } catch {
    /* sin almacenamiento: el borrador solo vive en memoria */
  }
}

// --- borrador: se conserva si se cierra sin guardar
function guardarBorrador() {
  const b = { titulo: titulo.value, cuerpo: cuerpo.value };
  escribir(BORRADOR, b.titulo || b.cuerpo ? JSON.stringify(b) : null);
}
function cargarBorrador() {
  try {
    const b = JSON.parse(leer(BORRADOR) ?? "null");
    if (b) {
      titulo.value = b.titulo ?? "";
      cuerpo.value = b.cuerpo ?? "";
    }
  } catch {
    /* borrador corrupto: se ignora */
  }
}
const vacio = () => !titulo.value.trim() && !cuerpo.value.trim();

function mostrarEstado(texto: string | null, clase = "") {
  if (texto === null) estadoEl.innerHTML = ayuda;
  else estadoEl.textContent = texto;
  estadoEl.className = clase;
  estadoEl.title = texto ?? "";
}

async function cargarProyectos() {
  try {
    estado = await api.estado();
    proyecto.innerHTML = opcionesProyecto(estado);
  } catch {
    /* sin biblioteca: queda «Bandeja» */
  }
  const ultimo = leer(ULTIMO_PROYECTO);
  if (ultimo && [...proyecto.options].some((o) => o.value === ultimo)) proyecto.value = ultimo;
}

function enfocarTexto() {
  cuerpo.focus();
  const fin = cuerpo.value.length;
  cuerpo.setSelectionRange(fin, fin);
}

// --- abrir / cerrar
function abrir(a: Apertura) {
  clearTimeout(temporizador);
  cerrando = false;
  abierto = true;
  const tema = temaActual(); // por si el aviso «storage» no llegó (otra ventana, otro proceso)
  aplicarTema(tema.paleta, tema.modo);
  document.documentElement.style.setProperty("--notch-ancho", `${a.notchAncho}px`);
  document.documentElement.style.setProperty("--notch-alto", `${a.hayNotch ? a.notchAlto : 0}px`);
  mostrarEstado(null);
  guardar.disabled = false;
  void panel.offsetWidth; // aplica el tamaño inicial antes de animar
  panel.classList.add("abierto");
  cargarProyectos();
  if (a.foco) setTimeout(enfocarTexto, 60);
}

function cerrar(devolverFoco: boolean) {
  if (!abierto || cerrando) return;
  clearTimeout(temporizador);
  cerrando = true;
  guardarBorrador();
  (document.activeElement as HTMLElement | null)?.blur();
  panel.classList.remove("abierto");
  setTimeout(() => {
    abierto = false;
    cerrando = false;
    invoke("notch_cerrar", { devolverFoco }).catch(() => {});
  }, DURACION_CIERRE);
}

/** Cierre por salir con el ratón: solo si no se está escribiendo. */
function programarCierre() {
  clearTimeout(temporizador);
  temporizador = window.setTimeout(() => {
    if (!ocupado && (!document.hasFocus() || vacio())) cerrar(false);
  }, RETARDO_CIERRE);
}

async function enviar() {
  if (cerrando) return;
  if (vacio()) {
    cuerpo.focus();
    return;
  }
  guardar.disabled = true;
  try {
    const t = await crearNotaRapida(
      { titulo: titulo.value.trim(), cuerpo: cuerpo.value, proyecto: proyecto.value || null },
      [],
      estado?.proyectos ?? [],
    );
    escribir(ULTIMO_PROYECTO, proyecto.value);
    titulo.value = "";
    cuerpo.value = "";
    escribir(BORRADOR, null);
    await emit("notas-cambiaron").catch(() => {});
    mostrarEstado(`Guardada: ${t}`, "ok");
    setTimeout(() => cerrar(true), 650);
  } catch (e) {
    mostrarEstado(mensajeError(e), "error");
    guardar.disabled = false;
  }
}

// --- capturar lo que tienes delante
const escaparEnlace = (s: string) => s.replace(/([[\]\\])/g, "\\$1");

const acciones: Record<string, () => Promise<string | null>> = {
  async portapapeles() {
    const p = await api.portapapeles();
    if (p.tipo === "texto") return p.texto;
    if (p.tipo === "imagen") return comoBloque(cuerpo, imagenAdjunta(p.nombre));
    throw "El portapapeles está vacío";
  },
  async pestana() {
    const { url, titulo: t } = await api.pestanaNavegador();
    return `[${escaparEnlace(t || url)}](${url})`;
  },
  async seleccion() {
    return await api.textoSeleccionado();
  },
  async captura() {
    const c = await api.capturarPantalla();
    if (!c) return null; // cancelada
    const texto = c.texto.trim()
      ? `\n\n<details>\n<summary>Texto de la captura</summary>\n\n${c.texto.trim()}\n\n</details>`
      : "";
    return comoBloque(cuerpo, imagenAdjunta(c.nombre, "Captura") + texto);
  },
};

async function ejecutarAccion(boton: HTMLButtonElement) {
  const accion = acciones[boton.dataset.accion ?? ""];
  if (!accion || ocupado) return;
  ocupado = true;
  clearTimeout(temporizador);
  boton.disabled = true;
  mostrarEstado("…");
  try {
    const texto = await accion();
    if (texto) {
      cuerpo.focus();
      insertarEnCursor(cuerpo, texto);
    }
    mostrarEstado(null);
  } catch (e) {
    mostrarEstado(mensajeError(e), "error");
  } finally {
    ocupado = false;
    boton.disabled = false;
  }
}

// --- eventos
form.addEventListener("submit", (e) => {
  e.preventDefault();
  enviar();
});
form.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
    e.preventDefault();
    enviar();
  } else if (e.key === "Escape") {
    e.preventDefault();
    cerrar(true);
  }
});
form.addEventListener("input", guardarBorrador);
proyecto.addEventListener("change", () => escribir(ULTIMO_PROYECTO, proyecto.value));
for (const b of document.querySelectorAll<HTMLButtonElement>(".accion")) {
  b.addEventListener("click", () => ejecutarAccion(b));
}
autocompletarPrefijos(titulo, sugerencias, () => fuentePrefijos(estado)());
autocompletarPrefijos(cuerpo, sugerencias, () => fuentePrefijos(estado)());
pegarImagenes(cuerpo, (e) => mostrarEstado(mensajeError(e), "error"));

// Clic fuera (otra app/ventana): se pliega y el borrador queda guardado.
window.addEventListener("blur", () => {
  if (abierto && !ocupado) cerrar(false);
});

listen<Apertura>("notch:abrir", (e) => abrir(e.payload));
listen("notch:fuera", () => abierto && programarCierre());
listen("notch:dentro", () => clearTimeout(temporizador));
listen("notch:cerrar", () => cerrar(true));
listen("notch:foco", () => abierto && enfocarTexto());

cargarBorrador();
