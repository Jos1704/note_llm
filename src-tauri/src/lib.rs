mod adjuntos;
mod biblioteca;
mod captura;
mod frontmatter;
mod historial;
pub mod indice;
pub mod mcp;
mod plantillas;
pub mod rapida;
mod relaciones;
#[cfg(target_os = "macos")]
mod notch;
mod store;

use std::sync::Mutex;

use biblioteca::{Biblioteca, Estado};
use serde::Serialize;
use store::{Lugar, Nota, NotaResumen, primer_titulo};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_dialog::DialogExt;

type Res<T> = Result<T, String>;
type Bib<'a> = State<'a, Mutex<Biblioteca>>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArchivoExterno {
    ruta: String,
    titulo: String,
    cuerpo: String,
}

fn con<T>(b: &Bib, f: impl FnOnce(&mut Biblioteca) -> anyhow::Result<T>) -> Res<T> {
    let mut b = b.lock().map_err(|e| e.to_string())?;
    f(&mut b).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
fn estado(b: Bib) -> Res<Estado> {
    con(&b, |b| b.estado())
}

#[tauri::command]
fn leer_nota(b: Bib, id: String) -> Res<Nota> {
    con(&b, |b| b.leer_nota(&id))
}

#[tauri::command]
fn buscar(b: Bib, texto: String) -> Res<Vec<String>> {
    con(&b, |b| b.buscar(&texto))
}

#[tauri::command]
fn crear_nota(b: Bib, proyecto: Option<String>, etiquetas: Vec<String>) -> Res<Nota> {
    con(&b, |b| b.crear_nota(proyecto.as_deref(), &etiquetas))
}

#[tauri::command]
fn guardar_nota(b: Bib, id: String, titulo: String, etiquetas: Vec<String>, hoja: Option<String>, pauta: Option<String>, cuerpo: String) -> Res<NotaResumen> {
    con(&b, |b| b.guardar_nota(&id, &titulo, &etiquetas, hoja.as_deref(), pauta.as_deref(), &cuerpo))
}

#[tauri::command]
fn mover_nota(b: Bib, id: String, proyecto: String) -> Res<NotaResumen> {
    con(&b, |b| b.mover_nota(&id, &proyecto))
}

#[tauri::command]
fn borrar_nota(b: Bib, id: String) -> Res<()> {
    con(&b, |b| b.borrar_nota(&id))
}

#[tauri::command]
fn crear_proyecto(b: Bib, nombre: String, lugar: Lugar) -> Res<String> {
    con(&b, |b| b.crear_proyecto(&nombre, lugar))
}

#[tauri::command]
fn renombrar_proyecto(b: Bib, id: String, nombre: String) -> Res<()> {
    con(&b, |b| b.renombrar_proyecto(&id, &nombre))
}

#[tauri::command]
fn borrar_proyecto(b: Bib, id: String) -> Res<()> {
    con(&b, |b| b.borrar_proyecto(&id))
}

#[tauri::command]
fn mover_proyecto(b: Bib, id: String, lugar: Lugar) -> Res<String> {
    con(&b, |b| b.mover_proyecto(&id, lugar))
}

#[tauri::command]
fn guardar_etiqueta(b: Bib, original: Option<String>, nombre: String, color: String) -> Res<String> {
    con(&b, |b| b.guardar_etiqueta(original.as_deref(), &nombre, &color))
}

#[tauri::command]
fn borrar_etiqueta(b: Bib, nombre: String) -> Res<()> {
    con(&b, |b| b.borrar_etiqueta(&nombre))
}

type Ind<'a> = State<'a, Mutex<indice::Indice>>;

/// Pone al día el índice de similitud (solo relee lo que cambió) y ejecuta `f` sobre él.
fn con_indice<T>(b: &Bib, ind: &Ind, f: impl FnOnce(&mut indice::Indice) -> T) -> Res<T> {
    let entradas = con(b, |b| b.entradas_indice())?;
    let mut ind = ind.lock().map_err(|e| e.to_string())?;
    ind.actualizar(&entradas);
    Ok(f(&mut ind))
}

#[tauri::command]
fn relacionadas(b: Bib, ind: Ind, id: String) -> Res<Vec<indice::Relacionada>> {
    con_indice(&b, &ind, |i| i.relacionadas(&id, 6))
}

#[derive(Serialize)]
struct Grafo {
    enlaces: Vec<relaciones::Arista>,
    similares: Vec<indice::Par>,
}

/// Aristas del grafo: wikilinks resueltos y pares parecidos (TF‑IDF ≥ `umbral`).
#[tauri::command]
fn grafo(b: Bib, ind: Ind, umbral: f32) -> Res<Grafo> {
    let enlaces = con(&b, |b| b.aristas())?;
    let similares = con_indice(&b, &ind, |i| i.pares(umbral, 4))?;
    Ok(Grafo { enlaces, similares })
}

#[tauri::command]
fn pares_similares(b: Bib, ind: Ind, umbral: f32) -> Res<Vec<indice::Par>> {
    con_indice(&b, &ind, |i| i.pares(umbral, 5))
}

#[tauri::command]
fn enlaces(b: Bib, id: String) -> Res<relaciones::Enlaces> {
    con(&b, |b| b.enlaces(&id))
}

#[tauri::command]
fn contar_enlaces(b: Bib, titulo: String) -> Res<usize> {
    con(&b, |b| b.contar_enlaces(&titulo))
}

#[tauri::command]
fn renombrar_enlaces(b: Bib, viejo: String, nuevo: String) -> Res<usize> {
    con(&b, |b| b.renombrar_enlaces(&viejo, &nuevo))
}

#[tauri::command]
fn tareas(b: Bib) -> Res<Vec<relaciones::Tarea>> {
    con(&b, |b| b.tareas())
}

#[tauri::command]
fn marcar_tarea(b: Bib, id: String, linea: usize, texto: String, hecha: bool) -> Res<NotaResumen> {
    con(&b, |b| b.marcar_tarea(&id, linea, &texto, hecha))
}

#[tauri::command]
fn plantillas(b: Bib, proyecto: Option<String>) -> Res<Vec<plantillas::Plantilla>> {
    con(&b, |b| b.plantillas(proyecto.as_deref()))
}

#[tauri::command]
fn abrir_plantillas(b: Bib, proyecto: Option<String>) -> Res<()> {
    let dir = con(&b, |b| b.carpeta_plantillas(proyecto.as_deref()))?;
    tauri_plugin_opener::open_path(dir, None::<&str>).map_err(|e| e.to_string())
}

/// Abre el selector de carpetas y usa la elegida como «Disco». `None` si se canceló.
#[tauri::command]
async fn elegir_disco(app: AppHandle, b: Bib<'_>) -> Res<Option<String>> {
    let Some(carpeta) = app.dialog().file().set_title("Carpeta del disco para tus notas").blocking_pick_folder()
    else {
        return Ok(None);
    };
    let ruta = carpeta.into_path().map_err(|e| e.to_string())?;
    let r = con(&b, |b| b.elegir_disco(&ruta)).map(Some);
    permitir_recursos(&app, &b);
    r
}

/// Abre un `.md` del equipo en modo solo lectura (no se guarda ni se mueve a un proyecto).
#[tauri::command]
async fn elegir_archivo(app: AppHandle) -> Res<Option<ArchivoExterno>> {
    let Some(archivo) = app
        .dialog()
        .file()
        .set_title("Abrir archivo Markdown")
        .add_filter("Markdown", &["md", "markdown"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let ruta = archivo.into_path().map_err(|e| e.to_string())?;
    let texto = std::fs::read_to_string(&ruta).map_err(|e| format!("leyendo {}: {e}", ruta.display()))?;
    let doc = frontmatter::parsear(&texto);
    let titulo = doc
        .titulo
        .filter(|t| !t.trim().is_empty())
        .or_else(|| primer_titulo(&doc.cuerpo))
        .unwrap_or_else(|| ruta.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
    Ok(Some(ArchivoExterno {
        ruta: ruta.display().to_string(),
        titulo,
        cuerpo: doc.cuerpo,
    }))
}

#[tauri::command]
fn olvidar_disco(b: Bib) -> Res<()> {
    con(&b, |b| b.olvidar_disco())
}

/// Abre en el gestor de archivos la carpeta de una nota o proyecto, o la raíz de un lugar.
#[tauri::command]
fn abrir_carpeta(b: Bib, id: Option<String>, lugar: Option<Lugar>) -> Res<()> {
    let ruta = con(&b, |b| b.ruta(id.as_deref(), lugar.unwrap_or(Lugar::Equipo)))?;
    let carpeta = if ruta.is_file() { ruta.parent().map(|p| p.to_path_buf()).unwrap_or(ruta) } else { ruta };
    tauri_plugin_opener::open_path(carpeta, None::<&str>).map_err(|e| e.to_string())
}

/// Decodifica `%XX` (cabeceras enviadas con `encodeURIComponent`).
fn decodificar_url(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..(i + 3).min(b.len())]).unwrap_or(""), 16)
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

#[tauri::command]
fn instantanea(b: Bib, id: String) -> Res<bool> {
    con(&b, |b| b.instantanea(&id))
}

#[tauri::command]
fn versiones(b: Bib, id: String) -> Res<Vec<historial::Version>> {
    con(&b, |b| b.versiones(&id))
}

#[tauri::command]
fn leer_version(b: Bib, id: String, nombre: String) -> Res<String> {
    con(&b, |b| b.leer_version(&id, &nombre))
}

#[tauri::command]
fn restaurar_version(b: Bib, id: String, nombre: String) -> Res<()> {
    con(&b, |b| b.restaurar_version(&id, &nombre))
}

/// Archivo pegado o arrastrado en el editor: se guarda en `assets/` junto a la nota.
/// Cuerpo: bytes crudos. Cabeceras `x-nota` y `x-nombre` (con `encodeURIComponent`).
#[tauri::command]
fn adjuntar_archivo(b: Bib, request: tauri::ipc::Request) -> Res<String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("se esperaban bytes".into());
    };
    let cabecera = |n: &str| request.headers().get(n).and_then(|v| v.to_str().ok()).map(decodificar_url);
    let nota = cabecera("x-nota").ok_or("falta la nota")?;
    let nombre = cabecera("x-nombre").unwrap_or_else(|| "archivo".into());
    con(&b, |b| b.adjuntar_bytes(&nota, &nombre, bytes))
}

/// Abre con la app del sistema un archivo enlazado con ruta relativa desde una nota
/// (solo dentro de la carpeta de la nota).
#[tauri::command]
fn abrir_adjunto(b: Bib, nota: String, ruta: String) -> Res<()> {
    let archivo = con(&b, |b| b.ruta(Some(&nota), Lugar::Equipo))?;
    let carpeta = archivo.parent().ok_or("nota sin carpeta")?.canonicalize().map_err(|e| e.to_string())?;
    let destino = carpeta.join(decodificar_url(&ruta)).canonicalize().map_err(|_| format!("no existe: {ruta}"))?;
    if !destino.starts_with(&carpeta) {
        return Err("ruta fuera de la carpeta de la nota".into());
    }
    tauri_plugin_opener::open_path(destino, None::<&str>).map_err(|e| e.to_string())
}

/// Abre un enlace del contenido en el navegador del sistema.
#[tauri::command]
fn abrir_url(url: String) -> Res<()> {
    let permitido = ["http://", "https://", "mailto:"].iter().any(|p| url.starts_with(p));
    if !permitido {
        return Err(format!("enlace no permitido: {url}"));
    }
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())
}

/// Obtiene el título (`<title>`) de una URL, para pegar enlaces con texto bonito.
/// Devuelve `""` ante cualquier error; la interfaz cae a la URL o al dominio.
#[tauri::command]
fn obtener_titulo_url(url: String) -> Res<String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("enlace no permitido".to_string());
    }
    let cuerpo = ureq::get(&url)
        .set("User-Agent", "md-notes/0.1")
        .timeout(std::time::Duration::from_secs(8))
        .call()
        .ok()
        .and_then(|r| r.into_string().ok())
        .unwrap_or_default();
    Ok(titulo_de_html(&cuerpo))
}

/// Copia el tema de la app a `tema.json` para el panel de Quickshell (Linux).
#[tauri::command]
fn guardar_tema(b: Bib, paleta: String, modo: String) -> Res<()> {
    con(&b, |b| b.guardar_tema(&paleta, &modo))
}

/// Cierra la ventana de nota rápida. Si la app se abrió solo para ella, el proceso termina.
#[tauri::command]
fn cerrar_rapida(window: WebviewWindow) -> Res<()> {
    window.close().map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ ventanas

/// Argumento con el que la barra del sistema pide el modal de nota rápida: `md-notes --rapida`.
const ARG_RAPIDA: &str = "--rapida";
/// `md-notes --capturar`: lo mismo que el atajo global (panel del notch con foco en macOS).
const ARG_CAPTURAR: &str = "--capturar";
/// Atajo global por defecto (se cambia con `"atajo"` en `ajustes.json`).
const ATAJO_POR_DEFECTO: &str = "alt+space";

/// Atajo global / `--capturar`: en macOS, el panel del notch; en otros sistemas, la ventana rápida.
fn capturar(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    if app.get_webview_window("notch").is_some() {
        return notch::atajo(app);
    }
    if let Err(e) = abrir_rapida(app) {
        eprintln!("no se pudo abrir la nota rápida: {e}");
    }
}

fn registrar_atajo(app: &AppHandle) {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
    let texto = app
        .state::<Mutex<Biblioteca>>()
        .lock()
        .ok()
        .and_then(|b| b.atajo().map(str::to_string))
        .unwrap_or_else(|| ATAJO_POR_DEFECTO.to_string());
    let atajo: Shortcut = match texto.parse() {
        Ok(a) => a,
        Err(e) => return eprintln!("atajo no válido «{texto}»: {e}"),
    };
    let r = app.global_shortcut().on_shortcut(atajo, |app, _atajo, evento| {
        if evento.state() == ShortcutState::Pressed {
            capturar(app);
        }
    });
    if let Err(e) = r {
        // En Wayland no hay atajos globales: se configuran en el compositor (`md-notes --capturar`).
        eprintln!("no se pudo registrar el atajo «{texto}»: {e}");
    }
}

/// Deja que la vista previa cargue imágenes de las carpetas de notas (protocolo `asset:`).
fn permitir_recursos(app: &AppHandle, b: &Bib) {
    let raices = b.lock().map(|b| b.raices()).unwrap_or_default();
    let alcance = app.asset_protocol_scope();
    for r in raices {
        let _ = alcance.allow_directory(&r, true);
    }
}

fn pide_rapida(args: &[String]) -> bool {
    args.iter().any(|a| a == ARG_RAPIDA)
}

fn abrir_principal(app: &AppHandle) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    segundo_plano::salir(app);
    if let Some(w) = app.get_webview_window("main") {
        w.unminimize()?;
        w.show()?;
        return w.set_focus();
    }
    let config = app.config().app.windows.iter().find(|w| w.label == "main").cloned().unwrap_or_default();
    WebviewWindowBuilder::from_config(app, &config)?.build()?;
    Ok(())
}

/// macOS: ⌘Q y el botón de cerrar solo ocultan la ventana; la app sigue en segundo plano (sin
/// icono en el Dock) para que el notch y el atajo funcionen. Para salir del todo: icono de la
/// barra de menús → «Salir de Notas».
#[cfg(target_os = "macos")]
mod segundo_plano {
    use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
    use tauri::tray::TrayIconBuilder;
    use tauri::{ActivationPolicy, AppHandle, Manager};

    pub fn entrar(app: &AppHandle) {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.hide();
        }
        let _ = app.set_activation_policy(ActivationPolicy::Accessory);
    }

    pub fn salir(app: &AppHandle) {
        let _ = app.set_activation_policy(ActivationPolicy::Regular);
    }

    /// Menú de la app como el estándar, pero ⌘Q pasa a segundo plano (el «Salir» predefinido llama
    /// a `terminate:` y tao no deja interceptarlo) y ⌥⌘Q sale del todo.
    pub fn menu_app(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
        let nombre = app.package_info().name.clone();
        let app_menu = SubmenuBuilder::new(app, &nombre)
            .item(&PredefinedMenuItem::about(app, None, None)?)
            .separator()
            .item(&PredefinedMenuItem::services(app, None)?)
            .separator()
            .item(&PredefinedMenuItem::hide(app, None)?)
            .item(&PredefinedMenuItem::hide_others(app, None)?)
            .item(&PredefinedMenuItem::show_all(app, None)?)
            .separator()
            .item(&MenuItemBuilder::with_id("segundo-plano", "Cerrar ventana (sigue en el notch)").accelerator("CmdOrCtrl+Q").build(app)?)
            .item(&MenuItemBuilder::with_id("salir", format!("Salir de {nombre}")).accelerator("Alt+CmdOrCtrl+Q").build(app)?)
            .build()?;
        let edicion = SubmenuBuilder::new(app, "Edición")
            .undo()
            .redo()
            .separator()
            .cut()
            .copy()
            .paste()
            .select_all()
            .build()?;
        let ver = SubmenuBuilder::new(app, "Visualización").fullscreen().build()?;
        let ventana = SubmenuBuilder::new(app, "Ventana")
            .minimize()
            .maximize()
            .separator()
            .close_window()
            .build()?;
        MenuBuilder::new(app).items(&[&app_menu, &edicion, &ver, &ventana]).build()
    }

    pub fn evento_menu(app: &AppHandle, id: &str) {
        match id {
            "abrir" => {
                let _ = crate::abrir_principal(app);
            }
            "rapida" => crate::capturar(app),
            "segundo-plano" => entrar(app),
            "salir" => app.exit(0),
            _ => {}
        }
    }

    pub fn icono_barra(app: &AppHandle) -> tauri::Result<()> {
        let atajo = app
            .state::<std::sync::Mutex<crate::Biblioteca>>()
            .lock()
            .ok()
            .and_then(|b| b.atajo().map(str::to_string))
            .unwrap_or_else(|| crate::ATAJO_POR_DEFECTO.to_string());
        let menu = MenuBuilder::new(app)
            .item(&MenuItemBuilder::with_id("abrir", "Abrir Notas").build(app)?)
            .item(&MenuItemBuilder::with_id("rapida", "Nota rápida").accelerator(&atajo).build(app)?)
            .separator()
            .item(&MenuItemBuilder::with_id("salir", "Salir de Notas").build(app)?)
            .build()?;
        TrayIconBuilder::with_id("barra")
            .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/barra.png"))?)
            .icon_as_template(true)
            .tooltip("Notas")
            .menu(&menu)
            .build(app)?;
        Ok(())
    }
}

/// Ventana pequeña, sin bordes y siempre encima, con solo el modal de nota rápida.
/// No se puede redimensionar: así Hyprland la abre flotante y centrada.
fn abrir_rapida(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window("rapida") {
        w.show()?;
        return w.set_focus();
    }
    WebviewWindowBuilder::new(app, "rapida", WebviewUrl::App("rapida.html".into()))
        .title("md-notes-rapida")
        .inner_size(560.0, 420.0)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .center()
        .focused(true)
        .build()?;
    Ok(())
}

fn abrir_segun(app: &AppHandle, args: &[String]) {
    if args.iter().any(|a| a == ARG_CAPTURAR) {
        return capturar(app);
    }
    let r = if pide_rapida(args) { abrir_rapida(app) } else { abrir_principal(app) };
    if let Err(e) = r {
        eprintln!("no se pudo abrir la ventana: {e}");
    }
}

fn titulo_de_html(html: &str) -> String {
    let haystack = html.to_ascii_lowercase();
    let Some(inicio) = haystack.find("<title") else { return String::new(); };
    let resto = &html[inicio..];
    let Some(cierre) = resto.find('>') else { return String::new(); };
    let contenido = resto[cierre + 1..].split("</title>").next().unwrap_or("");
    let titulo = contenido
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    titulo.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let biblioteca = Biblioteca::abrir().expect("no se pudo abrir la carpeta de notas");
    tauri::Builder::default()
        // Primero: si ya hay una instancia, le pasa los argumentos y esta termina.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| abrir_segun(app, &args)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(Mutex::new(biblioteca))
        .manage(Mutex::new(indice::Indice::default()))
        .setup(|app| {
            let args: Vec<String> = std::env::args().collect();
            permitir_recursos(app.handle(), &app.state());
            // Primero el notch: `--capturar` al arrancar lo necesita ya creado.
            #[cfg(target_os = "macos")]
            if !pide_rapida(&args) {
                if let Err(e) = notch::crear(app.handle()) {
                    eprintln!("no se pudo crear el widget del notch: {e}");
                }
                // Los eventos del menú de la app y del de la barra llegan aquí (son globales).
                app.set_menu(segundo_plano::menu_app(app.handle())?)?;
                app.on_menu_event(|app, e| segundo_plano::evento_menu(app, e.id().as_ref()));
                if let Err(e) = segundo_plano::icono_barra(app.handle()) {
                    eprintln!("no se pudo crear el icono de la barra de menús: {e}");
                }
            }
            if !pide_rapida(&args) {
                registrar_atajo(app.handle());
            }
            if args.iter().any(|a| a == ARG_CAPTURAR) {
                // Arranque desde el atajo del sistema: la ventana principal no hace falta.
                capturar(app.handle());
                return Ok(());
            }
            abrir_segun(app.handle(), &args);
            Ok(())
        })
        .on_window_event(|ventana, evento| {
            #[cfg(target_os = "macos")]
            if let tauri::WindowEvent::CloseRequested { api, .. } = evento {
                if ventana.label() == "main" && ventana.app_handle().get_webview_window("notch").is_some() {
                    api.prevent_close();
                    segundo_plano::entrar(ventana.app_handle());
                }
            }
            let _ = (ventana, evento);
        })
        .invoke_handler(tauri::generate_handler![
            estado,
            leer_nota,
            buscar,
            crear_nota,
            guardar_nota,
            mover_nota,
            borrar_nota,
            crear_proyecto,
            renombrar_proyecto,
            borrar_proyecto,
            mover_proyecto,
            guardar_etiqueta,
            borrar_etiqueta,
            elegir_disco,
            elegir_archivo,
            olvidar_disco,
            abrir_carpeta,
            abrir_url,
            obtener_titulo_url,
            cerrar_rapida,
            guardar_tema,
            enlaces,
            relacionadas,
            pares_similares,
            grafo,
            contar_enlaces,
            renombrar_enlaces,
            tareas,
            marcar_tarea,
            plantillas,
            abrir_plantillas,
            captura::guardar_temporal,
            adjuntar_archivo,
            instantanea,
            versiones,
            leer_version,
            restaurar_version,
            abrir_adjunto,
            captura::adjuntar,
            captura::carpeta_nota,
            captura::portapapeles,
            captura::pestana_navegador,
            captura::texto_seleccionado,
            captura::capturar_pantalla,
            #[cfg(target_os = "macos")]
            notch::notch_cerrar,
        ])
        .build(tauri::generate_context!())
        .expect("error al iniciar la aplicación")
        .run(|_app, _evento| {
            // macOS: con el notch abierto la app sigue viva al cerrar la ventana principal;
            // un clic en el Dock la vuelve a abrir (la ventana del notch cuenta como visible).
            #[cfg(target_os = "macos")]
            match _evento {
                tauri::RunEvent::Reopen { .. } => {
                    let _ = abrir_principal(_app);
                }
                // ⌘Q (sin código): a segundo plano. `app.exit(0)` (menú de la barra) sí sale.
                tauri::RunEvent::ExitRequested { code: None, api, .. }
                    if _app.get_webview_window("notch").is_some() =>
                {
                    api.prevent_exit();
                    segundo_plano::entrar(_app);
                }
                _ => {}
            }
        });
}
