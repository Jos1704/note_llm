import "./tema";
import "./estilos.css";
import { listen } from "@tauri-apps/api/event";
import { api, crearNotaRapida, normalizarEtiqueta, type ArchivoExterno, type Estado, type Lugar, type NotaResumen, type Proyecto } from "./api";
import { crearEditor } from "./editor";
import { icono } from "./iconos";
import { aviso, confirmar, editarEtiqueta, esc, menu, mensajeError, opcionesProyecto, panelFlotante, pedirNotaRapida, pedirTexto } from "./ui";
import { renderizar } from "./vista-previa";
import { MODOS, PALETAS, cambiarTema, temaActual } from "./tema";

type Vista = { tipo: "todas" } | { tipo: "proyecto"; id: string } | { tipo: "etiqueta"; nombre: string };
type Modo = "editar" | "dividido" | "vista";

interface Actual {
  id: string;
  proyecto: string;
  titulo: string;
  etiquetas: string[];
  modificado: number;
  hoja: string | null;
  pauta: string | null;
}

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

const el = {
  app: $("app"),
  lateral: $("lateral"),
  listaTitulo: $("lista-titulo"),
  notas: $("notas"),
  filtro: $<HTMLInputElement>("filtro"),
  vacio: $("vacio"),
  editor: $("editor"),
  titulo: $<HTMLInputElement>("titulo"),
  proyecto: $<HTMLSelectElement>("sel-proyecto"),
  meta: $("meta"),
  btnHoja: $<HTMLButtonElement>("btn-hoja"),
  chips: $("chips"),
  nuevaEtiqueta: $<HTMLInputElement>("nueva-etiqueta"),
  sugerencias: $("sugerencias"),
  paneles: $("paneles"),
  vista: $("vista"),
  guardado: $("guardado"),
};

let estado: Estado = { equipo: "", disco: null, proyectos: [], etiquetas: [], notas: [], paleta: [] };
let vista: Vista = { tipo: "todas" };
let filtro = "";
let coincidencias: Set<string> | null = null;
let actual: Actual | null = null;
let sucio = false;
let etiquetasCambiaron = false;
let guardando: Promise<void> | null = null;
let modo: Modo = (leerAjuste("modo") as Modo) ?? "dividido";
let lectura: ArchivoExterno | null = null;
let lateral: "abierta" | "iconos" | "fija" = (leerAjuste("lateral") as "abierta" | "iconos" | "fija") ?? "abierta";

// ------------------------------------------------------------------ utilidades

function leerAjuste(clave: string): string | null {
  try {
    return localStorage.getItem(clave);
  } catch {
    return null;
  }
}

function guardarAjuste(clave: string, valor: string) {
  try {
    localStorage.setItem(clave, valor);
  } catch {
    /* sin almacenamiento: no pasa nada */
  }
}

function debounce<A extends unknown[]>(fn: (...a: A) => void, ms: number) {
  let t = 0;
  return (...a: A) => {
    clearTimeout(t);
    t = window.setTimeout(() => fn(...a), ms);
  };
}

const tiempoRelativo = new Intl.RelativeTimeFormat("es", { numeric: "auto" });
function hace(ms: number): string {
  const seg = (ms - Date.now()) / 1000;
  const pasos: [number, Intl.RelativeTimeFormatUnit][] = [
    [60, "second"],
    [60, "minute"],
    [24, "hour"],
    [7, "day"],
    [4.35, "week"],
    [12, "month"],
    [Infinity, "year"],
  ];
  let valor = seg;
  for (const [tam, unidad] of pasos) {
    if (Math.abs(valor) < tam) {
      return unidad === "second" ? "justo ahora" : tiempoRelativo.format(Math.round(valor), unidad);
    }
    valor /= tam;
  }
  return "";
}

const proyectoPorId = (id: string) => estado.proyectos.find((p) => p.id === id);
const colorDe = (nombre: string) => estado.etiquetas.find((e) => e.nombre === nombre)?.color ?? "#8a9799";
const discoConectado = () => !!estado.disco?.conectado;

function chip(nombre: string, quitar = false): string {
  const x = quitar ? `<button class="chip-x" data-quitar="${esc(nombre)}" title="Quitar">${icono.x}</button>` : "";
  return `<span class="chip" style="--c:${colorDe(nombre)}">${esc(nombre)}${x}</span>`;
}

function fallo(e: unknown) {
  aviso(mensajeError(e), true);
}

// ------------------------------------------------------------------ datos

async function refrescar() {
  try {
    estado = await api.estado();
  } catch (e) {
    return fallo(e);
  }
  // Si la vista apunta a algo que ya no existe (p. ej. se desconectó el disco), volver a «Todas».
  if (vista.tipo === "proyecto" && !proyectoPorId(vista.id)) vista = { tipo: "todas" };
  if (vista.tipo === "etiqueta" && !estado.etiquetas.some((e) => e.nombre === (vista as { nombre: string }).nombre)) {
    vista = { tipo: "todas" };
  }

  if (actual && !sucio) {
    const enDisco = estado.notas.find((n) => n.id === actual!.id);
    if (!enDisco) {
      cerrarNota();
    } else if (enDisco.modificado !== actual.modificado) {
      await abrirNota(actual.id, false); // cambió por fuera (otro editor, otro equipo)
    }
  }
  pintarTodo();
}

function pintarTodo() {
  pintarLateral();
  pintarLista();
  pintarMeta();
}

function aplicarLateral() {
  const estrecha = matchMedia("(max-width: 1080px)").matches;
  const efectiva = lateral === "fija" ? "fija" : estrecha ? "iconos" : lateral;
  el.app.dataset.lateral = efectiva;
}

// ------------------------------------------------------------------ hoja y pauta

const HOJAS = [
  { valor: "auto", nombre: "Automática" },
  { valor: "blanca", nombre: "Papel" },
  { valor: "amarilla", nombre: "Arena" },
  { valor: "negra", nombre: "Pizarra" },
] as const;
const PAUTAS = [
  { valor: "ninguna", nombre: "Lisa" },
  { valor: "rayas", nombre: "Rayas" },
  { valor: "puntos", nombre: "Puntos" },
  { valor: "cuadricula", nombre: "Cuadrícula" },
] as const;

const hojaValida = (h: string | null | undefined) =>
  HOJAS.some((x) => x.valor === h && h !== "auto") ? (h as string) : null;
const pautaValida = (p: string | null | undefined) =>
  PAUTAS.some((x) => x.valor === p && p !== "ninguna") ? (p as string) : null;

/** `null` en hoja = «automática» (sigue el tema); `null` en pauta = lisa. Así no se escribe nada en el front matter. */
function aplicarHoja(hoja: string | null, pauta: string | null) {
  el.editor.dataset.hoja = hoja ?? "auto";
  el.editor.dataset.pauta = pauta ?? "ninguna";
}

function abrirSelectorHoja() {
  if (!actual || lectura) return;
  const panel = document.createElement("div");
  panel.className = "panel-hoja";
  const pintar = () => {
    const h = actual?.hoja ?? "auto";
    const p = actual?.pauta ?? "ninguna";
    panel.innerHTML = `
      <div class="panel-titulo">Hoja</div>
      <div class="opciones-hoja">${HOJAS.map(
        (x) => `<button class="opcion ${x.valor === h ? "activa" : ""}" data-hoja="${x.valor}">
          <span class="muestra-hoja hoja-${x.valor}"></span><span>${x.nombre}</span></button>`,
      ).join("")}</div>
      <div class="panel-titulo">Pauta</div>
      <div class="opciones-hoja">${PAUTAS.map(
        (x) => `<button class="opcion ${x.valor === p ? "activa" : ""}" data-pauta="${x.valor}">
          <span class="muestra-hoja hoja-${h} pauta-${x.valor}"></span><span>${x.nombre}</span></button>`,
      ).join("")}</div>`;
  };
  pintar();
  panel.addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("button");
    if (!b || !actual) return;
    if (b.dataset.hoja) actual.hoja = hojaValida(b.dataset.hoja);
    if (b.dataset.pauta) actual.pauta = pautaValida(b.dataset.pauta);
    aplicarHoja(actual.hoja, actual.pauta);
    pintar();
    marcarCambio();
  });
  el.btnHoja.classList.add("activo");
  panelFlotante(el.btnHoja, panel, () => el.btnHoja.classList.remove("activo"));
}

// ------------------------------------------------------------------ barra lateral

function filaProyecto(p: Proyecto): string {
  const activo = vista.tipo === "proyecto" && vista.id === p.id;
  const ico = p.id.endsWith(":bandeja") ? icono.bandeja : icono.carpeta;
  return `<div class="fila ${activo ? "activa" : ""}" data-accion="ver-proyecto" data-id="${esc(p.id)}">
    ${ico}<span class="nombre">${esc(p.nombre)}</span><span class="cuenta">${p.notas}</span>
    <button class="mas" data-accion="menu-proyecto" data-id="${esc(p.id)}" title="Opciones">${icono.puntos}</button></div>`;
}

function grupoLugar(lugar: Lugar): string {
  const proyectos = estado.proyectos.filter((p) => p.lugar === lugar);
  if (lugar === "disco") {
    const d = estado.disco;
    if (!d) {
      return `<div class="grupo">${icono.disco}<span>Disco</span></div>
        <button class="fila enlace" data-accion="elegir-disco">${icono.mas}<span>Usar un disco externo…</span></button>`;
    }
    const cab = `<div class="grupo" title="${esc(d.ruta)}">${icono.disco}<span>Disco</span>
      ${d.conectado ? "" : '<span class="insignia apagada">no conectado</span>'}
      ${d.conectado ? `<button class="mas visible" data-accion="nuevo-proyecto" data-lugar="disco" title="Nuevo proyecto en el disco">${icono.mas}</button>` : ""}
      <button class="mas visible" data-accion="menu-disco" title="Opciones del disco">${icono.puntos}</button></div>`;
    if (!d.conectado) {
      return `${cab}<p class="nota-lateral">Conecta el disco para ver sus notas.<br><span class="ruta">${esc(d.ruta)}</span></p>`;
    }
    return cab + (proyectos.map(filaProyecto).join("") || '<p class="nota-lateral">Sin proyectos todavía.</p>');
  }
  return `<div class="grupo" title="${esc(estado.equipo)}">${icono.equipo}<span>Este equipo</span>
      <button class="mas visible" data-accion="nuevo-proyecto" data-lugar="equipo" title="Nuevo proyecto en este equipo">${icono.mas}</button>
      <button class="mas visible" data-accion="menu-equipo" title="Opciones">${icono.puntos}</button></div>
    ${proyectos.map(filaProyecto).join("") || '<p class="nota-lateral">Sin proyectos todavía.</p>'}`;
}

function pintarLateral() {
  const etiquetas = estado.etiquetas
    .map((e) => {
      const activa = vista.tipo === "etiqueta" && vista.nombre === e.nombre;
      return `<div class="fila ${activa ? "activa" : ""}" data-accion="ver-etiqueta" data-nombre="${esc(e.nombre)}">
        <span class="punto" style="--c:${e.color}"></span><span class="nombre">${esc(e.nombre)}</span><span class="cuenta">${e.notas}</span>
        <button class="mas" data-accion="editar-etiqueta" data-nombre="${esc(e.nombre)}" title="Editar">${icono.lapiz}</button></div>`;
    })
    .join("");

  el.lateral.innerHTML = `
    <div class="lateral-cab">
      <button class="icono-btn" data-accion="alternar-lateral" title="Contraer o expandir">${icono.colapsar}</button>
      <span class="marca">Notas</span>
      <span class="lateral-acciones">
        <button class="icono-btn" data-accion="abrir-archivo" title="Abrir archivo .md">${icono.archivo}</button>
        <button class="icono-btn ${lateral === "fija" ? "activo" : ""}" data-accion="pin-lateral" title="Fijar barra lateral">${icono.pin}</button>
        <button class="icono-btn" data-accion="tema" title="Tema: paleta y modo">${icono.paleta}</button>
      </span>
    </div>
    <div class="desplazable">
      <button class="fila accion-rapida" data-accion="nota-rapida" title="Nota rápida (Ctrl+Shift+N)">
        ${icono.mas}<span class="nombre">Nota rápida</span><kbd class="atajo">⇧⌃N</kbd>
      </button>
      <div class="fila ${vista.tipo === "todas" ? "activa" : ""}" data-accion="ver-todas">
        ${icono.notas}<span class="nombre">Todas las notas</span><span class="cuenta">${estado.notas.length}</span>
      </div>
      <div class="seccion">Proyectos</div>
      ${grupoLugar("disco")}
      ${grupoLugar("equipo")}
      <div class="seccion">Etiquetas
        <button class="mas visible" data-accion="nueva-etiqueta" title="Nueva etiqueta">${icono.mas}</button></div>
      ${etiquetas || '<p class="nota-lateral">Crea etiquetas aquí o escríbelas en una nota.</p>'}
    </div>`;
}

el.lateral.addEventListener("click", async (ev) => {
  const objetivo = (ev.target as HTMLElement).closest<HTMLElement>("[data-accion]");
  if (!objetivo) return;
  const { accion, id, nombre, lugar } = objetivo.dataset;
  switch (accion) {
    case "ver-todas":
      return cambiarVista({ tipo: "todas" });
    case "ver-proyecto":
      return cambiarVista({ tipo: "proyecto", id: id! });
    case "ver-etiqueta":
      return cambiarVista({ tipo: "etiqueta", nombre: nombre! });
    case "menu-proyecto":
      return menuProyecto(objetivo, id!);
    case "nuevo-proyecto":
      return nuevoProyecto(lugar as Lugar);
    case "elegir-disco":
      return elegirDisco();
    case "menu-disco":
      return menu(objetivo, [
        { texto: "Abrir carpeta", icono: icono.abrirCarpeta, desactivado: !discoConectado(), accion: () => api.abrirCarpeta(null, "disco").catch(fallo) },
        { texto: "Cambiar carpeta del disco…", icono: icono.disco, accion: elegirDisco },
        { texto: "Dejar de usar este disco", icono: icono.x, accion: olvidarDisco },
      ]);
    case "menu-equipo":
      return menu(objetivo, [
        { texto: "Abrir carpeta", icono: icono.abrirCarpeta, accion: () => api.abrirCarpeta(null, "equipo").catch(fallo) },
      ]);
    case "nueva-etiqueta":
      return dialogoEtiqueta(null);
    case "editar-etiqueta":
      return dialogoEtiqueta(nombre!);
    case "alternar-lateral":
      return alternarLateral();
    case "pin-lateral":
      return pinLateral();
    case "abrir-archivo":
      return abrirArchivoExterno();
    case "nota-rapida":
      return notaRapida();
    case "tema":
      return abrirSelectorTema(objetivo);
  }
});

el.lateral.addEventListener("contextmenu", (ev) => {
  const fila = (ev.target as HTMLElement).closest<HTMLElement>('[data-accion="ver-proyecto"]');
  if (!fila) return;
  ev.preventDefault();
  menuProyecto({ x: ev.clientX, y: ev.clientY }, fila.dataset.id!);
});

function cambiarVista(v: Vista) {
  vista = v;
  pintarLateral();
  pintarLista();
}

function menuProyecto(junto: HTMLElement | { x: number; y: number }, id: string) {
  const p = proyectoPorId(id);
  if (!p) return;
  const destino: Lugar = p.lugar === "disco" ? "equipo" : "disco";
  menu(junto, [
    { texto: "Nueva nota aquí", icono: icono.nueva, accion: () => nuevaNota(id) },
    { texto: "Renombrar…", icono: icono.lapiz, accion: () => renombrarProyecto(p) },
    {
      texto: destino === "disco" ? "Mover al disco" : "Mover a este equipo",
      icono: icono.mover,
      desactivado: !discoConectado(),
      accion: () => moverProyecto(p, destino),
    },
    { texto: "Abrir carpeta", icono: icono.abrirCarpeta, accion: () => api.abrirCarpeta(id).catch(fallo) },
    { texto: "Eliminar…", icono: icono.basura, peligro: true, accion: () => borrarProyecto(p) },
  ]);
}

async function nuevoProyecto(lugar: Lugar) {
  const nombre = await pedirTexto({
    titulo: lugar === "disco" ? "Nuevo proyecto en el disco" : "Nuevo proyecto en este equipo",
    placeholder: "Nombre del proyecto",
    boton: "Crear",
  });
  if (!nombre) return;
  try {
    const id = await api.crearProyecto(nombre, lugar);
    vista = { tipo: "proyecto", id };
    await refrescar();
  } catch (e) {
    fallo(e);
  }
}

async function renombrarProyecto(p: Proyecto) {
  const nombre = await pedirTexto({ titulo: "Renombrar proyecto", valor: p.nombre, boton: "Guardar" });
  if (!nombre || nombre === p.nombre) return;
  try {
    await api.renombrarProyecto(p.id, nombre);
    await refrescar();
  } catch (e) {
    fallo(e);
  }
}

async function moverProyecto(p: Proyecto, lugar: Lugar) {
  await guardar();
  try {
    const nuevo = await api.moverProyecto(p.id, lugar);
    if (vista.tipo === "proyecto" && vista.id === p.id) vista = { tipo: "proyecto", id: nuevo };
    if (actual?.proyecto === p.id) {
      const nota = nuevo + actual.id.slice(p.id.length);
      actual = null;
      await refrescar();
      await abrirNota(nota, false);
    } else {
      await refrescar();
    }
    aviso(`«${p.nombre}» ahora está en ${lugar === "disco" ? "el disco" : "este equipo"}`);
  } catch (e) {
    fallo(e);
  }
}

async function borrarProyecto(p: Proyecto) {
  const ok = await confirmar({
    titulo: `¿Eliminar «${esc(p.nombre)}»?`,
    texto: `El proyecto y sus ${p.notas} notas se moverán a la papelera (<code>.papelera</code>) de ${p.lugar === "disco" ? "el disco" : "este equipo"}.`,
    boton: "Eliminar",
    peligro: true,
  });
  if (!ok) return;
  try {
    if (actual?.proyecto === p.id) cerrarNota();
    await api.borrarProyecto(p.id);
    await refrescar();
  } catch (e) {
    fallo(e);
  }
}

async function elegirDisco() {
  try {
    const ruta = await api.elegirDisco();
    if (ruta) {
      aviso(`Notas del disco en ${ruta}`);
      await refrescar();
    }
  } catch (e) {
    fallo(e);
  }
}

async function olvidarDisco() {
  const ok = await confirmar({
    titulo: "¿Dejar de usar este disco?",
    texto: "La app dejará de mostrar las notas del disco en este equipo. <b>No se borra nada</b>: puedes volver a elegir la carpeta cuando quieras.",
    boton: "Dejar de usar",
  });
  if (!ok) return;
  await guardar();
  try {
    await api.olvidarDisco();
    await refrescar();
  } catch (e) {
    fallo(e);
  }
}

async function dialogoEtiqueta(nombre: string | null) {
  const existente = nombre ? estado.etiquetas.find((e) => e.nombre === nombre) : undefined;
  const r = await editarEtiqueta({
    nombre: nombre ?? "",
    color: existente?.color ?? estado.paleta[estado.etiquetas.length % estado.paleta.length] ?? "#89b4fa",
    paleta: estado.paleta,
    existente: !!existente,
  });
  if (!r) return;
  await guardar();
  try {
    if (r.accion === "borrar") {
      const ok = await confirmar({
        titulo: `¿Eliminar la etiqueta «${esc(r.nombre)}»?`,
        texto: `Se quitará de ${existente?.notas ?? 0} notas. Las notas no se borran.`,
        boton: "Eliminar",
        peligro: true,
      });
      if (!ok) return;
      await api.borrarEtiqueta(r.nombre);
      if (vista.tipo === "etiqueta" && vista.nombre === r.nombre) vista = { tipo: "todas" };
    } else {
      const nuevo = await api.guardarEtiqueta(nombre, r.nombre, r.color);
      if (vista.tipo === "etiqueta" && vista.nombre === nombre) vista = { tipo: "etiqueta", nombre: nuevo };
    }
    if (actual) {
      // Las etiquetas de la nota abierta pudieron cambiar en disco.
      sucio = false;
      await abrirNota(actual.id, false);
    }
    await refrescar();
  } catch (e) {
    fallo(e);
  }
}

function abrirSelectorTema(boton: HTMLElement) {
  const panel = document.createElement("div");
  panel.className = "panel-hoja";
  const iconoModo = { auto: icono.equipo, light: icono.sol, dark: icono.luna };
  const pintar = () => {
    const { paleta, modo } = temaActual();
    panel.innerHTML = `
      <div class="panel-titulo">Paleta</div>
      <div class="opciones-hoja tres">${PALETAS.map(
        (p) => `<button class="opcion ${p.valor === paleta ? "activa" : ""}" data-paleta="${p.valor}">
          <span class="muestra-paleta">${p.muestra.map((c) => `<span style="background:${c}"></span>`).join("")}</span>
          <span>${p.nombre}</span></button>`,
      ).join("")}</div>
      <div class="panel-titulo">Modo</div>
      <div class="opciones-hoja tres">${MODOS.map(
        (m) => `<button class="opcion ${m.valor === modo ? "activa" : ""}" data-modo="${m.valor}">
          <span class="muestra-modo">${iconoModo[m.valor]}</span><span>${m.nombre}</span></button>`,
      ).join("")}</div>`;
  };
  pintar();
  panel.addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("button");
    if (!b) return;
    const actualTema = temaActual();
    cambiarTema(
      (b.dataset.paleta as typeof actualTema.paleta) ?? actualTema.paleta,
      (b.dataset.modo as typeof actualTema.modo) ?? actualTema.modo,
    );
    pintar();
  });
  boton.classList.add("activo");
  panelFlotante(boton, panel, () => boton.classList.remove("activo"));
}

function alternarLateral() {
  lateral = lateral === "abierta" || lateral === "fija" ? "iconos" : "abierta";
  guardarAjuste("lateral", lateral);
  aplicarLateral();
  pintarLateral();
}

function pinLateral() {
  lateral = lateral === "fija" ? "abierta" : "fija";
  guardarAjuste("lateral", lateral);
  aplicarLateral();
  pintarLateral();
}

// ------------------------------------------------------------------ lista de notas

function notasVisibles(): NotaResumen[] {
  let ns = estado.notas;
  if (vista.tipo === "proyecto") ns = ns.filter((n) => n.proyecto === (vista as { id: string }).id);
  if (vista.tipo === "etiqueta") ns = ns.filter((n) => n.etiquetas.includes((vista as { nombre: string }).nombre));
  if (filtro) {
    const f = filtro.toLowerCase();
    ns = ns.filter(
      (n) =>
        n.titulo.toLowerCase().includes(f) ||
        n.etiquetas.some((e) => e.includes(f)) ||
        coincidencias?.has(n.id),
    );
  }
  return ns;
}

function pintarLista() {
  const titulo =
    vista.tipo === "todas"
      ? "Todas las notas"
      : vista.tipo === "proyecto"
        ? (proyectoPorId(vista.id)?.nombre ?? "Proyecto")
        : `#${vista.nombre}`;
  el.listaTitulo.textContent = titulo;

  const ns = notasVisibles();
  if (!ns.length) {
    el.notas.innerHTML = `<p class="lista-vacia">${filtro ? "Nada coincide con el filtro." : "No hay notas aquí.<br>Crea una con <kbd>Ctrl</kbd>+<kbd>N</kbd>."}</p>`;
    return;
  }
  const mostrarProyecto = vista.tipo !== "proyecto";
  el.notas.innerHTML = ns
    .map((n) => {
      const p = proyectoPorId(n.proyecto);
      const lugar = mostrarProyecto && p ? `<span class="de">${n.lugar === "disco" ? icono.disco : icono.equipo}${esc(p.nombre)}</span>` : "";
      return `<button class="item ${actual?.id === n.id ? "activa" : ""}" data-id="${esc(n.id)}">
        <span class="item-titulo">${esc(n.titulo)}</span>
        <span class="item-meta"><span class="cuando">${hace(n.modificado)}</span>${lugar}${n.etiquetas.map((e) => chip(e)).join("")}</span>
        <span class="item-extracto">${esc(n.extracto) || "<i>Vacía</i>"}</span>
      </button>`;
    })
    .join("");
}

el.notas.addEventListener("click", (ev) => {
  const item = (ev.target as HTMLElement).closest<HTMLElement>(".item");
  if (!item) return;
  if (item.dataset.id !== actual?.id) abrirNota(item.dataset.id!);
  else el.app.dataset.nota = "";
});

el.notas.addEventListener("contextmenu", (ev) => {
  const item = (ev.target as HTMLElement).closest<HTMLElement>(".item");
  if (!item) return;
  ev.preventDefault();
  const id = item.dataset.id!;
  menu({ x: ev.clientX, y: ev.clientY }, [
    { texto: "Abrir carpeta", icono: icono.abrirCarpeta, accion: () => api.abrirCarpeta(id).catch(fallo) },
    { texto: "Mover a la papelera", icono: icono.basura, peligro: true, accion: () => borrarNota(id) },
  ]);
});

const buscarEnContenido = debounce(async (texto: string) => {
  if (!texto) return;
  try {
    const ids = await api.buscar(texto);
    if (texto === filtro) {
      coincidencias = new Set(ids);
      pintarLista();
    }
  } catch (e) {
    fallo(e);
  }
}, 200);

el.filtro.addEventListener("input", () => {
  filtro = el.filtro.value.trim();
  coincidencias = null;
  pintarLista();
  buscarEnContenido(filtro);
});

// ------------------------------------------------------------------ editor

const pintarVistaPrevia = debounce(() => {
  if (modo !== "editar") el.vista.innerHTML = renderizar(editor.texto());
}, 120);

let cargando = false;
const editor = crearEditor(
  $("cm"),
  () => {
    if (!actual || cargando) return;
    marcarCambio();
    pintarVistaPrevia();
  },
  (fraccion) => {
    if (modo !== "dividido") return;
    const v = el.vista;
    v.scrollTop = fraccion * (v.scrollHeight - v.clientHeight);
  },
);

async function abrirNota(id: string, guardarAntes = true) {
  if (guardarAntes) await guardar();
  let nota;
  try {
    nota = await api.leerNota(id);
  } catch (e) {
    return fallo(e);
  }
  lectura = null;
  actual = { id: nota.id, proyecto: nota.proyecto, titulo: nota.titulo, etiquetas: nota.etiquetas, modificado: nota.modificado, hoja: hojaValida(nota.hoja), pauta: pautaValida(nota.pauta) };
  sucio = false;
  etiquetasCambiaron = false;
  cargando = true;
  editor.poner(nota.cuerpo, false);
  cargando = false;
  el.titulo.value = nota.titulo === "Sin título" && !nota.cuerpo ? "" : nota.titulo;
  el.titulo.readOnly = false;
  el.meta.hidden = false;
  el.btnHoja.hidden = false;
  aplicarHoja(actual.hoja, actual.pauta);
  el.vista.innerHTML = renderizar(nota.cuerpo);
  el.vista.scrollTop = 0;
  el.editor.hidden = false;
  el.vacio.hidden = true;
  el.app.dataset.nota = "";
  estadoGuardado("");
  pintarMeta();
  pintarLista();
}

function cerrarNota() {
  actual = null;
  lectura = null;
  sucio = false;
  el.editor.hidden = true;
  el.vacio.hidden = false;
  delete el.app.dataset.nota;
  pintarVacio();
}

async function abrirArchivoExterno() {
  let archivo;
  try {
    archivo = await api.elegirArchivo();
  } catch (e) {
    return fallo(e);
  }
  if (!archivo) return;
  await guardar();
  actual = null;
  lectura = archivo;
  sucio = false;
  etiquetasCambiaron = false;
  cargando = true;
  editor.poner(archivo.cuerpo, true);
  cargando = false;
  el.titulo.value = archivo.titulo;
  el.titulo.readOnly = true;
  el.meta.hidden = true;
  el.btnHoja.hidden = true;
  aplicarHoja(null, null);
  el.vista.innerHTML = renderizar(archivo.cuerpo);
  el.vista.scrollTop = 0;
  el.editor.hidden = false;
  el.vacio.hidden = true;
  el.app.dataset.nota = "";
  estadoGuardado("Solo lectura");
  pintarLista();
}

function pintarVacio() {
  el.vacio.innerHTML = `${icono.notas}<p>Elige una nota, crea una nueva o abre un archivo .md</p>
    <div class="vacio-acciones">
      <button class="pri" id="vacio-nueva">Nueva nota</button>
      <button class="sec" id="vacio-abrir">Abrir archivo</button>
    </div>
    <p class="atajos"><kbd>Ctrl</kbd>+<kbd>N</kbd> nueva · <kbd>Ctrl</kbd>+<kbd>⇧</kbd>+<kbd>N</kbd> rápida · <kbd>Ctrl</kbd>+<kbd>E</kbd> cambiar vista · <kbd>Ctrl</kbd>+<kbd>K</kbd> filtrar</p>`;
  $("vacio-nueva").onclick = () => nuevaNota();
  $("vacio-abrir").onclick = () => abrirArchivoExterno();
}

function pintarMeta() {
  if (!actual) return;
  const opciones = (lugar: Lugar, etiqueta: string) => {
    const ps = estado.proyectos.filter((p) => p.lugar === lugar);
    if (!ps.length) return "";
    const ops = ps
      .map((p) => `<option value="${esc(p.id)}" ${p.id === actual!.proyecto ? "selected" : ""}>${esc(p.nombre)}</option>`)
      .join("");
    return `<optgroup label="${etiqueta}">${ops}</optgroup>`;
  };
  el.proyecto.innerHTML = opciones("disco", "Disco") + opciones("equipo", "Este equipo");
  el.chips.innerHTML = actual.etiquetas.map((e) => chip(e, true)).join("");
  el.sugerencias.innerHTML = estado.etiquetas
    .filter((e) => !actual!.etiquetas.includes(e.nombre))
    .map((e) => `<option value="${esc(e.nombre)}">`)
    .join("");
}

function estadoGuardado(texto: string, error = false) {
  el.guardado.textContent = texto;
  el.guardado.classList.toggle("error", error);
}

function marcarCambio() {
  if (lectura) return;
  sucio = true;
  estadoGuardado("Sin guardar");
  guardarPronto();
}

const guardarPronto = debounce(() => guardar(), 600);

async function guardar(): Promise<void> {
  if (guardando) {
    await guardando;
  }
  if (!actual || lectura || !sucio) return;
  const nota = actual;
  const id = nota.id;
  const conEtiquetas = etiquetasCambiaron;
  sucio = false;
  etiquetasCambiaron = false;
  estadoGuardado("Guardando…");
  guardando = (async () => {
    try {
      const r = await api.guardarNota(id, nota.titulo.trim() || "Sin título", nota.etiquetas, nota.hoja, nota.pauta, editor.texto());
      nota.id = r.id;
      nota.modificado = r.modificado;
      nota.proyecto = r.proyecto;
      if (conEtiquetas) {
        await refrescar();
      } else {
        estado.notas = [r, ...estado.notas.filter((n) => n.id !== id && n.id !== r.id)];
        pintarLista();
      }
      if (!sucio) estadoGuardado("Guardado");
    } catch (e) {
      sucio = true;
      etiquetasCambiaron ||= conEtiquetas;
      estadoGuardado("No se pudo guardar", true);
      fallo(e);
    } finally {
      guardando = null;
    }
  })();
  await guardando;
}

async function nuevaNota(proyecto: string | null = vista.tipo === "proyecto" ? vista.id : null) {
  await guardar();
  const etiquetas = vista.tipo === "etiqueta" ? [vista.nombre] : [];
  try {
    const n = await api.crearNota(proyecto, etiquetas);
    filtro = "";
    el.filtro.value = "";
    await refrescar();
    await abrirNota(n.id, false);
    el.titulo.focus();
  } catch (e) {
    fallo(e);
  }
}

/** Captura rápida sin salir de lo que estás haciendo: crea la nota y sigue en la nota actual. */
async function notaRapida() {
  let r = await pedirNotaRapida(opcionesProyecto(estado));
  while (r) {
    await guardar();
    try {
      const titulo = await crearNotaRapida(r, vista.tipo === "etiqueta" ? [vista.nombre] : []);
      await refrescar();
      aviso(`Nota «${titulo}» guardada`);
      return;
    } catch (e) {
      fallo(e);
      r = await pedirNotaRapida(opcionesProyecto(estado), r); // no perder lo escrito
    }
  }
}

async function borrarNota(id: string) {
  const n = estado.notas.find((x) => x.id === id);
  const ok = await confirmar({
    titulo: "¿Mover la nota a la papelera?",
    texto: `«${esc(n?.titulo ?? "")}» se moverá a la carpeta <code>.papelera</code>; podrás recuperarla desde ahí.`,
    boton: "Mover a la papelera",
    peligro: true,
  });
  if (!ok) return;
  try {
    if (actual?.id === id) {
      sucio = false;
      cerrarNota();
    }
    await api.borrarNota(id);
    await refrescar();
  } catch (e) {
    fallo(e);
  }
}

el.titulo.addEventListener("input", () => {
  if (!actual || lectura) return;
  actual.titulo = el.titulo.value;
  marcarCambio();
});
el.titulo.addEventListener("keydown", (e) => {
  if (e.key === "Enter" || e.key === "ArrowDown") {
    e.preventDefault();
    editor.view.focus();
  }
});

el.proyecto.addEventListener("change", async () => {
  if (!actual) return;
  await guardar();
  try {
    const r = await api.moverNota(actual.id, el.proyecto.value);
    actual.id = r.id;
    actual.proyecto = r.proyecto;
    actual.modificado = r.modificado;
    await refrescar();
    aviso(`Movida a «${proyectoPorId(r.proyecto)?.nombre ?? ""}»`);
  } catch (e) {
    fallo(e);
    pintarMeta();
  }
});

function agregarEtiqueta(texto: string) {
  if (!actual) return;
  const nuevas = texto.split(",").map(normalizarEtiqueta).filter((e) => e && !actual!.etiquetas.includes(e));
  if (!nuevas.length) return;
  actual.etiquetas = [...actual.etiquetas, ...nuevas].sort();
  etiquetasCambiaron = true;
  pintarMeta();
  marcarCambio();
}

el.nuevaEtiqueta.addEventListener("keydown", (e) => {
  if (e.key === "Enter" || e.key === ",") {
    e.preventDefault();
    agregarEtiqueta(el.nuevaEtiqueta.value);
    el.nuevaEtiqueta.value = "";
  } else if (e.key === "Backspace" && !el.nuevaEtiqueta.value && actual?.etiquetas.length) {
    actual.etiquetas = actual.etiquetas.slice(0, -1);
    etiquetasCambiaron = true;
    pintarMeta();
    marcarCambio();
  }
});
// Elegir una sugerencia del datalist dispara `input` con el valor completo.
el.nuevaEtiqueta.addEventListener("input", (e) => {
  if ((e as InputEvent).inputType === "insertReplacementText" || !(e as InputEvent).inputType) {
    if (estado.etiquetas.some((x) => x.nombre === el.nuevaEtiqueta.value)) {
      agregarEtiqueta(el.nuevaEtiqueta.value);
      el.nuevaEtiqueta.value = "";
    }
  }
});
el.nuevaEtiqueta.addEventListener("blur", () => {
  if (el.nuevaEtiqueta.value.trim()) {
    agregarEtiqueta(el.nuevaEtiqueta.value);
    el.nuevaEtiqueta.value = "";
  }
});

el.chips.addEventListener("click", (e) => {
  const x = (e.target as HTMLElement).closest<HTMLElement>("[data-quitar]");
  if (!x || !actual) return;
  actual.etiquetas = actual.etiquetas.filter((t) => t !== x.dataset.quitar);
  etiquetasCambiaron = true;
  pintarMeta();
  marcarCambio();
});

// Vista previa: casillas de tareas y enlaces.
el.vista.addEventListener("click", (e) => {
  const objetivo = e.target as HTMLElement;
  if (objetivo instanceof HTMLInputElement && objetivo.classList.contains("casilla")) {
    e.preventDefault();
    editor.alternarTarea(Number(objetivo.dataset.linea));
    return;
  }
  const a = objetivo.closest("a");
  if (a) {
    e.preventDefault();
    const href = a.getAttribute("href") ?? "";
    if (/^(https?:|mailto:)/.test(href)) api.abrirUrl(href).catch(fallo);
    else if (href.startsWith("#")) el.vista.querySelector(decodeURIComponent(href))?.scrollIntoView();
  }
});

// ------------------------------------------------------------------ modos y barra

function ponerModo(m: Modo) {
  modo = m;
  guardarAjuste("modo", m);
  el.paneles.dataset.modo = m;
  document.querySelectorAll<HTMLButtonElement>(".modos button").forEach((b) => b.classList.toggle("activo", b.dataset.modo === m));
  if (m !== "editar") el.vista.innerHTML = renderizar(editor.texto());
}

const botonesModo: Record<Modo, string> = { editar: icono.lapiz, dividido: icono.columnas, vista: icono.ojo };
document.querySelectorAll<HTMLButtonElement>(".modos button").forEach((b) => {
  const m = b.dataset.modo as Modo;
  b.innerHTML = botonesModo[m];
  b.onclick = () => ponerModo(m);
});

$("btn-nueva").innerHTML = icono.nueva;
$("btn-nueva").onclick = () => nuevaNota();
$("btn-carpeta").innerHTML = icono.abrirCarpeta;
$("btn-carpeta").onclick = () => actual && api.abrirCarpeta(actual.id).catch(fallo);
$("btn-borrar").innerHTML = icono.basura;
$("btn-borrar").onclick = () => actual && borrarNota(actual.id);
$("filtro-icono").innerHTML = icono.buscar;
el.btnHoja.innerHTML = icono.hoja;
el.btnHoja.onclick = abrirSelectorHoja;
$("btn-volver").innerHTML = icono.atras;
$("btn-volver").onclick = () => {
  guardar();
  delete el.app.dataset.nota;
};

// ------------------------------------------------------------------ atajos y ciclo de vida

document.addEventListener("keydown", (e) => {
  if (!(e.ctrlKey || e.metaKey)) return;
  const k = e.key.toLowerCase();
  if (k === "n" && e.shiftKey) {
    e.preventDefault();
    notaRapida();
  } else if (k === "n") {
    e.preventDefault();
    nuevaNota();
  } else if (k === "s") {
    e.preventDefault();
    guardar();
  } else if (k === "e") {
    e.preventDefault();
    const orden: Modo[] = ["editar", "dividido", "vista"];
    ponerModo(orden[(orden.indexOf(modo) + 1) % orden.length]);
  } else if (k === "k") {
    e.preventDefault();
    el.filtro.focus();
    el.filtro.select();
  }
});

// Notas creadas desde la ventana de nota rápida (barra del sistema).
listen("notas-cambiaron", () => refrescar()).catch(() => {});

// Al volver a la ventana: releer (cambios hechos en otro equipo, disco conectado/desconectado).
window.addEventListener("focus", () => {
  if (!guardando) refrescar();
});
window.addEventListener("blur", () => guardar());

// Mientras el disco recordado no está conectado, comprobar cada pocos segundos.
setInterval(() => {
  if (estado.disco && !estado.disco.conectado && !document.hidden) refrescar();
}, 4000);


matchMedia("(max-width: 1080px)").addEventListener("change", () => {
  aplicarLateral();
  pintarLateral();
});

aplicarLateral();
ponerModo(modo);
pintarVacio();
refrescar();
