//! Adjuntos de las notas: archivos en `assets/` junto a la nota, referenciados con rutas
//! relativas (`![](assets/foto.png)`, `[informe](assets/informe.pdf)`, `src="assets/…"`).
//!
//! - Pegar o arrastrar en el editor escribe el archivo en `assets/` sin pisar nada.
//! - Al mover una nota a otro proyecto, sus adjuntos la acompañan (se copian si otra nota de la
//!   carpeta de origen también los usa).
//! - Al borrar una nota, los adjuntos que ninguna otra nota usa van a la papelera con ella.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::store::slug;

pub const CARPETA: &str = "assets";

/// Rutas `assets/…` que usa un texto, sin repetir y en orden de aparición.
pub fn referencias(texto: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for marca in ["](", "](<", "src=\"", "src='", "href=\""] {
        let mut resto = texto;
        while let Some(i) = resto.find(marca) {
            resto = &resto[i + marca.len()..];
            let Some(ruta) = resto.strip_prefix("./").unwrap_or(resto).strip_prefix("assets/") else { continue };
            // Con `<…>` el nombre puede tener espacios; sin ellos, termina en el primer espacio.
            let fin = |c: &char| if marca.ends_with('<') { matches!(c, '>' | '\n') } else { matches!(c, ')' | '>' | '"' | '\'' | ' ' | '\n' | '#' | '?') };
            let nombre: String = ruta.chars().take_while(|c| !fin(c)).collect();
            let valido = !nombre.is_empty() && !nombre.contains("..") && !nombre.starts_with('/');
            let r = format!("{CARPETA}/{nombre}");
            if valido && !out.contains(&r) {
                out.push(r);
            }
        }
    }
    out
}

/// Nombre de archivo seguro para un adjunto: `<slug>.<ext>`; si el nombre no dice nada
/// (`image.png` del portapapeles), `pegado-<fecha>.<ext>`.
pub fn nombre_seguro(original: &str) -> String {
    let p = Path::new(original);
    let ext: String = p
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(10)
        .collect();
    let base = slug(&p.file_stem().unwrap_or_default().to_string_lossy());
    let base = if base.is_empty() || base == "image" || base == "imagen" {
        format!("pegado-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"))
    } else {
        base
    };
    if ext.is_empty() { base } else { format!("{base}.{ext}") }
}

/// `dir/nombre` si está libre; si no, `dir/<stem>-2.<ext>`, `-3`…
pub fn ruta_libre(dir: &Path, nombre: &str) -> PathBuf {
    let p = Path::new(nombre);
    let stem = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let mut ruta = dir.join(nombre);
    let mut n = 2;
    while ruta.exists() {
        ruta = dir.join(format!("{stem}-{n}{ext}"));
        n += 1;
    }
    ruta
}

/// Escribe bytes como adjunto de la nota `ruta_nota`. Devuelve la ruta relativa (`assets/…`).
pub fn guardar(ruta_nota: &Path, nombre: &str, bytes: &[u8]) -> Result<String> {
    let dir = ruta_nota.parent().context("nota sin carpeta")?.join(CARPETA);
    fs::create_dir_all(&dir)?;
    let destino = ruta_libre(&dir, &nombre_seguro(nombre));
    fs::write(&destino, bytes).with_context(|| format!("escribiendo {}", destino.display()))?;
    Ok(format!("{CARPETA}/{}", destino.file_name().unwrap_or_default().to_string_lossy()))
}

/// Referencias `assets/…` de las demás notas `.md` de la carpeta (excepto `excepto`).
pub fn usados_por_otras(carpeta: &Path, excepto: &Path) -> HashSet<String> {
    let Ok(entradas) = fs::read_dir(carpeta) else { return HashSet::new() };
    entradas
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p != excepto && p.extension().is_some_and(|e| e == "md"))
        .filter_map(|p| fs::read_to_string(p).ok())
        .flat_map(|t| referencias(&t))
        .collect()
}

/// Lleva los adjuntos de una nota de `origen` (carpeta) a la carpeta de `nota_destino`.
/// Si un nombre ya existe en el destino con otro contenido, usa uno libre y reescribe la
/// referencia en la nota movida. Se copian los que otra nota del origen también usa.
pub fn acompanar(texto: &str, origen: &Path, nota_origen: &Path, nota_destino: &Path) -> Result<Option<String>> {
    let refs = referencias(texto);
    if refs.is_empty() {
        return Ok(None);
    }
    let destino = nota_destino.parent().context("nota sin carpeta")?;
    let compartidos = usados_por_otras(origen, nota_origen);
    let mut nuevo = texto.to_string();
    let mut cambiado = false;
    for r in refs {
        let desde = origen.join(&r);
        if !desde.is_file() {
            continue;
        }
        let mut hasta = destino.join(&r);
        if hasta.exists() {
            if fs::read(&hasta).ok() == fs::read(&desde).ok() {
                if !compartidos.contains(&r) {
                    let _ = fs::remove_file(&desde);
                }
                continue; // mismo archivo ya en el destino
            }
            hasta = ruta_libre(hasta.parent().unwrap(), &hasta.file_name().unwrap().to_string_lossy());
            let r2 = format!("{CARPETA}/{}", hasta.file_name().unwrap().to_string_lossy());
            nuevo = nuevo.replace(&r, &r2);
            cambiado = true;
        }
        fs::create_dir_all(hasta.parent().unwrap())?;
        if compartidos.contains(&r) || fs::rename(&desde, &hasta).is_err() {
            fs::copy(&desde, &hasta).with_context(|| format!("copiando {}", desde.display()))?;
            if !compartidos.contains(&r) {
                let _ = fs::remove_file(&desde);
            }
        }
    }
    Ok(cambiado.then_some(nuevo))
}

/// Adjuntos de una nota que ninguna otra nota de su carpeta usa (para llevarlos a la papelera).
pub fn huerfanos_al_borrar(texto: &str, nota: &Path) -> Vec<PathBuf> {
    let Some(carpeta) = nota.parent() else { return Vec::new() };
    let compartidos = usados_por_otras(carpeta, nota);
    referencias(texto)
        .into_iter()
        .filter(|r| !compartidos.contains(r))
        .map(|r| carpeta.join(r))
        .filter(|p| p.is_file())
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn encuentra_referencias() {
        let t = "![a](assets/a.png) [pdf](./assets/informe.pdf) ![](<assets/b c.png>)\n<img src=\"assets/d.png\"> ![x](https://x/assets/no.png) [](assets/../fuera.txt) ![a](assets/a.png)";
        assert_eq!(referencias(t), ["assets/a.png", "assets/informe.pdf", "assets/b c.png", "assets/d.png"]);
    }

    #[test]
    fn nombres_seguros() {
        assert_eq!(nombre_seguro("Informe Final (v2).PDF"), "informe-final-v2.pdf");
        assert!(nombre_seguro("image.png").starts_with("pegado-"));
        assert_eq!(nombre_seguro("../../etc/passwd"), "passwd");
    }

    #[test]
    fn mover_y_borrar_con_adjuntos() {
        let base = std::env::temp_dir().join(format!("md-notes-adjuntos-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let (a, b) = (base.join("a"), base.join("b"));
        fs::create_dir_all(a.join("assets")).unwrap();
        fs::create_dir_all(b.join("assets")).unwrap();
        fs::write(a.join("assets/solo.png"), "1").unwrap();
        fs::write(a.join("assets/comun.png"), "2").unwrap();
        fs::write(b.join("assets/solo.png"), "otro").unwrap(); // choque de nombre en el destino
        let texto = "![](assets/solo.png) ![](assets/comun.png)";
        fs::write(a.join("nota.md"), texto).unwrap();
        fs::write(a.join("otra.md"), "![](assets/comun.png)").unwrap();

        let nuevo = acompanar(texto, &a, &a.join("nota.md"), &b.join("nota.md")).unwrap();
        assert_eq!(nuevo.as_deref(), Some("![](assets/solo-2.png) ![](assets/comun.png)"));
        assert!(!a.join("assets/solo.png").exists(), "el exclusivo se mueve");
        assert!(a.join("assets/comun.png").exists(), "el compartido se queda");
        assert_eq!(fs::read_to_string(b.join("assets/solo-2.png")).unwrap(), "1");
        assert_eq!(fs::read_to_string(b.join("assets/comun.png")).unwrap(), "2");

        let h = huerfanos_al_borrar("![](assets/comun.png) ![](assets/x.png)", &a.join("nota.md"));
        assert!(h.is_empty(), "comun lo usa otra nota y x no existe: {h:?}");
        fs::remove_file(a.join("otra.md")).unwrap();
        assert_eq!(huerfanos_al_borrar("![](assets/comun.png)", &a.join("nota.md")), [a.join("assets/comun.png")]);
        fs::remove_dir_all(&base).unwrap();
    }
}
