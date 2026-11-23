mod biblioteca;
mod frontmatter;
pub mod mcp;
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

/// Abre el selector de carpetas y usa la elegida como «Disco». `None` si se canceló.
#[tauri::command]
async fn elegir_disco(app: AppHandle, b: Bib<'_>) -> Res<Option<String>> {
    let Some(carpeta) = app.dialog().file().set_title("Carpeta del disco para tus notas").blocking_pick_folder()
    else {
        return Ok(None);
    };
    let ruta = carpeta.into_path().map_err(|e| e.to_string())?;
    con(&b, |b| b.elegir_disco(&ruta)).map(Some)
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

/// Cierra la ventana de nota rápida. Si la app se abrió solo para ella, el proceso termina.
#[tauri::command]
fn cerrar_rapida(window: WebviewWindow) -> Res<()> {
    window.close().map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ ventanas

/// Argumento con el que la barra del sistema pide el modal de nota rápida: `md-notes --rapida`.
const ARG_RAPIDA: &str = "--rapida";

fn pide_rapida(args: &[String]) -> bool {
    args.iter().any(|a| a == ARG_RAPIDA)
}

fn abrir_principal(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window("main") {
        w.unminimize()?;
        w.show()?;
        return w.set_focus();
    }
    let config = app.config().app.windows.iter().find(|w| w.label == "main").cloned().unwrap_or_default();
    WebviewWindowBuilder::from_config(app, &config)?.build()?;
    Ok(())
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
        .manage(Mutex::new(biblioteca))
        .setup(|app| {
            let args: Vec<String> = std::env::args().collect();
            abrir_segun(app.handle(), &args);
            Ok(())
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
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar la aplicación");
}
