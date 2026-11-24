//! Captura rápida: lo que tienes delante entra en la nota.
//!
//! - Adjuntos temporales: las imágenes (portapapeles, pegado, captura de pantalla) se guardan
//!   primero en una carpeta temporal y el texto las referencia como `adjunto:<nombre>`. Al guardar
//!   la nota, `adjuntar` las mueve a `assets/` junto a la nota y devuelve la ruta relativa.
//! - Portapapeles, pestaña activa del navegador, texto seleccionado en otra app.
//! - Captura de pantalla con OCR (Vision en macOS; `tesseract` en Linux si está instalado).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use tauri::ipc::{InvokeBody, Request};
use tauri::WebviewWindow;

use crate::store::Lugar;
use crate::{Bib, Res, con};

const EXTENSIONES: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "heic", "tiff"];

pub fn dir_temporal() -> PathBuf {
    std::env::temp_dir().join("md-notes-capturas")
}

fn nombre_nuevo(prefijo: &str, ext: &str) -> String {
    format!("{prefijo}-{}.{ext}", chrono::Local::now().format("%Y%m%d-%H%M%S-%3f"))
}

fn ruta_temporal(nombre: &str) -> Res<PathBuf> {
    let valido = !nombre.is_empty() && !nombre.contains(['/', '\\']) && !nombre.starts_with('.');
    if !valido {
        return Err(format!("adjunto no válido: {nombre}"));
    }
    Ok(dir_temporal().join(nombre))
}

fn guardar_bytes(prefijo: &str, ext: &str, bytes: &[u8]) -> Res<String> {
    let dir = dir_temporal();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let nombre = nombre_nuevo(prefijo, ext);
    fs::write(dir.join(&nombre), bytes).map_err(|e| e.to_string())?;
    Ok(nombre)
}

/// Guarda bytes crudos (imagen pegada en el webview) como adjunto temporal.
/// Cabecera `x-ext`: extensión del archivo. Devuelve el nombre del adjunto.
#[tauri::command]
pub fn guardar_temporal(request: Request) -> Res<String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("se esperaban bytes".into());
    };
    let ext = request
        .headers()
        .get("x-ext")
        .and_then(|v| v.to_str().ok())
        .map(|e| e.to_ascii_lowercase())
        .filter(|e| EXTENSIONES.contains(&e.as_str()))
        .unwrap_or_else(|| "png".into());
    guardar_bytes("pegado", &ext, bytes)
}

/// Mueve un adjunto temporal a `assets/` junto a la nota. Devuelve la ruta relativa a la nota.
#[tauri::command]
pub fn adjuntar(b: Bib, nota: String, nombre: String) -> Res<String> {
    let ruta_nota = con(&b, |b| b.ruta(Some(&nota), Lugar::Equipo))?;
    adjuntar_a(&ruta_nota, &nombre)
}

/// Igual que `adjuntar`, con la ruta de la nota ya resuelta (lo usa la línea de comandos).
pub fn adjuntar_a(ruta_nota: &Path, nombre: &str) -> Res<String> {
    let origen = ruta_temporal(nombre)?;
    if !origen.is_file() {
        return Err(format!("el adjunto ya no existe: {nombre}"));
    }
    mover_a_assets(&origen, ruta_nota)
}

/// Mueve `origen` a `assets/` junto a `ruta_nota` sin pisar nada. Devuelve `assets/<nombre>`.
fn mover_a_assets(origen: &Path, ruta_nota: &Path) -> Res<String> {
    let assets = ruta_nota.parent().ok_or("nota sin carpeta")?.join("assets");
    fs::create_dir_all(&assets).map_err(|e| e.to_string())?;
    let nombre = origen.file_name().ok_or("adjunto sin nombre")?;
    let stem = Path::new(nombre).file_stem().unwrap_or_default().to_string_lossy().to_string();
    let ext = Path::new(nombre).extension().unwrap_or_default().to_string_lossy().to_string();
    let mut destino = assets.join(nombre);
    let mut n = 2;
    while destino.exists() {
        destino = assets.join(format!("{stem}-{n}.{ext}"));
        n += 1;
    }
    // `rename` falla entre volúmenes (p. ej. temporal → disco externo): se copia y se borra.
    if fs::rename(origen, &destino).is_err() {
        fs::copy(origen, &destino).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(origen);
    }
    Ok(format!("assets/{}", destino.file_name().unwrap_or_default().to_string_lossy()))
}

/// Carpeta absoluta de una nota, para resolver sus imágenes relativas en la vista previa.
#[tauri::command]
pub fn carpeta_nota(b: Bib, id: String) -> Res<String> {
    let ruta = con(&b, |b| b.ruta(Some(&id), Lugar::Equipo))?;
    Ok(ruta.parent().unwrap_or(&ruta).display().to_string())
}

// ------------------------------------------------------------------ portapapeles

#[derive(Serialize)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum Portapapeles {
    Texto { texto: String },
    Imagen { nombre: String },
    Vacio,
}

#[tauri::command]
pub fn portapapeles() -> Res<Portapapeles> {
    sistema::portapapeles()
}

// ------------------------------------------------------------------ navegador y selección

#[derive(Serialize)]
pub struct Pestana {
    url: String,
    titulo: String,
}

#[tauri::command]
pub fn pestana_navegador() -> Res<Pestana> {
    sistema::pestana_navegador()
}

#[tauri::command]
pub fn texto_seleccionado() -> Res<String> {
    sistema::texto_seleccionado()
}

// ------------------------------------------------------------------ captura de pantalla

#[derive(Serialize)]
pub struct Captura {
    nombre: String,
    texto: String,
}

/// Oculta la ventana que llama, deja elegir una zona de la pantalla y extrae su texto.
/// `None` si se canceló.
#[tauri::command]
pub async fn capturar_pantalla(window: WebviewWindow) -> Res<Option<Captura>> {
    let dir = dir_temporal();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let nombre = nombre_nuevo("captura", "png");
    let ruta = dir.join(&nombre);

    window.hide().map_err(|e| e.to_string())?;
    let r = tauri::async_runtime::spawn_blocking({
        let ruta = ruta.clone();
        move || sistema::capturar(&ruta)
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);
    let _ = window.show();
    let _ = window.set_focus();
    r?;

    if !ruta.is_file() {
        return Ok(None); // cancelada (Esc)
    }
    let texto = tauri::async_runtime::spawn_blocking({
        let ruta = ruta.clone();
        move || sistema::ocr(&ruta)
    })
    .await
    .unwrap_or_default();
    Ok(Some(Captura { nombre, texto }))
}

/// Captura sin ventana que ocultar (línea de comandos). `None` si se canceló.
pub fn capturar_a_temporal() -> Res<Option<Captura>> {
    let dir = dir_temporal();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let nombre = nombre_nuevo("captura", "png");
    let ruta = dir.join(&nombre);
    sistema::capturar(&ruta)?;
    if !ruta.is_file() {
        return Ok(None);
    }
    let texto = sistema::ocr(&ruta);
    Ok(Some(Captura { nombre, texto }))
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
fn ejecutar(programa: &str, args: &[&str]) -> Option<Vec<u8>> {
    let salida = Command::new(programa).args(args).output().ok()?;
    salida.status.success().then_some(salida.stdout)
}

// ------------------------------------------------------------------ macOS

#[cfg(target_os = "macos")]
mod sistema {
    use std::path::Path;
    use std::ptr::NonNull;

    use objc2::rc::Retained;
    use objc2::AnyThread;
    use objc2_app_kit::{
        NSBitmapImageFileType, NSBitmapImageRep, NSPasteboard, NSPasteboardTypePNG, NSPasteboardTypeString,
        NSPasteboardTypeTIFF, NSRunningApplication, NSWorkspace,
    };
    use objc2_application_services::{AXIsProcessTrustedWithOptions, AXUIElement, kAXTrustedCheckOptionPrompt};
    use objc2_core_foundation::{CFBoolean, CFDictionary, CFRetained, CFString, CFType};
    use objc2_foundation::{NSArray, NSDictionary, NSString, NSURL};
    use objc2_vision::{
        VNImageRequestHandler, VNRecognizeTextRequest, VNRecognizedTextObservation, VNRequest,
        VNRequestTextRecognitionLevel,
    };

    use super::{Pestana, Portapapeles, guardar_bytes};
    use crate::Res;

    pub fn portapapeles() -> Res<Portapapeles> {
        let pb = NSPasteboard::generalPasteboard();
        // Texto primero: al copiar texto con formato suele venir también una imagen de vista previa.
        if let Some(texto) = unsafe { pb.stringForType(NSPasteboardTypeString) } {
            let texto = texto.to_string();
            if !texto.trim().is_empty() {
                return Ok(Portapapeles::Texto { texto });
            }
        }
        if let Some(png) = unsafe { pb.dataForType(NSPasteboardTypePNG) } {
            return Ok(Portapapeles::Imagen { nombre: guardar_bytes("portapapeles", "png", &png.to_vec())? });
        }
        if let Some(tiff) = unsafe { pb.dataForType(NSPasteboardTypeTIFF) } {
            let png = NSBitmapImageRep::imageRepWithData(&tiff).and_then(|rep| unsafe {
                rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
            });
            if let Some(png) = png {
                return Ok(Portapapeles::Imagen { nombre: guardar_bytes("portapapeles", "png", &png.to_vec())? });
            }
        }
        Ok(Portapapeles::Vacio)
    }

    /// Navegadores con AppleScript: (bundle id, estilo Safari).
    const NAVEGADORES: &[(&str, bool)] = &[
        ("com.apple.Safari", true),
        ("com.apple.SafariTechnologyPreview", true),
        ("com.google.Chrome", false),
        ("company.thebrowser.Browser", false), // Arc
        ("com.brave.Browser", false),
        ("com.microsoft.edgemac", false),
        ("com.vivaldi.Vivaldi", false),
        ("com.operasoftware.Opera", false),
    ];

    fn bundle(app: &NSRunningApplication) -> Option<String> {
        app.bundleIdentifier().map(|b| b.to_string())
    }

    /// El navegador del que se toma la pestaña: la app en la que estabas si es un navegador; si no,
    /// el primero de la lista que esté abierto.
    fn navegador() -> Option<(&'static str, bool)> {
        if let Some((_, id)) = crate::notch::app_anterior()
            && let Some(n) = NAVEGADORES.iter().find(|(b, _)| *b == id)
        {
            return Some(*n);
        }
        let abiertas: Vec<String> = NSWorkspace::sharedWorkspace().runningApplications().iter().filter_map(|a| bundle(&a)).collect();
        NAVEGADORES.iter().find(|(b, _)| abiertas.iter().any(|a| a == b)).copied()
    }

    pub fn pestana_navegador() -> Res<Pestana> {
        let Some((id, safari)) = navegador() else {
            return Err("No hay ningún navegador compatible abierto (Safari, Chrome, Arc, Brave, Edge…)".into());
        };
        let (tab, nombre) = if safari { ("current tab", "name") } else { ("active tab", "title") };
        let script = format!(
            "tell application id \"{id}\"\nset t to {tab} of front window\nreturn (URL of t) & linefeed & ({nombre} of t)\nend tell"
        );
        let salida = std::process::Command::new("osascript").args(["-e", &script]).output().map_err(|e| e.to_string())?;
        if !salida.status.success() {
            let err = String::from_utf8_lossy(&salida.stderr);
            return Err(if err.contains("-1743") {
                "Sin permiso: Ajustes del Sistema → Privacidad y seguridad → Automatización → Notas".into()
            } else {
                format!("No se pudo leer la pestaña: {}", err.trim())
            });
        }
        let texto = String::from_utf8_lossy(&salida.stdout);
        let mut lineas = texto.lines();
        let url = lineas.next().unwrap_or_default().trim().to_string();
        let titulo = lineas.collect::<Vec<_>>().join(" ").trim().to_string();
        if url.is_empty() {
            return Err("El navegador no tiene ninguna pestaña abierta".into());
        }
        Ok(Pestana { url, titulo })
    }

    fn atributo(elemento: &AXUIElement, nombre: &'static str) -> Option<CFRetained<CFType>> {
        let mut valor: *const CFType = std::ptr::null();
        let err = unsafe { elemento.copy_attribute_value(&CFString::from_static_str(nombre), NonNull::from(&mut valor)) };
        if err.0 != 0 || valor.is_null() {
            return None;
        }
        // SAFETY: `Copy…` devuelve una referencia con +1 que pasa a ser nuestra.
        Some(unsafe { CFRetained::from_raw(NonNull::new_unchecked(valor as *mut CFType)) })
    }

    pub fn texto_seleccionado() -> Res<String> {
        let clave: &CFString = unsafe { kAXTrustedCheckOptionPrompt };
        let opciones = CFDictionary::<CFString, CFBoolean>::from_slices(&[clave], &[CFBoolean::new(true)]);
        if !unsafe { AXIsProcessTrustedWithOptions(Some(opciones.as_opaque())) } {
            return Err("Notas necesita permiso de Accesibilidad para leer la selección: Ajustes del Sistema → Privacidad y seguridad → Accesibilidad. Después, vuelve a intentarlo.".into());
        }
        let Some((pid, _)) = crate::notch::app_anterior() else {
            return Err("No sé en qué app estabas: abre el panel desde otra app".into());
        };
        let app = unsafe { AXUIElement::new_application(pid) };
        let texto = atributo(&app, "AXFocusedUIElement")
            .and_then(|f| f.downcast::<AXUIElement>().ok())
            .and_then(|f| atributo(&f, "AXSelectedText"))
            .and_then(|t| t.downcast::<CFString>().ok())
            .map(|t| t.to_string())
            .unwrap_or_default();
        if texto.trim().is_empty() {
            return Err("No hay texto seleccionado en la app anterior (o esa app no lo comparte)".into());
        }
        Ok(texto)
    }

    pub fn capturar(ruta: &Path) -> Res<()> {
        let ruta = ruta.to_string_lossy();
        // -i: selección interactiva (Esc cancela y no crea archivo) · -x: sin sonido
        std::process::Command::new("screencapture")
            .args(["-i", "-x", &ruta])
            .status()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Texto de una imagen con el OCR de Vision (incluido en macOS; funciona sin conexión).
    pub fn ocr(ruta: &Path) -> String {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&ruta.to_string_lossy()));
        let handler = unsafe { VNImageRequestHandler::initWithURL_options(VNImageRequestHandler::alloc(), &url, &NSDictionary::new()) };
        let peticion = VNRecognizeTextRequest::new();
        peticion.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        peticion.setUsesLanguageCorrection(true);
        peticion.setRecognitionLanguages(&NSArray::from_retained_slice(&[NSString::from_str("es-ES"), NSString::from_str("en-US")]));
        let generica: Retained<VNRequest> = Retained::into_super(Retained::into_super(peticion.clone()));
        if handler.performRequests_error(&NSArray::from_retained_slice(&[generica])).is_err() {
            return String::new();
        }
        let Some(resultados) = peticion.results() else { return String::new() };
        resultados
            .iter()
            .filter_map(|o| o.downcast::<VNRecognizedTextObservation>().ok())
            .filter_map(|o| o.topCandidates(1).firstObject().map(|c| c.string().to_string()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ------------------------------------------------------------------ Linux y otros

#[cfg(not(target_os = "macos"))]
mod sistema {
    use std::path::Path;

    use super::{Pestana, Portapapeles, ejecutar, guardar_bytes};
    use crate::Res;

    fn texto(primaria: bool) -> Option<String> {
        let wl: &[&str] = if primaria { &["--primary", "--no-newline"] } else { &["--no-newline"] };
        let x: &[&str] = if primaria { &["-o", "-selection", "primary"] } else { &["-o", "-selection", "clipboard"] };
        ejecutar("wl-paste", wl)
            .or_else(|| ejecutar("xclip", x))
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .filter(|t| !t.trim().is_empty())
    }

    pub fn portapapeles() -> Res<Portapapeles> {
        if let Some(texto) = texto(false) {
            return Ok(Portapapeles::Texto { texto });
        }
        let tipos = ejecutar("wl-paste", &["--list-types"]).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default();
        if tipos.lines().any(|t| t == "image/png")
            && let Some(png) = ejecutar("wl-paste", &["--type", "image/png"])
        {
            return Ok(Portapapeles::Imagen { nombre: guardar_bytes("portapapeles", "png", &png)? });
        }
        Ok(Portapapeles::Vacio)
    }

    pub fn pestana_navegador() -> Res<Pestana> {
        Err("Leer la pestaña del navegador solo está disponible en macOS".into())
    }

    /// En Linux, el texto seleccionado es la selección primaria.
    pub fn texto_seleccionado() -> Res<String> {
        texto(true).ok_or_else(|| "No hay texto seleccionado".into())
    }

    pub fn capturar(ruta: &Path) -> Res<()> {
        let Some(zona) = ejecutar("slurp", &[]) else { return Ok(()) }; // cancelada o sin slurp
        let zona = String::from_utf8_lossy(&zona).trim().to_string();
        ejecutar("grim", &["-g", &zona, &ruta.to_string_lossy()]).ok_or("no se pudo capturar (¿está instalado grim?)")?;
        Ok(())
    }

    pub fn ocr(ruta: &Path) -> String {
        ejecutar("tesseract", &[&ruta.to_string_lossy(), "-", "-l", "spa+eng"])
            .map(|b| String::from_utf8_lossy(&b).trim().to_string())
            .unwrap_or_default()
    }
}


#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn adjuntos_van_a_assets_sin_pisar() {
        let base = std::env::temp_dir().join(format!("md-notes-prueba-adjuntos-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("tmp")).unwrap();
        let nota = base.join("proyecto/nota.md");
        fs::create_dir_all(nota.parent().unwrap()).unwrap();
        for _ in 0..2 {
            let origen = base.join("tmp/captura.png");
            fs::write(&origen, b"png").unwrap();
            let _ = mover_a_assets(&origen, &nota).unwrap();
            assert!(!origen.exists(), "el temporal se mueve");
        }
        assert!(base.join("proyecto/assets/captura.png").is_file());
        assert!(base.join("proyecto/assets/captura-2.png").is_file());
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn nombres_de_adjunto_no_escapan_de_la_carpeta_temporal() {
        for malo in ["", "../x.png", "a/b.png", ".oculto", "c\\d.png"] {
            assert!(ruta_temporal(malo).is_err(), "{malo}");
        }
        assert!(ruta_temporal("captura-1.png").is_ok());
    }
}
