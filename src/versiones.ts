// Panel «Versiones»: lista de instantáneas de la nota y diferencias lado a lado con el texto
// actual (`@codemirror/merge`). «Restaurar» vuelve a esa versión (el backend guarda antes el
// estado actual, así que también se puede deshacer).
import { MergeView } from "@codemirror/merge";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { markdown } from "@codemirror/lang-markdown";
import { api, type Version } from "./api";
import { dialogo, mensajeError } from "./ui";

const esc = (s: string) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);

/** Separa el front matter: `{ titulo, cuerpo }` (el cuerpo es lo que se compara). */
function partir(texto: string): { titulo: string; cuerpo: string } {
  const m = /^---\r?\n([\s\S]*?)\r?\n(?:---|\.\.\.)\r?\n?([\s\S]*)$/.exec(texto);
  if (!m) return { titulo: "", cuerpo: texto };
  const t = /^title:\s*(.*)$/m.exec(m[1])?.[1]?.trim() ?? "";
  let titulo = t;
  if (t.startsWith('"')) {
    try {
      titulo = JSON.parse(t);
    } catch {
      titulo = t.replace(/^"|"$/g, "");
    }
  } else titulo = t.replace(/^'|'$/g, "");
  return { titulo, cuerpo: m[2].replace(/^\r?\n/, "") };
}

function cuando(ms: number): string {
  const d = new Date(ms);
  const hoy = new Date();
  const ayer = new Date(hoy.getTime() - 864e5);
  const hora = d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  if (d.toDateString() === hoy.toDateString()) return `Hoy, ${hora}`;
  if (d.toDateString() === ayer.toDateString()) return `Ayer, ${hora}`;
  const fecha = d.toLocaleDateString(undefined, { day: "numeric", month: "short", year: d.getFullYear() === hoy.getFullYear() ? undefined : "numeric" });
  return `${fecha}, ${hora}`;
}

/** Abre el panel. `actual` es el texto del editor; resuelve `true` si se restauró una versión. */
export async function abrirVersiones(id: string, titulo: string, actual: string): Promise<boolean> {
  let lista: Version[] = [];
  try {
    lista = await api.versiones(id);
  } catch (e) {
    lista = [];
    console.error(e);
  }
  let restaurada = false;
  const { el, cerrar, alCerrar } = dialogo(`
    <div class="versiones-cab">
      <h3>Versiones</h3>
      <span class="versiones-nota">${esc(titulo || "Sin título")}</span>
    </div>
    ${
      lista.length
        ? `<div class="versiones-cuerpo">
            <ol class="versiones-lista">${lista
              .map((v, i) => `<li><button data-nombre="${esc(v.nombre)}" class="${i === 0 ? "activa" : ""}">${cuando(v.fecha)}</button></li>`)
              .join("")}</ol>
            <div class="versiones-diff">
              <div class="versiones-leyenda"><span>Versión</span><span>Ahora</span></div>
              <div class="versiones-titulos" hidden></div>
              <div class="versiones-merge"></div>
            </div>
          </div>`
        : `<p class="versiones-vacio">Todavía no hay versiones de esta nota. Se guarda una al editar (como mucho cada 10 minutos) y otra al cambiar de nota.</p>`
    }
    <div class="botones">
      <span class="versiones-ayuda">Se conservan todas las de 24 h, una por día durante 30 días y una por semana después.</span>
      <button class="sec" data-r="cerrar">Cerrar</button>
      ${lista.length ? `<button class="pri" data-r="restaurar">Restaurar esta versión</button>` : ""}
    </div>`);
  el.classList.add("dialogo-versiones");

  const ahora = actual;
  const contenedor = el.querySelector<HTMLElement>(".versiones-merge");
  const titulos = el.querySelector<HTMLElement>(".versiones-titulos");
  let vista: MergeView | null = null;
  let elegida = lista[0]?.nombre ?? "";

  async function mostrar(nombre: string) {
    if (!contenedor) return;
    elegida = nombre;
    for (const b of el.querySelectorAll<HTMLButtonElement>(".versiones-lista button")) b.classList.toggle("activa", b.dataset.nombre === nombre);
    let texto = "";
    try {
      texto = await api.leerVersion(id, nombre);
    } catch (e) {
      contenedor.textContent = mensajeError(e);
      return;
    }
    if (elegida !== nombre) return;
    const v = partir(texto);
    if (titulos) {
      titulos.hidden = v.titulo === titulo || !v.titulo;
      titulos.innerHTML = `Título: <del>${esc(v.titulo)}</del> → <ins>${esc(titulo)}</ins>`;
    }
    const ext = [EditorView.editable.of(false), EditorState.readOnly.of(true), EditorView.lineWrapping, markdown()];
    vista?.destroy();
    contenedor.innerHTML = "";
    vista = new MergeView({
      a: { doc: v.cuerpo, extensions: ext },
      b: { doc: ahora, extensions: ext },
      parent: contenedor,
      collapseUnchanged: { margin: 3, minSize: 6 },
      gutter: true,
    });
  }

  el.querySelector(".versiones-lista")?.addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLButtonElement>("button[data-nombre]");
    if (b) mostrar(b.dataset.nombre!);
  });
  el.querySelector<HTMLButtonElement>('[data-r="cerrar"]')!.onclick = cerrar;
  const btnRestaurar = el.querySelector<HTMLButtonElement>('[data-r="restaurar"]');
  if (btnRestaurar) {
    // Confirmación en dos pasos en el mismo botón (las capas no se apilan).
    const ayuda = el.querySelector<HTMLElement>(".versiones-ayuda")!;
    const textoAyuda = ayuda.textContent;
    const desarmar = () => {
      delete btnRestaurar.dataset.armado;
      btnRestaurar.textContent = "Restaurar esta versión";
      ayuda.textContent = textoAyuda;
      ayuda.classList.remove("aviso-restaurar");
    };
    el.querySelector(".versiones-lista")?.addEventListener("click", desarmar);
    btnRestaurar.onclick = async () => {
      const v = lista.find((x) => x.nombre === elegida);
      if (!v) return;
      if (!btnRestaurar.dataset.armado) {
        btnRestaurar.dataset.armado = "1";
        btnRestaurar.textContent = "Sí, restaurar";
        ayuda.textContent = `La nota volverá a como estaba (${cuando(v.fecha)}). El texto actual se guarda antes como otra versión.`;
        ayuda.classList.add("aviso-restaurar");
        return;
      }
      try {
        await api.restaurarVersion(id, v.nombre);
        restaurada = true;
        cerrar();
      } catch (e) {
        ayuda.textContent = mensajeError(e);
      }
    };
  }
  if (lista.length) mostrar(elegida);
  await alCerrar;
  (vista as MergeView | null)?.destroy();
  return restaurada;
}
