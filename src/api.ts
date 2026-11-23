import { invoke } from "@tauri-apps/api/core";

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
};

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

/** Crea una nota desde la captura rápida. Sin título, usa la primera línea del texto. Devuelve el título usado. */
export async function crearNotaRapida(
  r: { titulo: string; cuerpo: string; proyecto: string | null },
  etiquetas: string[] = [],
): Promise<string> {
  const primeraLinea = r.cuerpo
    .split("\n")
    .find((l) => l.trim())
    ?.replace(/^[#>\-*+\s]+/, "")
    .replace(/^\[[ xX]\]\s*/, "")
    .slice(0, 60)
    .trim();
  const titulo = r.titulo || primeraLinea || "Nota rápida";
  const nota = await api.crearNota(r.proyecto, etiquetas);
  await api.guardarNota(nota.id, titulo, nota.etiquetas, null, null, r.cuerpo);
  return titulo;
}
