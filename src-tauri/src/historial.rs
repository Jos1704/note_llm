//! Historial de versiones de las notas, sin git (así funciona igual en un disco exFAT).
//!
//! Las instantáneas son copias del archivo en `<raíz del lugar>/.historial/<proyecto>/<nota>.md/
//! <AAAAMMDD-HHMMSS>.md`. Se toman:
//! - al guardar, con el contenido *anterior*, si la última instantánea tiene más de 10 minutos;
//! - al cerrar la nota o cambiar de nota (`instantanea`), si el texto cambió desde la última;
//! - antes de restaurar una versión (para poder deshacerlo).
//!
//! Se conservan todas las de las últimas 24 h, una por día durante 30 días y una por semana
//! después. Si el texto es igual al de la última instantánea, no se duplica.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Datelike, Local, NaiveDateTime, TimeZone};
use serde::Serialize;

pub const CARPETA: &str = ".historial";
const FORMATO: &str = "%Y%m%d-%H%M%S";
/// Tiempo mínimo entre instantáneas automáticas al guardar.
pub const INTERVALO_SEG: i64 = 10 * 60;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    /// Nombre del archivo (`AAAAMMDD-HHMMSS.md`): identifica la versión.
    pub nombre: String,
    /// Milisegundos desde epoch.
    pub fecha: i64,
    pub bytes: u64,
}

/// Carpeta del historial de la nota `rel` (ruta relativa a la raíz del lugar).
pub fn carpeta(raiz: &Path, rel: &str) -> PathBuf {
    raiz.join(CARPETA).join(rel)
}

fn fecha_de(nombre: &str) -> Option<DateTime<Local>> {
    let base = nombre.strip_suffix(".md")?;
    let base = base.split_once('~').map_or(base, |(b, _)| b);
    let n = NaiveDateTime::parse_from_str(base, FORMATO).ok()?;
    Local.from_local_datetime(&n).single()
}

/// Versiones guardadas, de la más reciente a la más antigua.
pub fn versiones(dir: &Path) -> Vec<Version> {
    let Ok(entradas) = fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<Version> = entradas
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let nombre = e.file_name().to_string_lossy().to_string();
            let fecha = fecha_de(&nombre)?;
            Some(Version { nombre, fecha: fecha.timestamp_millis(), bytes: e.metadata().map(|m| m.len()).unwrap_or(0) })
        })
        .collect();
    out.sort_by(|a, b| b.nombre.cmp(&a.nombre));
    out
}

pub fn leer(dir: &Path, nombre: &str) -> Result<String> {
    if fecha_de(nombre).is_none() || nombre.contains(['/', '\\']) {
        bail!("versión no válida: {nombre}");
    }
    fs::read_to_string(dir.join(nombre)).with_context(|| format!("leyendo la versión {nombre}"))
}

/// Guarda `texto` como instantánea si es distinto de la última y, cuando `intervalo` es
/// `Some(s)`, solo si la última tiene más de `s` segundos. Devuelve si se guardó.
pub fn instantanea(dir: &Path, texto: &str, ahora: DateTime<Local>, intervalo: Option<i64>) -> Result<bool> {
    if texto.trim().is_empty() {
        return Ok(false);
    }
    let lista = versiones(dir);
    if let Some(ultima) = lista.first() {
        if let Some(s) = intervalo
            && ahora.timestamp_millis() - ultima.fecha < s * 1000
        {
            return Ok(false);
        }
        if fs::read_to_string(dir.join(&ultima.nombre)).ok().as_deref() == Some(texto) {
            return Ok(false);
        }
    }
    fs::create_dir_all(dir)?;
    let mut nombre = format!("{}.md", ahora.format(FORMATO));
    let mut n = 2;
    while dir.join(&nombre).exists() {
        nombre = format!("{}~{n}.md", ahora.format(FORMATO));
        n += 1;
    }
    fs::write(dir.join(&nombre), texto)?;
    podar(dir, ahora);
    Ok(true)
}

/// Rotación: todas las de 24 h, una por día hasta 30 días y una por semana después
/// (se queda la más reciente de cada día o semana).
pub fn podar(dir: &Path, ahora: DateTime<Local>) {
    let mut vistos = std::collections::HashSet::new();
    for v in versiones(dir) {
        let Some(f) = fecha_de(&v.nombre) else { continue };
        let edad = ahora.signed_duration_since(f);
        let grupo = if edad.num_hours() < 24 {
            continue;
        } else if edad.num_days() < 30 {
            format!("d{}", f.format("%Y%m%d"))
        } else {
            let s = f.iso_week();
            format!("s{}-{}", s.year(), s.week())
        };
        if !vistos.insert(grupo) {
            let _ = fs::remove_file(dir.join(&v.nombre));
        }
    }
}

/// Mueve el historial cuando la nota cambia de archivo (renombrar o mover de proyecto).
pub fn trasladar(desde: &Path, hasta: &Path) {
    if desde == hasta || !desde.is_dir() {
        return;
    }
    if let Some(p) = hasta.parent() {
        let _ = fs::create_dir_all(p);
    }
    if !hasta.exists() && fs::rename(desde, hasta).is_ok() {
        return;
    }
    // Entre volúmenes o si el destino ya tenía historial: copiar archivo a archivo.
    let _ = fs::create_dir_all(hasta);
    if let Ok(entradas) = fs::read_dir(desde) {
        for e in entradas.filter_map(|e| e.ok()) {
            let destino = hasta.join(e.file_name());
            if !destino.exists() {
                let _ = fs::copy(e.path(), destino);
            }
        }
    }
    let _ = fs::remove_dir_all(desde);
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use chrono::Duration;

    fn dir(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("md-notes-historial-{nombre}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn respeta_intervalo_y_no_duplica() {
        let d = dir("intervalo");
        let t0 = Local.with_ymd_and_hms(2026, 11, 20, 10, 0, 0).unwrap();
        assert!(instantanea(&d, "uno", t0, Some(INTERVALO_SEG)).unwrap());
        assert!(!instantanea(&d, "dos", t0 + Duration::minutes(5), Some(INTERVALO_SEG)).unwrap(), "muy pronto");
        assert!(!instantanea(&d, "uno", t0 + Duration::minutes(5), None).unwrap(), "mismo texto");
        assert!(instantanea(&d, "dos", t0 + Duration::minutes(5), None).unwrap(), "al cerrar, sin intervalo");
        assert!(instantanea(&d, "tres", t0 + Duration::minutes(16), Some(INTERVALO_SEG)).unwrap());
        let v = versiones(&d);
        assert_eq!(v.len(), 3);
        assert_eq!(leer(&d, &v[0].nombre).unwrap(), "tres");
        assert!(leer(&d, "../../secreto.md").is_err());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn rotacion() {
        let d = dir("rotacion");
        fs::create_dir_all(&d).unwrap();
        let ahora = Local.with_ymd_and_hms(2026, 11, 23, 12, 0, 0).unwrap();
        let poner = |f: DateTime<Local>| fs::write(d.join(format!("{}.md", f.format(FORMATO))), "x").unwrap();
        // Hoy: 3 (se quedan todas).
        for h in [1, 2, 3] {
            poner(ahora - Duration::hours(h));
        }
        // Hace 5 días: 3 el mismo día (queda 1).
        for h in [0, 2, 4] {
            poner(Local.with_ymd_and_hms(2026, 11, 18, 9 + h, 0, 0).unwrap());
        }
        // Hace ~2 meses: 3 en la misma semana ISO (lun 21–mié 23 sep), queda 1.
        for dia in [21, 22, 23] {
            poner(Local.with_ymd_and_hms(2026, 9, dia, 10, 0, 0).unwrap());
        }
        podar(&d, ahora);
        let nombres: Vec<String> = versiones(&d).into_iter().map(|v| v.nombre).collect();
        assert_eq!(nombres.len(), 5, "{nombres:?}");
        assert!(nombres.contains(&"20261118-130000.md".to_string()), "la más reciente del día");
        assert!(nombres.contains(&"20260923-100000.md".to_string()), "la más reciente de la semana");
        fs::remove_dir_all(&d).unwrap();
    }
}
