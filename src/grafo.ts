// Vista «Grafo»: proyectos como nodos principales y las notas orbitando a su alrededor.
// Aristas: pertenencia (tenue), wikilinks (sólida), etiqueta compartida (punteada, color de la
// etiqueta) y parecido TF‑IDF (fina). Se dibuja en <canvas> con d3-force.
// Con zoom alejado solo se ven los proyectos y las aristas proyecto ↔ proyecto (suma de las
// relaciones entre sus notas).
import { forceCollide, forceLink, forceManyBody, forceSimulation, forceX, forceY, type Simulation, type SimulationLinkDatum, type SimulationNodeDatum } from "d3-force";
import type { Etiqueta, NotaResumen, ParSimilar, Proyecto } from "./api";

export interface DatosGrafo {
  proyectos: Proyecto[];
  notas: NotaResumen[];
  etiquetas: Etiqueta[];
  paleta: string[];
  enlaces: { a: string; b: string }[];
  similares: ParSimilar[];
}

export interface OpcionesGrafo {
  cargar: () => Promise<DatosGrafo>;
  abrir: (id: string) => void;
  cerrar: () => void;
  nombreProyecto: (id: string) => string;
}

type TipoArista = "pertenece" | "enlace" | "etiqueta" | "parecida";

interface Nodo extends SimulationNodeDatum {
  id: string;
  tipo: "proyecto" | "nota";
  nombre: string;
  proyecto: string;
  radio: number;
  color: string;
  nota?: NotaResumen;
}

interface Arista extends SimulationLinkDatum<Nodo> {
  tipo: TipoArista;
  color?: string;
  peso: number;
  source: Nodo;
  target: Nodo;
}

interface AristaProyectos {
  a: Nodo;
  b: Nodo;
  peso: number;
}

const CLAVE_POSICIONES = "grafo-posiciones";
const CLAVE_FILTROS = "grafo-filtros";
const ZOOM_NOTAS = 0.55; // por debajo solo se ven los proyectos
const UMBRAL_PARECIDO = 0.18;

interface Filtros {
  tipos: Record<TipoArista, boolean>;
  lugar: "todos" | "equipo" | "disco";
  fecha: "todas" | "mes" | "semana";
  etiqueta: string;
}

const FILTROS_INICIALES: Filtros = {
  tipos: { pertenece: true, enlace: true, etiqueta: true, parecida: true },
  lugar: "todos",
  fecha: "todas",
  etiqueta: "",
};

function leer<T>(clave: string, defecto: T): T {
  try {
    const v = localStorage.getItem(clave);
    return v ? { ...defecto, ...JSON.parse(v) } : defecto;
  } catch {
    return defecto;
  }
}
function escribir(clave: string, valor: unknown) {
  try {
    localStorage.setItem(clave, JSON.stringify(valor));
  } catch {
    /* sin almacenamiento */
  }
}

const esc = (s: string) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);

/** Color calculado de una variable CSS del tema (con una sonda: `getPropertyValue` no resuelve `var()`). */
function css(sonda: HTMLElement, nombre: string, defecto: string) {
  sonda.style.color = "";
  sonda.style.color = `var(${nombre})`;
  return getComputedStyle(sonda).color || defecto;
}

function conAlfa(color: string, alfa: number) {
  // #rrggbb → rgba; otros formatos se devuelven tal cual con globalAlpha.
  const m = /^#([0-9a-f]{6})$/i.exec(color.trim());
  if (!m) return color;
  const n = parseInt(m[1], 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alfa})`;
}

export function abrirGrafo(contenedor: HTMLElement, op: OpcionesGrafo): { destruir: () => void; recargar: () => void } {
  contenedor.innerHTML = `
    <canvas class="grafo-lienzo"></canvas>
    <div class="grafo-barra">
      <div class="grafo-filtros">
        <button class="grafo-chip" data-tipo="enlace" title="Enlaces [[…]] entre notas"><i class="muestra solida"></i>Enlaces</button>
        <button class="grafo-chip" data-tipo="etiqueta" title="Notas que comparten etiqueta"><i class="muestra punteada"></i>Etiquetas</button>
        <button class="grafo-chip" data-tipo="parecida" title="Notas parecidas por su texto"><i class="muestra fina"></i>Parecidas</button>
        <button class="grafo-chip" data-tipo="pertenece" title="Nota → proyecto"><i class="muestra tenue"></i>Proyecto</button>
        <select class="grafo-sel" data-filtro="lugar" title="Lugar">
          <option value="todos">Todos los lugares</option><option value="equipo">Este equipo</option><option value="disco">Disco</option>
        </select>
        <select class="grafo-sel" data-filtro="fecha" title="Modificadas">
          <option value="todas">Cualquier fecha</option><option value="mes">Último mes</option><option value="semana">Última semana</option>
        </select>
        <select class="grafo-sel" data-filtro="etiqueta" title="Etiqueta"></select>
      </div>
      <input class="grafo-buscar" type="search" placeholder="Buscar y centrar" spellcheck="false" />
      <button class="grafo-cerrar" title="Cerrar (Esc)">✕</button>
    </div>
    <div class="grafo-ficha" hidden></div>
    <span class="grafo-sonda" hidden></span>
    <div class="grafo-ayuda">Rueda: zoom · arrastrar: mover · clic: resaltar · doble clic: abrir</div>`;

  const lienzo = contenedor.querySelector<HTMLCanvasElement>(".grafo-lienzo")!;
  const ctx = lienzo.getContext("2d")!;
  const ficha = contenedor.querySelector<HTMLDivElement>(".grafo-ficha")!;
  const buscar = contenedor.querySelector<HTMLInputElement>(".grafo-buscar")!;
  const selEtiqueta = contenedor.querySelector<HTMLSelectElement>('[data-filtro="etiqueta"]')!;

  let datos: DatosGrafo | null = null;
  let nodos: Nodo[] = [];
  let aristas: Arista[] = [];
  let entreProyectos: AristaProyectos[] = [];
  let vecinos = new Map<string, Set<string>>();
  let sim: Simulation<Nodo, Arista> | null = null;
  let filtros: Filtros = leer(CLAVE_FILTROS, FILTROS_INICIALES);
  filtros.tipos = { ...FILTROS_INICIALES.tipos, ...filtros.tipos };
  const posiciones: Record<string, [number, number]> = leer(CLAVE_POSICIONES, {});
  let t = { x: 0, y: 0, k: 1 };
  let encuadrado = false;
  let seleccion: Nodo | null = null;
  let encima: Nodo | null = null;
  let pendiente = false;
  let vivo = true;

  const colores = {
    texto: "#ddd",
    suave: "#888",
    fondo: "#111",
    borde: "#333",
  };
  const sonda = contenedor.querySelector<HTMLElement>(".grafo-sonda")!;
  function leerColores() {
    colores.texto = css(sonda, "--texto", "#ddd");
    colores.suave = css(sonda, "--suave", "#888");
    colores.fondo = css(sonda, "--bg", "#111");
    colores.borde = css(sonda, "--borde", "#333");
  }

  // ---------------------------------------------------------------- datos → nodos y aristas

  function construir() {
    if (!datos) return;
    const d = datos;
    const ahora = Date.now();
    const limite = filtros.fecha === "semana" ? ahora - 7 * 864e5 : filtros.fecha === "mes" ? ahora - 31 * 864e5 : 0;
    const colorProyecto = new Map<string, string>();
    d.proyectos.forEach((p, i) => colorProyecto.set(p.id, d.paleta[i % Math.max(1, d.paleta.length)] ?? "#89b4fa"));
    const colorEtiqueta = new Map(d.etiquetas.map((e) => [e.nombre, e.color]));

    const notas = d.notas.filter(
      (n) =>
        (filtros.lugar === "todos" || n.lugar === filtros.lugar) &&
        n.modificado >= limite &&
        (!filtros.etiqueta || n.etiquetas.includes(filtros.etiqueta)),
    );
    const conNotas = new Set(notas.map((n) => n.proyecto));
    const proyectos = d.proyectos.filter(
      (p) => (filtros.lugar === "todos" || p.lugar === filtros.lugar) && (conNotas.has(p.id) || (!limite && !filtros.etiqueta)),
    );

    const previos = new Map(nodos.map((n) => [n.id, n]));
    const colocar = (n: Nodo) => {
      const p = previos.get(n.id);
      const g = posiciones[n.id];
      if (p?.x !== undefined) Object.assign(n, { x: p.x, y: p.y });
      else if (g) Object.assign(n, { x: g[0], y: g[1] });
      return n;
    };
    const porId = new Map<string, Nodo>();
    for (const p of proyectos) {
      const cuantas = notas.filter((n) => n.proyecto === p.id).length;
      porId.set(
        p.id,
        colocar({ id: p.id, tipo: "proyecto", nombre: p.nombre, proyecto: p.id, radio: 12 + Math.sqrt(cuantas) * 4, color: colorProyecto.get(p.id)! }),
      );
    }
    for (const n of notas) {
      if (!porId.has(n.proyecto)) continue;
      porId.set(n.id, colocar({ id: n.id, tipo: "nota", nombre: n.titulo || "Sin título", proyecto: n.proyecto, radio: 4.5, color: colorProyecto.get(n.proyecto)!, nota: n }));
    }
    nodos = [...porId.values()];

    aristas = [];
    const poner = (a: string, b: string, tipo: TipoArista, peso: number, color?: string) => {
      const na = porId.get(a);
      const nb = porId.get(b);
      if (na && nb && na !== nb) aristas.push({ source: na, target: nb, tipo, peso, color });
    };
    for (const n of notas) poner(n.id, n.proyecto, "pertenece", 1);
    for (const e of d.enlaces) poner(e.a, e.b, "enlace", 1);
    for (const s of d.similares) if (s.puntuacion >= UMBRAL_PARECIDO) poner(s.a, s.b, "parecida", s.puntuacion);
    // Etiqueta compartida: anillo entre las notas de cada etiqueta (no todas con todas).
    const porEtiqueta = new Map<string, string[]>();
    for (const n of notas) for (const e of n.etiquetas) porEtiqueta.set(e, [...(porEtiqueta.get(e) ?? []), n.id]);
    for (const [e, ids] of porEtiqueta) {
      if (ids.length < 2 || ids.length > 60) continue;
      ids.forEach((id, i) => {
        if (ids.length === 2 && i === 1) return;
        poner(id, ids[(i + 1) % ids.length], "etiqueta", 0.5, colorEtiqueta.get(e));
      });
    }

    // Proyecto ↔ proyecto: suma de las relaciones entre sus notas.
    const suma = new Map<string, AristaProyectos>();
    for (const a of aristas) {
      if (a.tipo === "pertenece" || !filtros.tipos[a.tipo]) continue;
      const pa = porId.get(a.source.proyecto)!;
      const pb = porId.get(a.target.proyecto)!;
      if (pa === pb) continue;
      const clave = pa.id < pb.id ? `${pa.id}|${pb.id}` : `${pb.id}|${pa.id}`;
      const x = suma.get(clave) ?? { a: pa, b: pb, peso: 0 };
      x.peso += a.tipo === "parecida" ? a.peso : a.tipo === "etiqueta" ? 0.5 : 1;
      suma.set(clave, x);
    }
    entreProyectos = [...suma.values()];

    vecinos = new Map();
    const unir = (a: string, b: string) => {
      if (!vecinos.has(a)) vecinos.set(a, new Set());
      vecinos.get(a)!.add(b);
    };
    for (const a of aristas) {
      if (!filtros.tipos[a.tipo]) continue;
      unir(a.source.id, a.target.id);
      unir(a.target.id, a.source.id);
    }
    for (const e of entreProyectos) {
      unir(e.a.id, e.b.id);
      unir(e.b.id, e.a.id);
    }

    if (seleccion && !porId.has(seleccion.id)) seleccion = null;
    simular(nodos.every((n) => n.x !== undefined) ? 0.15 : 1);
  }

  function simular(alfa: number) {
    sim?.stop();
    const activas = aristas.filter((a) => a.tipo === "pertenece" || filtros.tipos[a.tipo]);
    sim = forceSimulation<Nodo, Arista>(nodos)
      .force(
        "aristas",
        forceLink<Nodo, Arista>(activas)
          .id((n) => n.id)
          .distance((a) => (a.tipo === "pertenece" ? 26 + a.target.radio * 2.2 : 70))
          .strength((a) =>
            a.tipo === "pertenece" ? 0.7 : a.source.proyecto === a.target.proyecto ? 0.2 * a.peso : 0.03 * a.peso,
          ),
      )
      .force("carga", forceManyBody<Nodo>().strength((n) => (n.tipo === "proyecto" ? -900 : -28)))
      .force("choque", forceCollide<Nodo>((n) => n.radio + (n.tipo === "proyecto" ? 10 : 2)))
      .force("x", forceX<Nodo>(0).strength(0.03))
      .force("y", forceY<Nodo>(0).strength(0.03))
      .alpha(alfa)
      .on("tick", pintar)
      .on("end", guardarPosiciones);
  }

  function guardarPosiciones() {
    for (const n of nodos) if (n.x !== undefined && n.y !== undefined) posiciones[n.id] = [Math.round(n.x), Math.round(n.y)];
    escribir(CLAVE_POSICIONES, posiciones);
  }

  // ---------------------------------------------------------------- dibujo

  function ajustarTamano() {
    const r = contenedor.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    lienzo.width = Math.round(r.width * dpr);
    lienzo.height = Math.round(r.height * dpr);
    lienzo.style.width = `${r.width}px`;
    lienzo.style.height = `${r.height}px`;
    if (!encuadrado) {
      t = { x: r.width / 2, y: r.height / 2, k: 0.9 };
      encuadrado = true;
    }
    pintar();
  }

  function pintar() {
    if (pendiente || !vivo) return;
    pendiente = true;
    requestAnimationFrame(() => {
      pendiente = false;
      dibujar();
    });
  }

  const conNotas = () => t.k >= ZOOM_NOTAS;
  const visible = (n: Nodo) => n.tipo === "proyecto" || conNotas();
  const resaltado = (n: Nodo) => !seleccion || n === seleccion || !!vecinos.get(seleccion.id)?.has(n.id);

  function dibujar() {
    const dpr = window.devicePixelRatio || 1;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, lienzo.width, lienzo.height);
    ctx.setTransform(dpr * t.k, 0, 0, dpr * t.k, dpr * t.x, dpr * t.y);
    const atenuado = (a: Nodo, b: Nodo) => !!seleccion && !(resaltado(a) && resaltado(b) && (a === seleccion || b === seleccion || a.tipo === "proyecto" && b.tipo === "proyecto"));

    // Proyecto ↔ proyecto
    for (const e of entreProyectos) {
      ctx.globalAlpha = atenuado(e.a, e.b) ? 0.08 : conNotas() ? 0.25 : 0.55;
      ctx.strokeStyle = colores.suave;
      ctx.lineWidth = Math.min(14, 1 + Math.sqrt(e.peso) * 2) / Math.max(t.k, 0.4);
      linea(e.a, e.b);
    }

    if (conNotas()) {
      for (const a of aristas) {
        if (a.tipo !== "pertenece" && !filtros.tipos[a.tipo]) continue;
        if (a.tipo === "pertenece" && !filtros.tipos.pertenece) continue;
        const tenue = atenuado(a.source, a.target);
        ctx.setLineDash([]);
        switch (a.tipo) {
          case "pertenece":
            ctx.strokeStyle = a.source.color;
            ctx.globalAlpha = tenue ? 0.04 : 0.18;
            ctx.lineWidth = 0.8 / t.k;
            break;
          case "enlace":
            ctx.strokeStyle = colores.texto;
            ctx.globalAlpha = tenue ? 0.08 : 0.6;
            ctx.lineWidth = 1.4 / t.k;
            break;
          case "etiqueta":
            ctx.strokeStyle = a.color ?? colores.suave;
            ctx.globalAlpha = tenue ? 0.06 : 0.5;
            ctx.lineWidth = 1 / t.k;
            ctx.setLineDash([3 / t.k, 3 / t.k]);
            break;
          case "parecida":
            ctx.strokeStyle = colores.suave;
            ctx.globalAlpha = tenue ? 0.05 : 0.25 + a.peso * 0.6;
            ctx.lineWidth = 0.6 / t.k;
            break;
        }
        linea(a.source, a.target);
      }
      ctx.setLineDash([]);
    }

    // Nodos: notas primero, proyectos encima.
    for (const fase of ["nota", "proyecto"] as const) {
      for (const n of nodos) {
        if (n.tipo !== fase || !visible(n) || n.x === undefined) continue;
        ctx.globalAlpha = resaltado(n) ? 1 : 0.15;
        ctx.beginPath();
        ctx.arc(n.x, n.y!, n.radio, 0, Math.PI * 2);
        if (n.tipo === "proyecto") {
          ctx.fillStyle = conAlfa(n.color, 0.22);
          ctx.fill();
          ctx.strokeStyle = n.color;
          ctx.lineWidth = 2 / Math.max(t.k, 0.5);
          ctx.stroke();
        } else {
          ctx.fillStyle = n.color;
          ctx.fill();
        }
        if (n === seleccion || n === encima) {
          ctx.strokeStyle = colores.texto;
          ctx.lineWidth = 1.5 / t.k;
          ctx.beginPath();
          ctx.arc(n.x, n.y!, n.radio + 3 / t.k, 0, Math.PI * 2);
          ctx.stroke();
        }
      }
    }

    // Etiquetas de texto (tamaño constante en pantalla).
    ctx.textAlign = "center";
    ctx.textBaseline = "top";
    for (const n of nodos) {
      if (!visible(n) || n.x === undefined) continue;
      const mostrar =
        n.tipo === "proyecto" || n === encima || n === seleccion || t.k > 1.6 || (!!seleccion && resaltado(n) && t.k > 0.9);
      if (!mostrar) continue;
      const grande = n.tipo === "proyecto";
      ctx.globalAlpha = resaltado(n) ? (grande ? 0.95 : 0.85) : 0.15;
      ctx.font = `${grande ? 600 : 400} ${(grande ? 13 : 11.5) / t.k}px "Inter Variable", system-ui, sans-serif`;
      const texto = n.nombre.length > 40 ? n.nombre.slice(0, 39) + "…" : n.nombre;
      const y = n.y! + n.radio + 4 / t.k;
      ctx.lineWidth = 3 / t.k;
      ctx.strokeStyle = colores.fondo;
      ctx.strokeText(texto, n.x, y);
      ctx.fillStyle = grande ? colores.texto : colores.suave;
      ctx.fillText(texto, n.x, y);
    }
    ctx.globalAlpha = 1;
  }

  function linea(a: Nodo, b: Nodo) {
    if (a.x === undefined || b.x === undefined) return;
    ctx.beginPath();
    ctx.moveTo(a.x, a.y!);
    ctx.lineTo(b.x, b.y!);
    ctx.stroke();
  }

  // ---------------------------------------------------------------- interacción

  const aMundo = (ev: MouseEvent) => {
    const r = lienzo.getBoundingClientRect();
    return { x: (ev.clientX - r.left - t.x) / t.k, y: (ev.clientY - r.top - t.y) / t.k };
  };

  function nodoEn(ev: MouseEvent): Nodo | null {
    const p = aMundo(ev);
    let mejor: Nodo | null = null;
    for (const n of nodos) {
      if (!visible(n) || n.x === undefined) continue;
      const margen = Math.max(n.radio, 6 / t.k);
      if ((n.x - p.x) ** 2 + (n.y! - p.y) ** 2 <= margen ** 2) {
        if (!mejor || n.tipo === "proyecto" || mejor.tipo === "nota") mejor = n;
      }
    }
    return mejor;
  }

  let arrastre: { nodo: Nodo | null; x: number; y: number; tx: number; ty: number; movido: boolean } | null = null;

  lienzo.addEventListener("pointerdown", (ev) => {
    lienzo.setPointerCapture(ev.pointerId);
    arrastre = { nodo: nodoEn(ev), x: ev.clientX, y: ev.clientY, tx: t.x, ty: t.y, movido: false };
  });

  lienzo.addEventListener("pointermove", (ev) => {
    if (!arrastre) {
      const n = nodoEn(ev);
      if (n !== encima) {
        encima = n;
        lienzo.style.cursor = n ? "pointer" : "grab";
        pintar();
      }
      return;
    }
    const dx = ev.clientX - arrastre.x;
    const dy = ev.clientY - arrastre.y;
    if (!arrastre.movido && Math.hypot(dx, dy) < 4) return;
    arrastre.movido = true;
    lienzo.style.cursor = "grabbing";
    if (arrastre.nodo) {
      const p = aMundo(ev);
      arrastre.nodo.fx = p.x;
      arrastre.nodo.fy = p.y;
      sim?.alphaTarget(0.2).restart();
    } else {
      t.x = arrastre.tx + dx;
      t.y = arrastre.ty + dy;
      pintar();
    }
  });

  lienzo.addEventListener("pointerup", (ev) => {
    const a = arrastre;
    arrastre = null;
    lienzo.style.cursor = encima ? "pointer" : "grab";
    if (!a) return;
    if (a.nodo && a.movido) {
      a.nodo.fx = null;
      a.nodo.fy = null;
      sim?.alphaTarget(0);
      return;
    }
    if (a.movido) return;
    seleccionar(nodoEn(ev), ev);
  });

  lienzo.addEventListener("dblclick", (ev) => {
    const n = nodoEn(ev);
    if (n?.tipo === "nota") op.abrir(n.id);
    else if (n) centrar(n, Math.max(t.k, 1.4));
  });

  lienzo.addEventListener(
    "wheel",
    (ev) => {
      ev.preventDefault();
      const r = lienzo.getBoundingClientRect();
      const mx = ev.clientX - r.left;
      const my = ev.clientY - r.top;
      // Pellizco del trackpad (ctrlKey) o rueda: zoom alrededor del cursor. Desplazamiento con dos dedos: mover.
      if (ev.ctrlKey || Math.abs(ev.deltaY) > 40 && ev.deltaMode === 0 && !ev.deltaX) {
        const k = Math.min(4, Math.max(0.12, t.k * Math.exp(-ev.deltaY * (ev.ctrlKey ? 0.01 : 0.0015))));
        t.x = mx - ((mx - t.x) * k) / t.k;
        t.y = my - ((my - t.y) * k) / t.k;
        t.k = k;
      } else {
        t.x -= ev.deltaX;
        t.y -= ev.deltaY;
      }
      pintar();
    },
    { passive: false },
  );

  function seleccionar(n: Nodo | null, ev?: MouseEvent) {
    seleccion = n;
    pintar();
    if (!n || n.tipo === "proyecto") {
      ficha.hidden = true;
      if (n && !conNotas()) centrar(n, 1);
      return;
    }
    const nota = n.nota!;
    const fecha = new Date(nota.modificado).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
    ficha.innerHTML = `
      <div class="grafo-ficha-titulo">${esc(n.nombre)}</div>
      <div class="grafo-ficha-meta"><i style="background:${n.color}"></i>${esc(op.nombreProyecto(n.proyecto))} · ${fecha}</div>
      ${nota.extracto ? `<p>${esc(nota.extracto)}</p>` : ""}
      ${nota.etiquetas.length ? `<div class="grafo-ficha-etiquetas">${nota.etiquetas.map((e) => `<span>#${esc(e)}</span>`).join("")}</div>` : ""}
      <button class="grafo-ficha-abrir" data-id="${esc(n.id)}">Abrir nota</button>`;
    ficha.hidden = false;
    if (ev) {
      const r = contenedor.getBoundingClientRect();
      const x = Math.min(ev.clientX - r.left + 14, r.width - 300);
      const y = Math.min(ev.clientY - r.top + 14, r.height - ficha.offsetHeight - 12);
      ficha.style.left = `${Math.max(12, x)}px`;
      ficha.style.top = `${Math.max(60, y)}px`;
    }
  }

  ficha.addEventListener("click", (ev) => {
    const b = (ev.target as HTMLElement).closest<HTMLElement>(".grafo-ficha-abrir");
    if (b) op.abrir(b.dataset.id!);
  });

  function centrar(n: Nodo, k = t.k) {
    if (n.x === undefined) return;
    const r = lienzo.getBoundingClientRect();
    const desde = { ...t };
    const hasta = { k, x: r.width / 2 - n.x * k, y: r.height / 2 - n.y! * k };
    const inicio = performance.now();
    const paso = (ahora: number) => {
      const p = Math.min(1, (ahora - inicio) / 350);
      const e = 1 - (1 - p) ** 3;
      t = { x: desde.x + (hasta.x - desde.x) * e, y: desde.y + (hasta.y - desde.y) * e, k: desde.k + (hasta.k - desde.k) * e };
      dibujar();
      if (p < 1 && vivo) requestAnimationFrame(paso);
    };
    requestAnimationFrame(paso);
  }

  buscar.addEventListener("keydown", (ev) => {
    if (ev.key !== "Enter") return;
    const q = buscar.value.trim().toLowerCase();
    if (!q) return;
    const n =
      nodos.find((x) => x.nombre.toLowerCase() === q) ??
      nodos.find((x) => x.tipo === "proyecto" && x.nombre.toLowerCase().includes(q)) ??
      nodos.find((x) => x.nombre.toLowerCase().includes(q));
    if (!n) {
      buscar.classList.add("sin-resultado");
      setTimeout(() => buscar.classList.remove("sin-resultado"), 600);
      return;
    }
    seleccionar(n);
    centrar(n, n.tipo === "nota" ? Math.max(t.k, 1.6) : Math.max(t.k, 0.9));
  });

  // ---------------------------------------------------------------- filtros

  function pintarFiltros() {
    for (const b of contenedor.querySelectorAll<HTMLButtonElement>(".grafo-chip")) {
      b.classList.toggle("activo", filtros.tipos[b.dataset.tipo as TipoArista]);
    }
    for (const s of contenedor.querySelectorAll<HTMLSelectElement>(".grafo-sel")) {
      s.value = filtros[s.dataset.filtro as "lugar" | "fecha" | "etiqueta"];
    }
  }

  contenedor.querySelector(".grafo-filtros")!.addEventListener("click", (ev) => {
    const b = (ev.target as HTMLElement).closest<HTMLButtonElement>(".grafo-chip");
    if (!b) return;
    const tipo = b.dataset.tipo as TipoArista;
    filtros.tipos[tipo] = !filtros.tipos[tipo];
    escribir(CLAVE_FILTROS, filtros);
    pintarFiltros();
    construir();
  });
  contenedor.querySelector(".grafo-filtros")!.addEventListener("change", (ev) => {
    const s = ev.target as HTMLSelectElement;
    if (!s.dataset.filtro) return;
    (filtros as unknown as Record<string, string>)[s.dataset.filtro] = s.value;
    escribir(CLAVE_FILTROS, filtros);
    construir();
  });

  contenedor.querySelector(".grafo-cerrar")!.addEventListener("click", () => op.cerrar());
  const teclas = (ev: KeyboardEvent) => {
    if (ev.key !== "Escape") return;
    if (seleccion) seleccionar(null);
    else op.cerrar();
  };
  window.addEventListener("keydown", teclas);
  const observador = new ResizeObserver(ajustarTamano);
  observador.observe(contenedor);
  const tema = new MutationObserver(() => {
    leerColores();
    pintar();
  });
  tema.observe(document.documentElement, { attributes: true });

  async function recargar() {
    try {
      datos = await op.cargar();
    } catch {
      return;
    }
    if (!vivo || !datos) return;
    selEtiqueta.innerHTML =
      `<option value="">Todas las etiquetas</option>` +
      datos.etiquetas.map((e) => `<option value="${esc(e.nombre)}">#${esc(e.nombre)}</option>`).join("");
    if (filtros.etiqueta && !datos.etiquetas.some((e) => e.nombre === filtros.etiqueta)) filtros.etiqueta = "";
    pintarFiltros();
    construir();
  }

  leerColores();
  lienzo.style.cursor = "grab";
  pintarFiltros();
  recargar();

  return {
    recargar,
    destruir() {
      vivo = false;
      sim?.stop();
      guardarPosiciones();
      observador.disconnect();
      tema.disconnect();
      window.removeEventListener("keydown", teclas);
      contenedor.innerHTML = "";
    },
  };
}
