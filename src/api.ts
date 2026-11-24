import { invoke } from "@tauri-apps/api/core";
import { extraerPrefijos } from "./prefijos";

export type Lugar = "equipo" | "disco";

export interface Proyecto {
  id: string;
  lugar: Lugar;
  nombre: string;
  notas: number;
}

export interface Etiqueta {
  nombre: string;
  color: string;
  notas: number;
}

export interface NotaResumen {
  id: string;
  proyecto: string;
  lugar: Lugar;
  titulo: string;
  etiquetas: string[];
  modificado: number;
  extracto: string;
}

export interface Nota extends NotaResumen {
  cuerpo: string;
  hoja?: string | null;
  pauta?: string | null;
}

export interface ArchivoExterno {
  ruta: string;
  titulo: string;
  cuerpo: string;
}

export interface Estado {
  equipo: string;
  disco: { ruta: string; conectado: boolean } | null;
  proyectos: Proyecto[];
  etiquetas: Etiqueta[];
  notas: NotaResumen[];
  paleta: string[];
}

export interface Entrante {
  id: string;
  titulo: string;
  proyecto: string;
  contexto: string;
  linea: number;
}

export interface Enlaces {
  salientes: { destino: string; id: string | null }[];
  entrantes: Entrante[];
}

export interface Relacionada {
  id: string;
  titulo: string;
  proyecto: string;
  puntuacion: number;
  /** Palabras que más pesan en el parecido. */
  terminos: string[];
}

export interface ParSimilar {
  a: string;
  b: string;
  puntuacion: number;
}

export interface Version {
  /** `AAAAMMDD-HHMMSS.md`: identifica la versión. */
  nombre: string;
  /** Milisegundos desde epoch. */
  fecha: number;
  bytes: number;
}

export interface Tarea {
  id: string;
  titulo: string;
  proyecto: string;
  /** Línea en el cuerpo de la nota (base 0). */
  linea: number;
  hecha: boolean;
  texto: string;
  /** `AAAA-MM-DD` */
  fecha: string | null;
}

export interface Plantilla {
  nombre: string;
  origen: "proyecto" | "equipo" | "disco";
  titulo: string | null;
  etiquetas: string[];
  cuerpo: string;
}

export const api = {
  estado: () => invoke<Estado>("estado"),
  leerNota: (id: string) => invoke<Nota>("leer_nota", { id }),
  buscar: (texto: string) => invoke<string[]>("buscar", { texto }),
  crearNota: (proyecto: string | null, etiquetas: string[]) =>
    invoke<Nota>("crear_nota", { proyecto, etiquetas }),
  guardarNota: (id: string, titulo: string, etiquetas: string[], hoja: string | null, pauta: string | null, cuerpo: string) =>
    invoke<NotaResumen>("guardar_nota", { id, titulo, etiquetas, hoja, pauta, cuerpo }),
  moverNota: (id: string, proyecto: string) => invoke<NotaResumen>("mover_nota", { id, proyecto }),
  borrarNota: (id: string) => invoke<void>("borrar_nota", { id }),
  crearProyecto: (nombre: string, lugar: Lugar) => invoke<string>("crear_proyecto", { nombre, lugar }),
  renombrarProyecto: (id: string, nombre: string) => invoke<void>("renombrar_proyecto", { id, nombre }),
  borrarProyecto: (id: string) => invoke<void>("borrar_proyecto", { id }),
  moverProyecto: (id: string, lugar: Lugar) => invoke<string>("mover_proyecto", { id, lugar }),
  guardarEtiqueta: (original: string | null, nombre: string, color: string) =>
    invoke<string>("guardar_etiqueta", { original, nombre, color }),
  borrarEtiqueta: (nombre: string) => invoke<void>("borrar_etiqueta", { nombre }),
  elegirDisco: () => invoke<string | null>("elegir_disco"),
  olvidarDisco: () => invoke<void>("olvidar_disco"),
  elegirArchivo: () => invoke<ArchivoExterno | null>("elegir_archivo"),
  obtenerTituloUrl: (url: string) => invoke<string>("obtener_titulo_url", { url }),
  abrirCarpeta: (id: string | null, lugar: Lugar | null = null) =>
    invoke<void>("abrir_carpeta", { id, lugar }),
  abrirUrl: (url: string) => invoke<void>("abrir_url", { url }),
  carpetaNota: (id: string) => invoke<string>("carpeta_nota", { id }),
  // relaciones, tareas y plantillas
  enlaces: (id: string) => invoke<Enlaces>("enlaces", { id }),
  relacionadas: (id: string) => invoke<Relacionada[]>("relacionadas", { id }),
  paresSimilares: (umbral: number) => invoke<ParSimilar[]>("pares_similares", { umbral }),
  grafo: (umbral: number) => invoke<{ enlaces: { a: string; b: string }[]; similares: ParSimilar[] }>("grafo", { umbral }),
  contarEnlaces: (titulo: string) => invoke<number>("contar_enlaces", { titulo }),
  renombrarEnlaces: (viejo: string, nuevo: string) => invoke<number>("renombrar_enlaces", { viejo, nuevo }),
  tareas: () => invoke<Tarea[]>("tareas"),
  marcarTarea: (id: string, linea: number, texto: string, hecha: boolean) =>
    invoke<NotaResumen>("marcar_tarea", { id, linea, texto, hecha }),
  plantillas: (proyecto: string | null) => invoke<Plantilla[]>("plantillas", { proyecto }),
  abrirPlantillas: (proyecto: string | null) => invoke<void>("abrir_plantillas", { proyecto }),
  // captura rápida
  guardarTemporal: (bytes: Uint8Array, ext: string) =>
    invoke<string>("guardar_temporal", bytes, { headers: { "x-ext": ext } }),
  adjuntar: (nota: string, nombre: string) => invoke<string>("adjuntar", { nota, nombre }),
  /** Guarda un archivo en `assets/` junto a la nota; devuelve `assets/<nombre>`. */
  adjuntarArchivo: async (nota: string, archivo: File) =>
    invoke<string>("adjuntar_archivo", new Uint8Array(await archivo.arrayBuffer()), {
      headers: { "x-nota": encodeURIComponent(nota), "x-nombre": encodeURIComponent(archivo.name || "archivo") },
    }),
  abrirAdjunto: (nota: string, ruta: string) => invoke<void>("abrir_adjunto", { nota, ruta }),
  instantanea: (id: string) => invoke<boolean>("instantanea", { id }),
  versiones: (id: string) => invoke<Version[]>("versiones", { id }),
  leerVersion: (id: string, nombre: string) => invoke<string>("leer_version", { id, nombre }),
  restaurarVersion: (id: string, nombre: string) => invoke<void>("restaurar_version", { id, nombre }),
  portapapeles: () => invoke<Portapapeles>("portapapeles"),
  pestanaNavegador: () => invoke<{ url: string; titulo: string }>("pestana_navegador"),
  textoSeleccionado: () => invoke<string>("texto_seleccionado"),
  capturarPantalla: () => invoke<{ nombre: string; texto: string } | null>("capturar_pantalla"),
};

export type Portapapeles = { tipo: "texto"; texto: string } | { tipo: "imagen"; nombre: string } | { tipo: "vacio" };

/** Markdown de una imagen adjunta aún sin guardar; al crear la nota pasa a `assets/…`. */
export const imagenAdjunta = (nombre: string, alt = "Imagen") => `![${alt}](adjunto:${nombre})`;

/** Normaliza una etiqueta igual que el backend. */
export function normalizarEtiqueta(e: string): string {
  return e
    .trim()
    .replace(/^#+/, "")
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .join("-")
    .replace(/[,[\]{}"':#]/g, "");
}

/**
 * Crea una nota desde la captura rápida y devuelve el título usado.
 * - `#etiqueta` / `@proyecto` en el título o la primera línea se aplican y se quitan del texto.
 * - Las imágenes `adjunto:<nombre>` se mueven a `assets/` junto a la nota.
 * - Sin título, se usa la primera línea de texto.
 */
export async function crearNotaRapida(
  r: { titulo: string; cuerpo: string; proyecto: string | null },
  etiquetas: string[] = [],
  proyectos: Proyecto[] = [],
): Promise<string> {
  const p = extraerPrefijos(r.titulo, r.cuerpo, proyectos);
  const proyecto = p.proyecto !== undefined ? p.proyecto : r.proyecto;
  const primeraLinea = p.cuerpo
    .split("\n")
    .find((l) => l.trim() && !/^\s*(!\[|<)/.test(l))
    ?.replace(/^[#>\-*+\s]+/, "")
    .replace(/^\[[ xX]\]\s*/, "")
    .slice(0, 60)
    .trim();
  const titulo = p.titulo.trim() || primeraLinea || "Nota rápida";
  const nota = await api.crearNota(proyecto, [...new Set([...etiquetas, ...p.etiquetas])]);
  let cuerpo = p.cuerpo;
  const adjuntos = new Set([...cuerpo.matchAll(/\(adjunto:([^)\s]+)\)/g)].map((m) => m[1]));
  for (const a of adjuntos) {
    try {
      const ruta = await api.adjuntar(nota.id, a);
      cuerpo = cuerpo.split(`(adjunto:${a})`).join(`(${ruta})`);
    } catch {
      /* el adjunto ya no existe: se deja la referencia para no perder texto */
    }
  }
  await api.guardarNota(nota.id, titulo, nota.etiquetas, null, null, cuerpo);
  return titulo;
}
