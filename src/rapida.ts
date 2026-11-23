// Ventana de nota rápida: se abre con `md-notes --rapida` (p. ej. desde la barra del sistema).
// Muestra el mismo modal que la app y se cierra al guardar o cancelar.
import "./tema"; // fuentes + paleta y modo guardados (los mismos que la ventana principal)
import "./estilos.css";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { api, crearNotaRapida, type Estado } from "./api";
import { aviso, mensajeError, opcionesProyecto, pedirNotaRapida, type NotaRapida } from "./ui";

const cerrar = () => invoke("cerrar_rapida").catch(() => window.close());

async function iniciar() {
  let estado: Estado | null = null;
  try {
    estado = await api.estado();
  } catch (e) {
    aviso(mensajeError(e), true);
  }
  const opciones = estado ? opcionesProyecto(estado) : `<option value="">Bandeja</option>`;

  let inicial: NotaRapida | undefined;
  for (;;) {
    const r = await pedirNotaRapida(opciones, inicial);
    if (!r) return cerrar();
    try {
      await crearNotaRapida(r);
      await emit("notas-cambiaron").catch(() => {});
      return cerrar();
    } catch (e) {
      aviso(mensajeError(e), true);
      inicial = r; // se vuelve a mostrar con lo escrito
    }
  }
}

iniciar();
