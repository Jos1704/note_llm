//! Los dos lugares donde viven las notas, como en Syntara:
//!
//! - **Este equipo** (siempre): `~/Notas` (o `$MD_NOTES_DIR`).
//! - **Disco** (opcional): una carpeta del disco externo que viaja entre la Mac y Linux.
//!   Su ruta se recuerda en los ajustes de cada equipo y solo se usa si está montada;
//!   nunca se crea nada en esa ruta si el disco no está conectado.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::store::{
    self, Lugar, MARCADOR, Nota, NotaResumen, Proyecto, Store, normalizar_etiqueta, normalizar_etiquetas,
};

pub const PALETA: &[&str] = &[
    "#6e9c84", "#6d93ba", "#8d80bf", "#bc9a55", "#c98468",
    "#8a9799", "#5e9c9a", "#a57fa0", "#9aa35e", "#c77d8e",
];
const COLOR_SIN_REGISTRO: &str = "#8a9799";

#[derive(Serialize, Deserialize, Default)]
struct Ajustes {
    /// Carpeta de notas del disco externo en este equipo.
    disco: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Etiqueta {
    pub nombre: String,
    pub color: String,
    pub notas: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoDisco {
    pub ruta: String,
    pub conectado: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Estado {
    pub equipo: String,
    pub disco: Option<InfoDisco>,
    pub proyectos: Vec<Proyecto>,
    pub etiquetas: Vec<Etiqueta>,
    pub notas: Vec<NotaResumen>,
    pub paleta: Vec<String>,
}

pub struct Biblioteca {
    equipo: Store,
    disco: Option<PathBuf>,
    ajustes: PathBuf,
}

impl Biblioteca {
    pub fn abrir() -> Result<Self> {
        let equipo = match std::env::var_os("MD_NOTES_DIR") {
            Some(d) => PathBuf::from(d),
            None => dirs::home_dir().context("no se encontró la carpeta personal")?.join("Notas"),
        };
        let ajustes = match std::env::var_os("MD_NOTES_CONFIG") {
            Some(d) => PathBuf::from(d),
            None => dirs::config_dir().context("no se encontró la carpeta de configuración")?.join("md-notes"),
        }
        .join("ajustes.json");
        Self::con_rutas(equipo, ajustes)
    }

    pub(crate) fn con_rutas(equipo: PathBuf, ajustes: PathBuf) -> Result<Self> {
        let guardado: Ajustes = fs::read_to_string(&ajustes)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let disco = std::env::var_os("MD_NOTES_DISCO").map(PathBuf::from).or(guardado.disco);
        Ok(Self { equipo: Store::crear_en(equipo, Lugar::Equipo)?, disco, ajustes })
    }

    // ---------------------------------------------------------------- lugares

    fn disco(&self) -> Option<Store> {
        Store::existente(self.disco.clone()?, Lugar::Disco)
    }

    fn lugar(&self, lugar: Lugar) -> Result<Store> {
        match lugar {
            Lugar::Equipo => Ok(Store::existente(self.equipo.raiz().to_path_buf(), Lugar::Equipo)
                .context("la carpeta de este equipo no existe")?),
            Lugar::Disco => self.disco().context("el disco no está conectado"),
        }
    }

    fn de(&self, id: &str) -> Result<Store> {
        self.lugar(Lugar::de_id(id)?)
    }

    fn disponibles(&self) -> Vec<Store> {
        [Lugar::Equipo, Lugar::Disco].into_iter().filter_map(|l| self.lugar(l).ok()).collect()
    }

    /// Ruta de un id (o la raíz de su lugar) para abrirla en el gestor de archivos.
    pub fn ruta(&self, id: Option<&str>, lugar: Lugar) -> Result<PathBuf> {
        match id {
            Some(id) => self.de(id)?.ruta(id),
            None => Ok(self.lugar(lugar)?.raiz().to_path_buf()),
        }
    }

    /// Usa `carpeta` como lugar «Disco». Si no es ya una biblioteca, se usa `<carpeta>/Notas`
    /// para no mezclar las notas con lo demás que haya en el disco.
    pub fn elegir_disco(&mut self, carpeta: &Path) -> Result<String> {
        let raiz = if carpeta.join(MARCADOR).is_file() { carpeta.to_path_buf() } else { carpeta.join("Notas") };
        if mismo_dir(&raiz, self.equipo.raiz()) {
            bail!("esa carpeta ya es la de este equipo");
        }
        Store::crear_en(raiz.clone(), Lugar::Disco)?;
        self.disco = Some(raiz.clone());
        self.guardar_ajustes()?;
        Ok(mostrar(&raiz))
    }

    pub fn olvidar_disco(&mut self) -> Result<()> {
        self.disco = None;
        self.guardar_ajustes()
    }

    fn guardar_ajustes(&self) -> Result<()> {
        if let Some(dir) = self.ajustes.parent() {
            fs::create_dir_all(dir)?;
        }
        let ajustes = Ajustes { disco: self.disco.clone() };
        store::escribir(&self.ajustes, &serde_json::to_string_pretty(&ajustes)?)
    }

    // ---------------------------------------------------------------- lectura

    pub fn estado(&self) -> Result<Estado> {
        let mut proyectos = Vec::new();
        let mut notas = Vec::new();
        for s in self.disponibles() {
            let (p, n) = s.proyectos_y_notas()?;
            proyectos.extend(p);
            notas.extend(n);
        }
        proyectos.sort_by_key(|p| (p.lugar != Lugar::Disco, p.nombre.to_lowercase()));
        notas.sort_by_key(|n| std::cmp::Reverse(n.modificado));

        let colores = self.colores()?;
        let mut conteo: BTreeMap<String, usize> = colores.keys().map(|k| (k.clone(), 0)).collect();
        for n in &notas {
            for e in &n.etiquetas {
                *conteo.entry(e.clone()).or_default() += 1;
            }
        }
        let etiquetas = conteo
            .into_iter()
            .map(|(nombre, notas)| Etiqueta {
                color: colores.get(&nombre).cloned().unwrap_or_else(|| COLOR_SIN_REGISTRO.into()),
                nombre,
                notas,
            })
            .collect();

        Ok(Estado {
            equipo: mostrar(self.equipo.raiz()),
            disco: self.disco.as_ref().map(|d| InfoDisco { ruta: mostrar(d), conectado: d.is_dir() }),
            proyectos,
            etiquetas,
            notas,
            paleta: PALETA.iter().map(|c| c.to_string()).collect(),
        })
    }

    pub fn leer_nota(&self, id: &str) -> Result<Nota> {
        self.de(id)?.leer_nota(id)
    }

    pub fn buscar(&self, texto: &str) -> Result<Vec<String>> {
        let aguja = texto.to_lowercase();
        let mut ids = Vec::new();
        for s in self.disponibles() {
            ids.extend(s.buscar(&aguja)?);
        }
        Ok(ids)
    }

    // ---------------------------------------------------------------- notas

    /// Sin proyecto, la nota va a la Bandeja del disco si está conectado; si no, a la del equipo.
    pub fn crear_nota(&self, proyecto: Option<&str>, etiquetas: &[String]) -> Result<Nota> {
        let s = match proyecto {
            Some(p) => self.de(p)?,
            None => self.disco().unwrap_or(self.lugar(Lugar::Equipo)?),
        };
        self.registrar_etiquetas(&s, etiquetas)?;
        s.crear_nota(proyecto, etiquetas)
    }

    pub fn guardar_nota(&self, id: &str, titulo: &str, etiquetas: &[String], hoja: Option<&str>, pauta: Option<&str>, cuerpo: &str) -> Result<NotaResumen> {
        let s = self.de(id)?;
        self.registrar_etiquetas(&s, etiquetas)?;
        s.guardar_nota(id, titulo, etiquetas, hoja, pauta, cuerpo)
    }

    /// Mueve una nota a otro proyecto, aunque esté en el otro lugar.
    pub fn mover_nota(&self, id: &str, proyecto: &str) -> Result<NotaResumen> {
        let origen = self.de(id)?;
        let destino = self.de(proyecto)?;
        let path = origen.ruta(id)?;
        let nota = origen.leer_nota(id)?;
        if nota.resumen.proyecto == proyecto {
            return Ok(nota.resumen);
        }
        self.registrar_etiquetas(&destino, &nota.resumen.etiquetas)?;
        let movida = destino.recibir_nota(&path, proyecto)?;
        fs::remove_file(&path).with_context(|| format!("quitando {}", path.display()))?;
        Ok(movida)
    }

    pub fn borrar_nota(&self, id: &str) -> Result<()> {
        self.de(id)?.borrar(id)
    }

    // ---------------------------------------------------------------- proyectos

    pub fn crear_proyecto(&self, nombre: &str, lugar: Lugar) -> Result<String> {
        self.lugar(lugar)?.crear_proyecto(nombre)
    }

    pub fn renombrar_proyecto(&self, id: &str, nombre: &str) -> Result<()> {
        self.de(id)?.renombrar_proyecto(id, nombre)
    }

    pub fn borrar_proyecto(&self, id: &str) -> Result<()> {
        let s = self.de(id)?;
        if !s.ruta(id)?.is_dir() {
            bail!("el proyecto no existe");
        }
        s.borrar(id)
    }

    /// Lleva un proyecto con todas sus notas al otro lugar. El original queda en la
    /// papelera de su lugar, por si algo sale mal. Devuelve el id nuevo.
    pub fn mover_proyecto(&self, id: &str, lugar: Lugar) -> Result<String> {
        let origen = self.de(id)?;
        if origen.lugar() == lugar {
            return Ok(id.to_string());
        }
        let destino = self.lugar(lugar)?;
        let dir = origen.ruta(id)?;
        if !dir.is_dir() {
            bail!("el proyecto no existe");
        }
        let (_, notas) = origen.proyectos_y_notas()?;
        let etiquetas: Vec<String> =
            notas.iter().filter(|n| n.proyecto == id).flat_map(|n| n.etiquetas.clone()).collect();
        self.registrar_etiquetas(&destino, &etiquetas)?;
        let nuevo = destino.recibir_proyecto(&dir)?;
        origen.borrar(id)?;
        Ok(nuevo)
    }

    // ---------------------------------------------------------------- etiquetas

    /// Colores de todos los lugares; el disco manda porque es el que viaja.
    fn colores(&self) -> Result<BTreeMap<String, String>> {
        let mut colores = BTreeMap::new();
        for s in self.disponibles() {
            colores.extend(s.colores()?);
        }
        Ok(colores)
    }

    /// Da color en `s` a las etiquetas que aún no tiene (el mismo que en el otro lugar, si existe).
    fn registrar_etiquetas(&self, s: &Store, etiquetas: &[String]) -> Result<()> {
        let globales = self.colores()?;
        let mut locales = s.colores()?;
        let mut nuevas = 0;
        let mut cambio = false;
        for e in normalizar_etiquetas(etiquetas) {
            if let std::collections::btree_map::Entry::Vacant(hueco) = locales.entry(e) {
                let color = match globales.get(hueco.key()) {
                    Some(c) => c.clone(),
                    None => {
                        nuevas += 1;
                        PALETA[(globales.len() + nuevas - 1) % PALETA.len()].to_string()
                    }
                };
                hueco.insert(color);
                cambio = true;
            }
        }
        if cambio {
            s.guardar_colores(&locales)?;
        }
        Ok(())
    }

    /// Crea o edita una etiqueta en todos los lugares conectados. Si cambia el nombre,
    /// se renombra en todas las notas.
    pub fn guardar_etiqueta(&self, original: Option<&str>, nombre: &str, color: &str) -> Result<String> {
        let nombre = normalizar_etiqueta(nombre);
        if nombre.is_empty() {
            bail!("el nombre de la etiqueta no es válido");
        }
        let existe = self.estado()?.etiquetas.iter().any(|e| e.nombre == nombre);
        let renombrar = original.filter(|o| *o != nombre);
        if existe && (original.is_none() || renombrar.is_some()) {
            bail!("ya existe la etiqueta «{nombre}»");
        }
        for s in self.disponibles() {
            let mut colores = s.colores()?;
            if let Some(o) = renombrar {
                colores.remove(o);
                let nuevo = nombre.clone();
                s.reescribir_etiquetas(&|e| {
                    for x in e.iter_mut().filter(|x| *x == o) {
                        x.clone_from(&nuevo);
                    }
                })?;
            }
            colores.insert(nombre.clone(), color.to_string());
            s.guardar_colores(&colores)?;
        }
        Ok(nombre)
    }

    pub fn borrar_etiqueta(&self, nombre: &str) -> Result<()> {
        for s in self.disponibles() {
            let mut colores = s.colores()?;
            if colores.remove(nombre).is_some() {
                s.guardar_colores(&colores)?;
            }
            s.reescribir_etiquetas(&|e| e.retain(|x| x != nombre))?;
        }
        Ok(())
    }
}

fn mismo_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Ruta con `~` en lugar de la carpeta personal, para mostrarla.
fn mostrar(path: &Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(resto) = path.strip_prefix(&home)
    {
        return format!("~/{}", resto.display());
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::dir_temporal;

    fn biblioteca(nombre: &str) -> (Biblioteca, PathBuf) {
        let base = dir_temporal(nombre);
        let b = Biblioteca::con_rutas(base.join("equipo"), base.join("config/ajustes.json")).unwrap();
        (b, base)
    }

    #[test]
    fn disco_no_conectado_no_crea_nada() {
        let (mut b, base) = biblioteca("sin-disco");
        b.disco = Some(base.join("Volumes/DISCO/Notas"));
        let est = b.estado().unwrap();
        assert!(!est.disco.unwrap().conectado);
        assert!(b.crear_proyecto("X", Lugar::Disco).is_err());
        assert!(!base.join("Volumes").exists(), "no debe crear la ruta del disco");
        // Sin proyecto, la nota cae en la Bandeja del equipo.
        assert_eq!(b.crear_nota(None, &[]).unwrap().resumen.proyecto, "equipo:bandeja");
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn elegir_disco_usa_subcarpeta_y_se_recuerda() {
        let (mut b, base) = biblioteca("elegir");
        let montaje = base.join("DISCO");
        fs::create_dir_all(montaje.join("Syntara")).unwrap();
        b.elegir_disco(&montaje).unwrap();
        assert!(montaje.join("Notas").join(MARCADOR).is_file());
        // Elegir la carpeta Notas directamente (ya marcada) no anida otra.
        b.elegir_disco(&montaje.join("Notas")).unwrap();
        assert_eq!(b.disco.as_deref(), Some(montaje.join("Notas").as_path()));
        // Otra instancia lee la ruta de los ajustes.
        let b2 = Biblioteca::con_rutas(base.join("equipo"), base.join("config/ajustes.json")).unwrap();
        assert_eq!(b2.disco, b.disco);
        // Las carpetas del disco que no son Notas no aparecen como proyectos.
        assert!(b2.estado().unwrap().proyectos.is_empty());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn mover_entre_equipo_y_disco() {
        let (mut b, base) = biblioteca("mover");
        b.elegir_disco(&base.join("DISCO")).unwrap();

        let local = b.crear_proyecto("Recetas", Lugar::Equipo).unwrap();
        let n = b.crear_nota(Some(&local), &[]).unwrap();
        let n = b.guardar_nota(&n.resumen.id, "Pan", &["cocina".into()], None, None, "harina").unwrap();

        // Proyecto completo al disco.
        let en_disco = b.mover_proyecto(&local, Lugar::Disco).unwrap();
        assert_eq!(en_disco, "disco:recetas");
        assert!(base.join("DISCO/Notas/recetas/pan.md").is_file());
        assert!(!base.join("equipo/recetas").exists());
        let est = b.estado().unwrap();
        assert_eq!(est.proyectos.len(), 1);
        assert_eq!(est.notas[0].id, "disco:recetas/pan.md");
        assert!(b.lugar(Lugar::Disco).unwrap().colores().unwrap().contains_key("cocina"), "el color viaja");

        // Una nota suelta de vuelta al equipo.
        let otro = b.crear_proyecto("Apuntes", Lugar::Equipo).unwrap();
        let movida = b.mover_nota("disco:recetas/pan.md", &otro).unwrap();
        assert_eq!(movida.id, "equipo:apuntes/pan.md");
        assert!(!base.join("DISCO/Notas/recetas/pan.md").exists());
        let _ = n;

        // Varias etiquetas nuevas a la vez reciben colores distintos.
        let n2 = b.crear_nota(Some(&otro), &[]).unwrap();
        b.guardar_nota(&n2.resumen.id, "Varias", &["x1".into(), "x2".into()], None, None, "").unwrap();
        let colores = b.colores().unwrap();
        assert_ne!(colores["x1"], colores["x2"]);

        // Renombrar una etiqueta afecta a los dos lugares.
        b.guardar_etiqueta(Some("cocina"), "comida", "#000000").unwrap();
        assert_eq!(b.leer_nota(&movida.id).unwrap().resumen.etiquetas, vec!["comida"]);
        assert!(b.guardar_etiqueta(None, "comida", "#fff").is_err(), "duplicada");
        fs::remove_dir_all(base).unwrap();
    }
}
