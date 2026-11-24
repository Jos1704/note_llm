//! Plantillas de notas: archivos `.md` en `.plantillas/`, en la raíz de cada lugar (para todos los
//! proyectos) o dentro de un proyecto (solo para ese proyecto; tiene prioridad).
//!
//! El front matter de la plantilla puede traer `title` y `tags`. Variables (se rellenan en la app):
//! `{{fecha}}`, `{{hora}}`, `{{proyecto}}`, `{{titulo}}` y `{{cursor}}` (dónde queda el cursor).

use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::frontmatter;
use crate::store;

pub const CARPETA: &str = ".plantillas";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plantilla {
    pub nombre: String,
    /// `proyecto`, `equipo` o `disco`.
    pub origen: String,
    pub titulo: Option<String>,
    pub etiquetas: Vec<String>,
    pub cuerpo: String,
}

/// Plantillas de una carpeta `.plantillas` (vacío si no existe).
pub fn leer_dir(dir: &Path, origen: &str) -> Vec<Plantilla> {
    let Ok(entradas) = fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<Plantilla> = entradas
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let n = p.file_name().unwrap_or_default().to_string_lossy();
            !n.starts_with('.') && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("md"))
        })
        .filter_map(|p| {
            let texto = fs::read_to_string(&p).ok()?;
            let doc = frontmatter::parsear(&texto);
            Some(Plantilla {
                nombre: p.file_stem()?.to_string_lossy().to_string(),
                origen: origen.to_string(),
                titulo: doc.titulo.filter(|t| !t.trim().is_empty()),
                etiquetas: doc.etiquetas,
                cuerpo: doc.cuerpo,
            })
        })
        .collect();
    out.sort_by(|a, b| a.nombre.to_lowercase().cmp(&b.nombre.to_lowercase()));
    out
}

const EJEMPLOS: &[(&str, &str)] = &[
    (
        "Reunión",
        "---\ntitle: \"Reunión {{fecha}}\"\ntags: [reunion]\n---\n**Fecha:** {{fecha}} {{hora}}\n**Asistentes:** \n\n## Temas\n\n- {{cursor}}\n\n## Acuerdos\n\n- \n\n## Pendientes\n\n- [ ] \n",
    ),
    (
        "Bug",
        "---\ntags: [bug]\n---\n## Qué pasa\n\n{{cursor}}\n\n## Pasos para reproducirlo\n\n1. \n\n## Qué debería pasar\n\n\n## Entorno\n\n- Versión: \n- Sistema: \n",
    ),
    (
        "Idea",
        "---\ntags: [idea]\n---\n## La idea\n\n{{cursor}}\n\n## Por qué importa\n\n\n## Siguiente paso\n\n- [ ] \n",
    ),
];

/// Crea `.plantillas/` con ejemplos si no existe todavía (solo la primera vez).
pub fn sembrar(raiz: &Path) -> Result<()> {
    let dir = raiz.join(CARPETA);
    if dir.exists() {
        return Ok(());
    }
    fs::create_dir_all(&dir)?;
    for (nombre, texto) in EJEMPLOS {
        store::escribir(&dir.join(format!("{nombre}.md")), texto)?;
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn siembra_una_vez_y_lee_front_matter() {
        let base = std::env::temp_dir().join(format!("md-notes-prueba-plantillas-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        sembrar(&base).unwrap();
        let ps = leer_dir(&base.join(CARPETA), "equipo");
        let nombres: Vec<&str> = ps.iter().map(|p| p.nombre.as_str()).collect();
        assert_eq!(nombres, ["Bug", "Idea", "Reunión"]);
        let reunion = ps.iter().find(|p| p.nombre == "Reunión").unwrap();
        assert_eq!(reunion.titulo.as_deref(), Some("Reunión {{fecha}}"));
        assert_eq!(reunion.etiquetas, ["reunion"]);
        assert!(reunion.cuerpo.starts_with("**Fecha:**"));
        // Si el usuario borra una plantilla, no se vuelve a crear.
        fs::remove_file(base.join(CARPETA).join("Bug.md")).unwrap();
        sembrar(&base).unwrap();
        assert_eq!(leer_dir(&base.join(CARPETA), "equipo").len(), 2);
        fs::remove_dir_all(&base).unwrap();
    }
}
