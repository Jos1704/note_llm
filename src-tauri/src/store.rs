//! Una carpeta de notas («lugar»): la de este equipo o la del disco externo.
//!
//! ```text
//! Notas/
//! ├── .md-notes                 marca la carpeta como biblioteca
//! ├── .etiquetas.toml           colores de las etiquetas
//! ├── .papelera/                notas y proyectos eliminados
//! └── <proyecto>/               un proyecto = una carpeta
//!     ├── .proyecto.toml        nombre visible del proyecto
//!     └── <nota>.md             front matter (title, tags) + Markdown
//! ```
//!
//! Los ids llevan el lugar delante: `disco:trabajo/mi-nota.md`, `equipo:trabajo`.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::frontmatter::{self, Documento};

/// Valores admitidos en el front matter. «Automática» y «lisa» no se escriben (son el valor por defecto).
const HOJAS: [&str; 3] = ["blanca", "amarilla", "negra"];
const PAUTAS: [&str; 3] = ["rayas", "puntos", "cuadricula"];

pub const MARCADOR: &str = ".md-notes";
const ETIQUETAS_FILE: &str = ".etiquetas.toml";
const PROYECTO_FILE: &str = ".proyecto.toml";
const PAPELERA: &str = ".papelera";
const BANDEJA: &str = "Bandeja";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lugar {
    Equipo,
    Disco,
}

impl Lugar {
    pub fn prefijo(self) -> &'static str {
        match self {
            Lugar::Equipo => "equipo",
            Lugar::Disco => "disco",
        }
    }

    /// Lugar de un id (`disco:…`).
    pub fn de_id(id: &str) -> Result<Self> {
        match id.split_once(':').map(|(l, _)| l) {
            Some("equipo") => Ok(Lugar::Equipo),
            Some("disco") => Ok(Lugar::Disco),
            _ => bail!("id sin lugar: {id}"),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Proyecto {
    pub id: String,
    pub lugar: Lugar,
    pub nombre: String,
    pub notas: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotaResumen {
    pub id: String,
    pub proyecto: String,
    pub lugar: Lugar,
    pub titulo: String,
    pub etiquetas: Vec<String>,
    /// Milisegundos desde epoch.
    pub modificado: u64,
    pub extracto: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Nota {
    #[serde(flatten)]
    pub resumen: NotaResumen,
    pub cuerpo: String,
    pub hoja: Option<String>,
    pub pauta: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct MetaProyecto {
    nombre: String,
}

pub struct Store {
    raiz: PathBuf,
    lugar: Lugar,
}

impl Store {
    /// Usa (creando si hace falta) la carpeta `raiz` como biblioteca.
    pub fn crear_en(raiz: PathBuf, lugar: Lugar) -> Result<Self> {
        fs::create_dir_all(&raiz).with_context(|| format!("creando {}", raiz.display()))?;
        let marcador = raiz.join(MARCADOR);
        if !marcador.exists() {
            fs::write(&marcador, "Carpeta de notas de md-notes.\n")?;
        }
        Ok(Self { raiz, lugar })
    }

    /// Abre una biblioteca existente sin crear nada (para el disco, que puede no estar montado).
    pub fn existente(raiz: PathBuf, lugar: Lugar) -> Option<Self> {
        raiz.is_dir().then_some(Self { raiz, lugar })
    }

    pub fn raiz(&self) -> &Path {
        &self.raiz
    }

    pub fn lugar(&self) -> Lugar {
        self.lugar
    }

    // ---------------------------------------------------------------- lectura

    pub fn proyectos_y_notas(&self) -> Result<(Vec<Proyecto>, Vec<NotaResumen>)> {
        let mut notas = Vec::new();
        let mut proyectos = Vec::new();
        for dir in self.dirs_proyecto()? {
            let antes = notas.len();
            for path in archivos_md(&dir)? {
                match self.leer(&path) {
                    Ok(n) => notas.push(n.resumen),
                    Err(e) => eprintln!("omitiendo {}: {e:#}", path.display()),
                }
            }
            let carpeta = nombre_archivo(&dir);
            proyectos.push(Proyecto {
                id: self.id_de(&dir),
                lugar: self.lugar,
                nombre: self.nombre_proyecto(&carpeta),
                notas: notas.len() - antes,
            });
        }
        Ok((proyectos, notas))
    }

    pub fn leer_nota(&self, id: &str) -> Result<Nota> {
        self.leer(&self.ruta(id)?)
    }

    /// Ids de las notas que contienen `aguja` (ya en minúsculas).
    pub fn buscar(&self, aguja: &str) -> Result<Vec<String>> {
        let mut ids = Vec::new();
        for dir in self.dirs_proyecto()? {
            for path in archivos_md(&dir)? {
                if let Ok(contenido) = fs::read_to_string(&path)
                    && contenido.to_lowercase().contains(aguja)
                {
                    ids.push(self.id_de(&path));
                }
            }
        }
        Ok(ids)
    }

    // ---------------------------------------------------------------- notas

    pub fn crear_nota(&self, proyecto: Option<&str>, etiquetas: &[String]) -> Result<Nota> {
        let dir = match proyecto {
            Some(p) => self.ruta(p)?,
            None => self.bandeja()?,
        };
        if !dir.is_dir() {
            bail!("el proyecto no existe");
        }
        let path = libre(&dir, "sin-titulo", None);
        escribir(&path, &frontmatter::serializar("Sin título", &normalizar_etiquetas(etiquetas), None, None, &[], ""))?;
        self.leer(&path)
    }

    /// Guarda la nota. Si cambió el título, renombra el archivo; el id devuelto puede cambiar.
    pub fn guardar_nota(&self, id: &str, titulo: &str, etiquetas: &[String], hoja: Option<&str>, pauta: Option<&str>, cuerpo: &str) -> Result<NotaResumen> {
        let path = self.ruta(id)?;
        let texto = fs::read_to_string(&path).with_context(|| format!("leyendo {id}"))?;
        let previo = frontmatter::parsear(&texto);
        let titulo = titulo.trim();
        let etiquetas = normalizar_etiquetas(etiquetas);

        let base = slug(titulo);
        let base = if base.is_empty() { "sin-titulo".to_string() } else { base };
        let destino = libre(path.parent().unwrap(), &base, Some(&path));

        let hoja = hoja.filter(|h| HOJAS.contains(h));
        let pauta = pauta.filter(|p| PAUTAS.contains(p));
        escribir(&destino, &frontmatter::serializar(titulo, &etiquetas, hoja, pauta, &previo.extra, cuerpo))?;
        if destino != path {
            fs::remove_file(&path)?;
        }
        Ok(self.leer(&destino)?.resumen)
    }

    /// Copia el archivo de una nota (de este u otro lugar) al proyecto `proyecto` de este lugar.
    pub fn recibir_nota(&self, origen: &Path, proyecto: &str) -> Result<NotaResumen> {
        let dir = self.ruta(proyecto)?;
        if !dir.is_dir() {
            bail!("el proyecto no existe");
        }
        let stem = origen.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let destino = libre(&dir, &stem, None);
        escribir(&destino, &fs::read_to_string(origen)?)?;
        Ok(self.leer(&destino)?.resumen)
    }

    pub fn borrar(&self, id: &str) -> Result<()> {
        let path = self.ruta(id)?;
        if !path.exists() {
            bail!("no existe: {id}");
        }
        self.a_papelera(&path)
    }

    // ---------------------------------------------------------------- proyectos

    pub fn crear_proyecto(&self, nombre: &str) -> Result<String> {
        let nombre = nombre.trim();
        let base = slug(nombre);
        if base.is_empty() {
            bail!("el nombre del proyecto no es válido");
        }
        let dir = carpeta_libre(&self.raiz, &base);
        fs::create_dir_all(&dir)?;
        self.guardar_meta_proyecto(&dir, nombre)?;
        Ok(self.id_de(&dir))
    }

    pub fn renombrar_proyecto(&self, id: &str, nombre: &str) -> Result<()> {
        let nombre = nombre.trim();
        if nombre.is_empty() {
            bail!("el nombre no puede estar vacío");
        }
        let dir = self.ruta(id)?;
        if !dir.is_dir() {
            bail!("el proyecto no existe");
        }
        self.guardar_meta_proyecto(&dir, nombre)
    }

    /// Copia la carpeta de un proyecto (de otro lugar) aquí. Devuelve el nuevo id.
    pub fn recibir_proyecto(&self, origen: &Path) -> Result<String> {
        let destino = carpeta_libre(&self.raiz, &nombre_archivo(origen));
        copiar_dir(origen, &destino)?;
        Ok(self.id_de(&destino))
    }

    // ---------------------------------------------------------------- etiquetas

    pub fn colores(&self) -> Result<BTreeMap<String, String>> {
        let path = self.raiz.join(ETIQUETAS_FILE);
        if !path.is_file() {
            return Ok(BTreeMap::new());
        }
        toml::from_str(&fs::read_to_string(&path)?).with_context(|| format!("formato inválido en {}", path.display()))
    }

    pub fn guardar_colores(&self, colores: &BTreeMap<String, String>) -> Result<()> {
        escribir(&self.raiz.join(ETIQUETAS_FILE), &toml::to_string(colores)?)
    }

    /// Aplica `f` a las etiquetas de cada nota y reescribe las que cambian.
    pub fn reescribir_etiquetas(&self, f: &dyn Fn(&mut Vec<String>)) -> Result<()> {
        for dir in self.dirs_proyecto()? {
            for path in archivos_md(&dir)? {
                let doc = frontmatter::parsear(&fs::read_to_string(&path)?);
                let antes = normalizar_etiquetas(&doc.etiquetas);
                let mut etiquetas = antes.clone();
                f(&mut etiquetas);
                let etiquetas = normalizar_etiquetas(&etiquetas);
                if etiquetas != antes {
                    let titulo = match doc.titulo.clone() {
                        Some(t) => t,
                        None => self.leer(&path)?.resumen.titulo,
                    };
                    escribir(&path, &frontmatter::serializar(&titulo, &etiquetas, doc.hoja.as_deref(), doc.pauta.as_deref(), &doc.extra, &doc.cuerpo))?;
                }
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------- utilidades

    /// Convierte un id de este lugar en ruta absoluta, rechazando rutas fuera de la raíz.
    pub fn ruta(&self, id: &str) -> Result<PathBuf> {
        let rel = id
            .strip_prefix(self.lugar.prefijo())
            .and_then(|r| r.strip_prefix(':'))
            .with_context(|| format!("el id no es de este lugar: {id}"))?;
        let rel_path = Path::new(rel);
        let valida = !rel.is_empty() && rel_path.components().all(|c| matches!(c, Component::Normal(_)));
        if !valida {
            bail!("ruta no válida: {id}");
        }
        Ok(self.raiz.join(rel_path))
    }

    fn id_de(&self, path: &Path) -> String {
        let rel = path.strip_prefix(&self.raiz).unwrap_or(path);
        let rel = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
        format!("{}:{rel}", self.lugar.prefijo())
    }

    fn leer(&self, path: &Path) -> Result<Nota> {
        let texto = fs::read_to_string(path).with_context(|| format!("leyendo {}", path.display()))?;
        let Documento { titulo, etiquetas, hoja, pauta, cuerpo, .. } = frontmatter::parsear(&texto);
        let titulo = titulo
            .filter(|t| !t.trim().is_empty())
            .or_else(|| primer_titulo(&cuerpo))
            .unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().to_string());
        let modificado = fs::metadata(path)?
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_millis() as u64);
        let id = self.id_de(path);
        let carpeta = path
            .strip_prefix(&self.raiz)
            .ok()
            .and_then(|r| r.components().next())
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .unwrap_or_default();
        Ok(Nota {
            resumen: NotaResumen {
                proyecto: format!("{}:{carpeta}", self.lugar.prefijo()),
                lugar: self.lugar,
                id,
                titulo,
                etiquetas: normalizar_etiquetas(&etiquetas),
                modificado,
                extracto: extracto(&cuerpo),
            },
            cuerpo,
            hoja,
            pauta,
        })
    }

    fn dirs_proyecto(&self) -> Result<Vec<PathBuf>> {
        let mut dirs = Vec::new();
        for entrada in fs::read_dir(&self.raiz).with_context(|| format!("leyendo {}", self.raiz.display()))? {
            let path = entrada?.path();
            if path.is_dir() && !oculto(&path) {
                dirs.push(path);
            }
        }
        Ok(dirs)
    }

    fn nombre_proyecto(&self, carpeta: &str) -> String {
        fs::read_to_string(self.raiz.join(carpeta).join(PROYECTO_FILE))
            .ok()
            .and_then(|t| toml::from_str::<MetaProyecto>(&t).ok())
            .map(|m| m.nombre)
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| carpeta.to_string())
    }

    fn guardar_meta_proyecto(&self, dir: &Path, nombre: &str) -> Result<()> {
        let meta = MetaProyecto { nombre: nombre.to_string() };
        escribir(&dir.join(PROYECTO_FILE), &toml::to_string(&meta)?)
    }

    fn bandeja(&self) -> Result<PathBuf> {
        let dir = self.raiz.join(slug(BANDEJA));
        if !dir.is_dir() {
            fs::create_dir_all(&dir)?;
            self.guardar_meta_proyecto(&dir, BANDEJA)?;
        }
        Ok(dir)
    }

    fn a_papelera(&self, path: &Path) -> Result<()> {
        let papelera = self.raiz.join(PAPELERA);
        fs::create_dir_all(&papelera)?;
        let marca = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let destino = papelera.join(format!("{marca}-{}", nombre_archivo(path)));
        fs::rename(path, &destino).with_context(|| format!("moviendo {} a la papelera", path.display()))
    }
}

// -------------------------------------------------------------------- funciones libres

fn archivos_md(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    recolectar_md(dir, &mut out)?;
    Ok(out)
}

fn recolectar_md(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entrada in fs::read_dir(dir).with_context(|| format!("leyendo {}", dir.display()))? {
        let path = entrada?.path();
        // Ocultos incluye `._*` y `.DS_Store` que macOS crea en discos exFAT.
        if oculto(&path) {
            continue;
        }
        if path.is_dir() {
            recolectar_md(&path, out)?;
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("md")) {
            out.push(path);
        }
    }
    Ok(())
}

fn oculto(path: &Path) -> bool {
    path.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'))
}

fn nombre_archivo(path: &Path) -> String {
    path.file_name().unwrap_or_default().to_string_lossy().to_string()
}

/// Escritura atómica y sincronizada (temporal oculto + rename), para que
/// desconectar el disco externo no deje archivos a medias.
pub fn escribir(path: &Path, contenido: &str) -> Result<()> {
    let tmp = path.with_file_name(format!(".{}.tmp", nombre_archivo(path)));
    {
        let mut f = fs::File::create(&tmp).with_context(|| format!("escribiendo {}", tmp.display()))?;
        f.write_all(contenido.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path).with_context(|| format!("guardando {}", path.display()))
}

/// Primera ruta `<dir>/<base>.md`, `<base>-2.md`… que esté libre (o que sea `actual`).
fn libre(dir: &Path, base: &str, actual: Option<&Path>) -> PathBuf {
    let mut n = 1;
    loop {
        let nombre = if n == 1 { format!("{base}.md") } else { format!("{base}-{n}.md") };
        let path = dir.join(nombre);
        if Some(path.as_path()) == actual || !path.exists() {
            return path;
        }
        n += 1;
    }
}

fn carpeta_libre(raiz: &Path, base: &str) -> PathBuf {
    let mut dir = raiz.join(base);
    let mut n = 2;
    while dir.exists() {
        dir = raiz.join(format!("{base}-{n}"));
        n += 1;
    }
    dir
}

pub fn copiar_dir(origen: &Path, destino: &Path) -> Result<()> {
    fs::create_dir_all(destino)?;
    for entrada in fs::read_dir(origen)? {
        let entrada = entrada?;
        let path = entrada.path();
        let nombre = entrada.file_name();
        if nombre.to_string_lossy().starts_with("._") || nombre == ".DS_Store" {
            continue;
        }
        let dest = destino.join(&nombre);
        if entrada.file_type()?.is_dir() {
            copiar_dir(&path, &dest)?;
        } else {
            fs::copy(&path, &dest).with_context(|| format!("copiando {}", path.display()))?;
        }
    }
    Ok(())
}

/// Nombre de archivo/carpeta en ASCII: evita problemas de acentos entre macOS y Linux.
pub fn slug(texto: &str) -> String {
    let mut out = String::new();
    for c in texto.trim().chars().flat_map(char::to_lowercase) {
        let c = match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
        if out.len() >= 80 {
            break;
        }
    }
    out.trim_end_matches('-').to_string()
}

pub fn normalizar_etiqueta(e: &str) -> String {
    e.trim()
        .trim_start_matches('#')
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .filter(|c| !matches!(c, ',' | '[' | ']' | '{' | '}' | '"' | '\'' | ':' | '#'))
        .collect()
}

pub fn normalizar_etiquetas(etiquetas: &[String]) -> Vec<String> {
    let mut out: Vec<String> = etiquetas.iter().map(|e| normalizar_etiqueta(e)).filter(|e| !e.is_empty()).collect();
    out.sort();
    out.dedup();
    out
}

pub fn primer_titulo(cuerpo: &str) -> Option<String> {
    cuerpo
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Primeras palabras del cuerpo sin la sintaxis Markdown más común.
fn extracto(cuerpo: &str) -> String {
    let mut out = String::new();
    let mut en_codigo = false;
    for linea in cuerpo.lines() {
        let l = linea.trim();
        if l.starts_with("```") {
            en_codigo = !en_codigo;
            continue;
        }
        if en_codigo || l.is_empty() || l.starts_with('#') || l.starts_with("---") || l.starts_with('|') {
            continue;
        }
        let l = l.trim_start_matches(['>', '-', '*', '+', ' ']);
        let l = l.strip_prefix("[ ] ").or_else(|| l.strip_prefix("[x] ")).unwrap_or(l);
        // Marcador de alerta de GitHub (`> [!NOTE]`): no aporta al extracto.
        if l.starts_with("[!") && l.ends_with(']') {
            continue;
        }
        let limpio: String = l.replace("==", "").chars().filter(|c| !matches!(c, '*' | '_' | '`')).collect();
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&limpio);
        if out.chars().count() > 160 {
            break;
        }
    }
    out.chars().take(160).collect()
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn dir_temporal(nombre: &str) -> PathBuf {
        let raiz = std::env::temp_dir().join(format!("md-notes-{nombre}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&raiz);
        raiz
    }

    #[test]
    fn slug_y_etiquetas() {
        assert_eq!(slug("Diseño de la API: v2!"), "diseno-de-la-api-v2");
        assert_eq!(normalizar_etiquetas(&["#Rust".into(), "mi tag".into(), "rust".into()]), vec!["mi-tag", "rust"]);
    }

    #[test]
    fn ruta_valida_lugar_y_raiz() {
        let s = Store::crear_en(dir_temporal("ruta"), Lugar::Disco).unwrap();
        assert!(s.ruta("disco:../etc/passwd").is_err());
        assert!(s.ruta("disco:/etc/passwd").is_err());
        assert!(s.ruta("equipo:a/b.md").is_err(), "id de otro lugar");
        assert!(s.ruta("disco:a/b.md").is_ok());
        fs::remove_dir_all(s.raiz()).unwrap();
    }

    #[test]
    fn notas_en_un_lugar() {
        let s = Store::crear_en(dir_temporal("notas"), Lugar::Equipo).unwrap();
        let p = s.crear_proyecto("Trabajo").unwrap();
        assert_eq!(p, "equipo:trabajo");

        let n = s.crear_nota(Some(&p), &["Rust".into()]).unwrap();
        assert_eq!(n.resumen.id, "equipo:trabajo/sin-titulo.md");
        assert_eq!(n.resumen.proyecto, p);

        let r = s.guardar_nota(&n.resumen.id, "Diseño API", &["rust".into()], None, None, "Hola **mundo**").unwrap();
        assert_eq!(r.id, "equipo:trabajo/diseno-api.md");
        assert!(!s.raiz().join("trabajo/sin-titulo.md").exists());
        assert_eq!(r.extracto, "Hola mundo");
        assert_eq!(s.guardar_nota(&r.id, "Diseño API", &[], None, None, "otro").unwrap().id, r.id, "mismo título no renombra");

        let suelta = s.crear_nota(None, &[]).unwrap();
        assert_eq!(suelta.resumen.proyecto, "equipo:bandeja");

        assert_eq!(s.buscar("otro").unwrap(), vec![r.id.clone()]);
        s.borrar(&r.id).unwrap();
        assert_eq!(fs::read_dir(s.raiz().join(PAPELERA)).unwrap().count(), 1);
        fs::remove_dir_all(s.raiz()).unwrap();
    }

    #[test]
    fn hoja_y_pauta_se_guardan_y_se_validan() {
        let s = Store::crear_en(dir_temporal("hoja"), Lugar::Equipo).unwrap();
        let n = s.crear_nota(None, &[]).unwrap();
        let r = s.guardar_nota(&n.resumen.id, "Papel", &[], Some("amarilla"), Some("cuadricula"), "x").unwrap();
        let leida = s.leer(&s.ruta(&r.id).unwrap()).unwrap();
        assert_eq!(leida.hoja.as_deref(), Some("amarilla"));
        assert_eq!(leida.pauta.as_deref(), Some("cuadricula"));

        // Valores desconocidos (o los de por defecto) no se escriben.
        let r = s.guardar_nota(&r.id, "Papel", &[], Some("morada"), Some("ninguna"), "x").unwrap();
        let leida = s.leer(&s.ruta(&r.id).unwrap()).unwrap();
        assert_eq!(leida.hoja, None);
        assert_eq!(leida.pauta, None);
        fs::remove_dir_all(s.raiz()).unwrap();
    }
}
