import { api, imagenAdjunta, type Estado, type Lugar } from "./api";
import { autocompletarPrefijos, type FuentePrefijos } from "./prefijos";
import { icono } from "./iconos";
// Piezas de interfaz reutilizables: menús, diálogos y avisos (sin diálogos nativos).

const capa = () => document.getElementById("capa")!;

export function esc(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

let cerrarActual: (() => void) | null = null;

export function cerrarCapa() {
  cerrarActual?.();
}

function abrirCapa(el: HTMLElement, alCerrar?: () => void) {
  cerrarCapa();
  capa().append(el);
  const fuera = (e: MouseEvent) => {
    if (!el.contains(e.target as Node)) cerrar();
  };
  const tecla = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      cerrar();
    }
  };
  const cerrar = () => {
    el.remove();
    document.removeEventListener("mousedown", fuera, true);
    document.removeEventListener("keydown", tecla, true);
    cerrarActual = null;
    alCerrar?.();
  };
  // Se registra en el siguiente ciclo para no cerrar con el mismo clic que lo abrió.
  setTimeout(() => {
    document.addEventListener("mousedown", fuera, true);
    document.addEventListener("keydown", tecla, true);
  });
  cerrarActual = cerrar;
  return cerrar;
}

export interface OpcionMenu {
  texto: string;
  icono?: string;
  peligro?: boolean;
  desactivado?: boolean;
  accion: () => void;
}

export function menu(junto: HTMLElement | { x: number; y: number }, opciones: OpcionMenu[]) {
  const el = document.createElement("div");
  el.className = "menu";
  for (const o of opciones) {
    const b = document.createElement("button");
    b.className = o.peligro ? "peligro" : "";
    b.disabled = !!o.desactivado;
    b.innerHTML = `${o.icono ?? ""}<span>${esc(o.texto)}</span>`;
    b.onclick = () => {
      cerrar();
      o.accion();
    };
    el.append(b);
  }
  const cerrar = abrirCapa(el);
  const r = junto instanceof HTMLElement ? junto.getBoundingClientRect() : { left: junto.x, bottom: junto.y };
  const ancho = 220;
  el.style.left = `${Math.min(r.left, innerWidth - ancho - 8)}px`;
  el.style.top = `${Math.min(r.bottom + 4, innerHeight - el.offsetHeight - 8)}px`;
}

/** Panel flotante junto a un botón (se cierra al hacer clic fuera o con Escape). */
export function panelFlotante(junto: HTMLElement, el: HTMLElement, alCerrar?: () => void): () => void {
  el.classList.add("flotante");
  const cerrar = abrirCapa(el, alCerrar);
  const r = junto.getBoundingClientRect();
  const ancho = el.offsetWidth;
  el.style.left = `${Math.max(8, Math.min(r.right - ancho, innerWidth - ancho - 8))}px`;
  el.style.top = `${Math.min(r.bottom + 6, innerHeight - el.offsetHeight - 8)}px`;
  return cerrar;
}

export function dialogo(html: string): { el: HTMLElement; cerrar: () => void; alCerrar: Promise<void> } {
  const fondo = document.createElement("div");
  fondo.className = "fondo";
  const el = document.createElement("div");
  el.className = "dialogo";
  el.innerHTML = html;
  fondo.append(el);
  let resolver!: () => void;
  const alCerrar = new Promise<void>((r) => (resolver = r));
  const cerrar = abrirCapa(fondo, resolver);
  fondo.addEventListener("mousedown", (e) => {
    if (e.target === fondo) cerrar();
  });
  return { el, cerrar, alCerrar };
}

export function pedirTexto(op: { titulo: string; valor?: string; placeholder?: string; boton: string }): Promise<string | null> {
  return new Promise((resolve) => {
    let resultado: string | null = null;
    const { el, cerrar, alCerrar } = dialogo(`
      <h3>${esc(op.titulo)}</h3>
      <input class="campo" value="${esc(op.valor ?? "")}" placeholder="${esc(op.placeholder ?? "")}" spellcheck="false">
      <div class="botones"><button class="sec" data-r="no">Cancelar</button><button class="pri" data-r="si">${esc(op.boton)}</button></div>`);
    const input = el.querySelector("input")!;
    const aceptar = () => {
      if (!input.value.trim()) return input.focus();
      resultado = input.value.trim();
      cerrar();
    };
    input.addEventListener("keydown", (e) => e.key === "Enter" && aceptar());
    el.querySelector<HTMLButtonElement>('[data-r="si"]')!.onclick = aceptar;
    el.querySelector<HTMLButtonElement>('[data-r="no"]')!.onclick = cerrar;
    alCerrar.then(() => resolve(resultado));
    input.focus();
    input.select();
  });
}

export function confirmar(op: { titulo: string; texto: string; boton: string; peligro?: boolean }): Promise<boolean> {
  return new Promise((resolve) => {
    let ok = false;
    const { el, cerrar, alCerrar } = dialogo(`
      <h3>${esc(op.titulo)}</h3>
      <p>${op.texto}</p>
      <div class="botones"><button class="sec" data-r="no">Cancelar</button><button class="${op.peligro ? "peligro" : "pri"}" data-r="si">${esc(op.boton)}</button></div>`);
    const si = el.querySelector<HTMLButtonElement>('[data-r="si"]')!;
    si.onclick = () => {
      ok = true;
      cerrar();
    };
    el.querySelector<HTMLButtonElement>('[data-r="no"]')!.onclick = cerrar;
    alCerrar.then(() => resolve(ok));
    si.focus();
  });
}

export interface ResultadoEtiqueta {
  accion: "guardar" | "borrar";
  nombre: string;
  color: string;
}

export function editarEtiqueta(op: {
  nombre?: string;
  color: string;
  paleta: string[];
  existente: boolean;
}): Promise<ResultadoEtiqueta | null> {
  return new Promise((resolve) => {
    let resultado: ResultadoEtiqueta | null = null;
    let color = op.color;
    const muestras = op.paleta
      .map((c) => `<button class="muestra" data-color="${c}" style="--c:${c}" title="${c}"></button>`)
      .join("");
    const { el, cerrar, alCerrar } = dialogo(`
      <h3>${op.existente ? "Editar etiqueta" : "Nueva etiqueta"}</h3>
      <input class="campo" value="${esc(op.nombre ?? "")}" placeholder="nombre" spellcheck="false">
      <div class="muestras">${muestras}<label class="muestra libre" title="Otro color"><input type="color" value="${color}"></label></div>
      <div class="botones">
        ${op.existente ? '<button class="peligro izq" data-r="borrar">Eliminar</button>' : ""}
        <button class="sec" data-r="no">Cancelar</button><button class="pri" data-r="si">Guardar</button>
      </div>`);
    const input = el.querySelector<HTMLInputElement>("input.campo")!;
    const marcar = () =>
      el.querySelectorAll<HTMLElement>(".muestra[data-color]").forEach((m) => m.classList.toggle("activa", m.dataset.color === color));
    marcar();
    el.querySelectorAll<HTMLElement>(".muestra[data-color]").forEach((m) => {
      m.onclick = () => {
        color = m.dataset.color!;
        marcar();
      };
    });
    const libre = el.querySelector<HTMLInputElement>('input[type="color"]')!;
    libre.oninput = () => {
      color = libre.value;
      marcar();
    };
    const guardar = () => {
      if (!input.value.trim()) return input.focus();
      resultado = { accion: "guardar", nombre: input.value.trim(), color };
      cerrar();
    };
    input.addEventListener("keydown", (e) => e.key === "Enter" && guardar());
    el.querySelector<HTMLButtonElement>('[data-r="si"]')!.onclick = guardar;
    el.querySelector<HTMLButtonElement>('[data-r="no"]')!.onclick = cerrar;
    const borrar = el.querySelector<HTMLButtonElement>('[data-r="borrar"]');
    if (borrar) {
      borrar.onclick = () => {
        resultado = { accion: "borrar", nombre: op.nombre ?? "", color };
        cerrar();
      };
    }
    alCerrar.then(() => resolve(resultado));
    input.focus();
    input.select();
  });
}

let temporizadorAviso = 0;
export function aviso(texto: string, error = false) {
  const el = document.getElementById("aviso")!;
  el.textContent = texto;
  el.classList.toggle("error", error);
  el.classList.add("visible");
  clearTimeout(temporizadorAviso);
  temporizadorAviso = window.setTimeout(() => el.classList.remove("visible"), error ? 6000 : 2800);
}

/** Texto de error de un `invoke` fallido. */
export function mensajeError(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}

export interface NotaRapida {
  titulo: string;
  cuerpo: string;
  proyecto: string | null;
}

/** `<option>`s del selector de proyecto de la nota rápida. Vacío = Bandeja (disco si está conectado; si no, este equipo). */
export function opcionesProyecto(estado: Estado): string {
  const grupo = (lugar: Lugar, etiqueta: string) => {
    const ps = estado.proyectos.filter((p) => p.lugar === lugar && !p.id.endsWith(":bandeja"));
    if (!ps.length) return "";
    return `<optgroup label="${etiqueta}">${ps.map((p) => `<option value="${esc(p.id)}">${esc(p.nombre)}</option>`).join("")}</optgroup>`;
  };
  return `<option value="">Bandeja</option>` + (estado.disco?.conectado ? grupo("disco", "Disco") : "") + grupo("equipo", "Este equipo");
}

/** Diálogo de captura rápida: título, texto y proyecto. `inicial` repone lo escrito si guardar falló. */
/** Fuente del autocompletado de `#etiqueta` / `@proyecto` a partir del estado de la biblioteca. */
export const fuentePrefijos = (estado: Estado | null) => (): FuentePrefijos => ({
  proyectos: estado?.proyectos ?? [],
  etiquetas: estado?.etiquetas.map((e) => e.nombre) ?? [],
});

/** Inserta texto en la posición del cursor de un campo. */
export function insertarEnCursor(campo: HTMLTextAreaElement | HTMLInputElement, texto: string) {
  const ini = campo.selectionStart ?? campo.value.length;
  const fin = campo.selectionEnd ?? ini;
  campo.value = campo.value.slice(0, ini) + texto + campo.value.slice(fin);
  const pos = ini + texto.length;
  campo.setSelectionRange(pos, pos);
  campo.dispatchEvent(new Event("input", { bubbles: true }));
}

/** Bloque Markdown separado por líneas en blanco del texto que lo rodea. */
export function comoBloque(campo: HTMLTextAreaElement, bloque: string): string {
  const antes = campo.value.slice(0, campo.selectionStart ?? campo.value.length);
  const pre = !antes || antes.endsWith("\n\n") ? "" : antes.endsWith("\n") ? "\n" : "\n\n";
  return `${pre}${bloque}\n\n`;
}

/** Pegar una imagen en un textarea: se guarda como adjunto temporal y se inserta su Markdown. */
export function pegarImagenes(campo: HTMLTextAreaElement, alError: (e: unknown) => void) {
  campo.addEventListener("paste", async (e) => {
    const archivos = [...(e.clipboardData?.files ?? [])].filter((f) => f.type.startsWith("image/"));
    if (!archivos.length) return;
    e.preventDefault();
    for (const f of archivos) {
      try {
        const ext = f.type.split("/")[1]?.replace("jpeg", "jpg") || "png";
        const nombre = await api.guardarTemporal(new Uint8Array(await f.arrayBuffer()), ext);
        insertarEnCursor(campo, comoBloque(campo, imagenAdjunta(nombre)));
      } catch (err) {
        alError(err);
      }
    }
  });
}

export function pedirNotaRapida(opciones: string, inicial?: NotaRapida, fuente?: () => FuentePrefijos): Promise<NotaRapida | null> {
  return new Promise((resolve) => {
    let resultado: NotaRapida | null = null;
    const { el, cerrar, alCerrar } = dialogo(`
      <div class="dialogo-cab"><span class="dialogo-ico">${icono.mas}</span><h3>Nota rápida</h3></div>
      <input class="campo titular" data-c="titulo" placeholder="Título (opcional)" spellcheck="false">
      <textarea class="campo area" data-c="cuerpo" placeholder="Escribe en Markdown… (#etiqueta o @proyecto en la primera línea)" spellcheck="false"></textarea>
      <div class="sugerencias" hidden></div>
      <div class="botones">
        <select class="campo izq" data-c="proyecto" title="Proyecto">${opciones}</select>
        <button class="sec" data-r="no">Cancelar</button><button class="pri" data-r="si">Guardar</button>
      </div>
      <p class="pista"><kbd>Ctrl</kbd>+<kbd>Enter</kbd> guardar · <kbd>Esc</kbd> cerrar</p>`);
    el.classList.add("dialogo-ancho");
    const titulo = el.querySelector<HTMLInputElement>('[data-c="titulo"]')!;
    const cuerpo = el.querySelector<HTMLTextAreaElement>('[data-c="cuerpo"]')!;
    const proyecto = el.querySelector<HTMLSelectElement>('[data-c="proyecto"]')!;
    const sugerencias = el.querySelector<HTMLDivElement>(".sugerencias")!;
    if (fuente) {
      autocompletarPrefijos(titulo, sugerencias, fuente);
      autocompletarPrefijos(cuerpo, sugerencias, fuente);
    }
    pegarImagenes(cuerpo, (e) => aviso(mensajeError(e), true));
    if (inicial) {
      titulo.value = inicial.titulo;
      cuerpo.value = inicial.cuerpo;
      proyecto.value = inicial.proyecto ?? "";
    }
    const guardar = () => {
      if (!titulo.value.trim() && !cuerpo.value.trim()) return cuerpo.focus();
      resultado = { titulo: titulo.value.trim(), cuerpo: cuerpo.value, proyecto: proyecto.value || null };
      cerrar();
    };
    titulo.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        if (e.ctrlKey || e.metaKey) guardar();
        else cuerpo.focus();
      }
    });
    el.addEventListener("keydown", (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
        e.preventDefault();
        guardar();
      }
    });
    el.querySelector<HTMLButtonElement>('[data-r="si"]')!.onclick = guardar;
    el.querySelector<HTMLButtonElement>('[data-r="no"]')!.onclick = cerrar;
    alCerrar.then(() => resolve(resultado));
    titulo.focus();
  });
}
