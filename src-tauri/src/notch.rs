//! Widget del notch (solo macOS): una ventana transparente encima del notch que, al pasar el
//! ratón, se despliega con un panel para capturar notas rápidas (`notch.html`).
//!
//! - La ventana vive en el nivel de la barra de estado (por encima de la barra de menús) y en
//!   todos los Spaces, también sobre apps a pantalla completa.
//! - El hover se detecta desde aquí, consultando la posición del ratón cada 50 ms: así funciona
//!   aunque la ventana no tenga el foco (WebKit no recibe `mouseenter` en ventanas inactivas).
//! - Plegada mide lo mismo que el notch; desplegada, lo que el panel. En pantallas sin notch
//!   (monitor externo, tapa cerrada) usa una franja fina arriba al centro.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::Duration;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSEvent, NSScreen, NSStatusWindowLevel, NSWindow, NSWindowCollectionBehavior, NSWorkspace,
    NSRunningApplication, NSApplicationActivationOptions};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const ETIQUETA: &str = "notch";

/// Tamaño del panel desplegado (puntos).
const PANEL_ANCHO: f64 = 480.0;
const PANEL_ALTO: f64 = 330.0;
/// Zona activa en pantallas sin notch.
const SIN_NOTCH: (f64, f64) = (200.0, 6.0);
/// Ticks seguidos con el ratón dentro antes de abrir (evita aperturas al cruzar de paso).
const TICKS_PARA_ABRIR: u8 = 2;

static ABIERTO: AtomicBool = AtomicBool::new(false);
static FUERA: AtomicBool = AtomicBool::new(false);
static TICKS_DENTRO: AtomicU8 = AtomicU8::new(0);
/// Si la app estaba activa al abrir el panel; si no, al cerrar se devuelve el foco a la anterior.
static ESTABA_ACTIVA: AtomicBool = AtomicBool::new(false);
/// App que estaba delante al abrir el panel (pid, bundle id): de ella se toman la pestaña del
/// navegador y el texto seleccionado.
static APP_ANTERIOR: Mutex<Option<(i32, String)>> = Mutex::new(None);
/// Abierto con el atajo: no se pliega al estar el ratón fuera.
static POR_ATAJO: AtomicBool = AtomicBool::new(false);
/// Tras cerrar, el ratón tiene que salir del notch antes de poder abrirlo otra vez.
static ESPERA_SALIDA: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
struct Zona {
    notch: NSRect,
    panel: NSRect,
    hay_notch: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Apertura {
    notch_ancho: f64,
    notch_alto: f64,
    hay_notch: bool,
    /// Abierto con el atajo: el panel toma el foco y no se pliega al sacar el ratón.
    foco: bool,
}

/// La app que estaba delante al abrir el panel (si no era Notas).
pub fn app_anterior() -> Option<(i32, String)> {
    APP_ANTERIOR.lock().ok()?.clone()
}

/// Calcula el rectángulo del notch y del panel (coordenadas de Cocoa: origen abajo a la izquierda).
fn zona(mtm: MainThreadMarker) -> Option<Zona> {
    let pantallas = NSScreen::screens(mtm);
    let pantalla: Retained<NSScreen> = pantallas
        .iter()
        .find(|s| s.safeAreaInsets().top > 0.0)
        .or_else(|| NSScreen::mainScreen(mtm))?;
    let marco = pantalla.frame();
    let alto_notch = pantalla.safeAreaInsets().top;
    let hay_notch = alto_notch > 0.0;
    let (ancho, alto) = if hay_notch {
        let izq = pantalla.auxiliaryTopLeftArea();
        let der = pantalla.auxiliaryTopRightArea();
        let ancho = marco.size.width - izq.size.width - der.size.width;
        (if ancho > 40.0 { ancho } else { SIN_NOTCH.0 }, alto_notch)
    } else {
        SIN_NOTCH
    };
    let arriba = marco.origin.y + marco.size.height;
    let centro = marco.origin.x + marco.size.width / 2.0;
    let panel_ancho = PANEL_ANCHO.max(ancho + 80.0);
    Some(Zona {
        notch: NSRect::new(NSPoint::new(centro - ancho / 2.0, arriba - alto), NSSize::new(ancho, alto)),
        panel: NSRect::new(NSPoint::new(centro - panel_ancho / 2.0, arriba - PANEL_ALTO), NSSize::new(panel_ancho, PANEL_ALTO)),
        hay_notch,
    })
}

fn contiene(r: NSRect, p: NSPoint) -> bool {
    p.x >= r.origin.x && p.x <= r.origin.x + r.size.width && p.y >= r.origin.y && p.y <= r.origin.y + r.size.height
}

fn ns_window(w: &WebviewWindow) -> Option<&NSWindow> {
    let ptr = w.ns_window().ok()?;
    // SAFETY: Tauri devuelve el puntero a la NSWindow viva de esta ventana.
    unsafe { (ptr as *const NSWindow).as_ref() }
}

/// Crea la ventana del notch (plegada). Se llama una vez desde `setup`.
pub fn crear(app: &AppHandle) -> tauri::Result<()> {
    if app.get_webview_window(ETIQUETA).is_some() {
        return Ok(());
    }
    let ventana = WebviewWindowBuilder::new(app, ETIQUETA, WebviewUrl::App("notch.html".into()))
        .title("md-notes-notch")
        .inner_size(SIN_NOTCH.0, SIN_NOTCH.1)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .skip_taskbar(true)
        .accept_first_mouse(true)
        .focused(false)
        .visible(false)
        .build()?;

    if let (Some(mtm), Some(nsw)) = (MainThreadMarker::new(), ns_window(&ventana)) {
        nsw.setLevel(NSStatusWindowLevel);
        nsw.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle
                | NSWindowCollectionBehavior::FullScreenAuxiliary,
        );
        nsw.setHasShadow(false);
        if let Some(z) = zona(mtm) {
            nsw.setFrame_display(z.notch, true);
        }
    }
    ventana.show()?;

    let app = app.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(50));
            let a = app.clone();
            if app.run_on_main_thread(move || tick(&a)).is_err() {
                break; // la app está terminando
            }
        }
    });
    Ok(())
}

/// Un paso del sondeo del ratón (hilo principal).
fn tick(app: &AppHandle) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Some(ventana) = app.get_webview_window(ETIQUETA) else { return };
    let Some(nsw) = ns_window(&ventana) else { return };
    let Some(z) = zona(mtm) else { return };
    let raton = NSEvent::mouseLocation();

    if ABIERTO.load(Ordering::SeqCst) {
        if POR_ATAJO.load(Ordering::SeqCst) {
            return;
        }
        let fuera = !contiene(z.panel, raton);
        if fuera != FUERA.swap(fuera, Ordering::SeqCst) {
            let _ = ventana.emit_to(ETIQUETA, if fuera { "notch:fuera" } else { "notch:dentro" }, ());
        }
        return;
    }

    // Plegada: se mantiene pegada al notch (por si cambia la pantalla o la resolución).
    if nsw.frame() != z.notch {
        nsw.setFrame_display(z.notch, true);
    }
    if !contiene(z.notch, raton) {
        TICKS_DENTRO.store(0, Ordering::SeqCst);
        ESPERA_SALIDA.store(false, Ordering::SeqCst);
        return;
    }
    if ESPERA_SALIDA.load(Ordering::SeqCst) {
        return;
    }
    if TICKS_DENTRO.fetch_add(1, Ordering::SeqCst) + 1 < TICKS_PARA_ABRIR {
        return;
    }
    TICKS_DENTRO.store(0, Ordering::SeqCst);
    abrir(mtm, &ventana, nsw, z, false);
}

/// Despliega el panel. Con `foco`, además activa la ventana para escribir directamente.
fn abrir(mtm: MainThreadMarker, ventana: &WebviewWindow, nsw: &NSWindow, z: Zona, foco: bool) {
    ABIERTO.store(true, Ordering::SeqCst);
    FUERA.store(false, Ordering::SeqCst);
    POR_ATAJO.store(foco, Ordering::SeqCst);
    let ns_app = NSApplication::sharedApplication(mtm);
    ESTABA_ACTIVA.store(ns_app.isActive(), Ordering::SeqCst);
    if !ns_app.isActive() {
        let delante = NSWorkspace::sharedWorkspace().frontmostApplication();
        let anterior = delante.and_then(|a| Some((a.processIdentifier(), a.bundleIdentifier()?.to_string())));
        if let Ok(mut a) = APP_ANTERIOR.lock() {
            *a = anterior;
        }
    }
    nsw.setFrame_display(z.panel, true);
    if foco {
        let _ = ventana.set_focus();
    }
    let _ = ventana.emit_to(
        ETIQUETA,
        "notch:abrir",
        Apertura { notch_ancho: z.notch.size.width, notch_alto: z.notch.size.height, hay_notch: z.hay_notch, foco },
    );
}

/// Atajo global (o `md-notes --capturar`): abre el panel con el foco en el texto. Si ya está
/// abierto y activo, lo cierra; si está abierto sin foco, se lo da.
pub fn atajo(app: &AppHandle) {
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let Some(ventana) = a.get_webview_window(ETIQUETA) else { return };
        let Some(nsw) = ns_window(&ventana) else { return };
        let Some(z) = zona(mtm) else { return };
        if !ABIERTO.load(Ordering::SeqCst) {
            abrir(mtm, &ventana, nsw, z, true);
        } else if ventana.is_focused().unwrap_or(false) {
            let _ = ventana.emit_to(ETIQUETA, "notch:cerrar", ());
        } else {
            POR_ATAJO.store(true, Ordering::SeqCst);
            let _ = ventana.set_focus();
            let _ = ventana.emit_to(ETIQUETA, "notch:foco", ());
        }
    });
}

/// Reactiva la app que estaba delante al abrir el panel; si ya no existe, deja que macOS elija.
#[allow(deprecated)] // `activateWithOptions` sigue siendo la forma de activar otra app sin pedirle permiso
fn devolver_foco_a_anterior(ns_app: &NSApplication) {
    let anterior = app_anterior().and_then(|(pid, _)| NSRunningApplication::runningApplicationWithProcessIdentifier(pid));
    match anterior {
        Some(app) if !app.isTerminated() => {
            app.activateWithOptions(NSApplicationActivationOptions::empty());
        }
        _ => ns_app.deactivate(),
    }
}

/// Pliega el panel (lo llama `notch.ts` al terminar la animación de cierre).
/// Con `devolver_foco`, si la app no estaba activa antes de abrir, se devuelve el foco a la anterior.
#[tauri::command]
pub fn notch_cerrar(app: AppHandle, devolver_foco: bool) {
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(mtm) = MainThreadMarker::new() else { return };
        if let (Some(ventana), Some(z)) = (a.get_webview_window(ETIQUETA), zona(mtm)) {
            if let Some(nsw) = ns_window(&ventana) {
                nsw.setFrame_display(z.notch, true);
            }
        }
        ABIERTO.store(false, Ordering::SeqCst);
        ESPERA_SALIDA.store(true, Ordering::SeqCst);
        TICKS_DENTRO.store(0, Ordering::SeqCst);
        let ns_app = NSApplication::sharedApplication(mtm);
        if devolver_foco && !ESTABA_ACTIVA.load(Ordering::SeqCst) && ns_app.isActive() {
            devolver_foco_a_anterior(&ns_app);
        }
    });
}
