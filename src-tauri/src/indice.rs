//! Índice de similitud local (TF‑IDF), sin modelos ni red.
//!
//! - Cada nota se convierte en una bolsa de palabras: minúsculas, sin acentos, sin palabras vacías
//!   (español e inglés), plurales simples unificados. El título pesa ×3 y las etiquetas ×2.
//! - Es incremental: se guarda la fecha de modificación de cada archivo y solo se vuelven a leer
//!   los que cambiaron (o aparecieron); los borrados salen del índice.
//! - Da notas relacionadas (similitud del coseno), pares parecidos (para el grafo y la IA) y
//!   búsqueda BM25 (para «Preguntar a tus notas»).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use serde::Serialize;

use crate::frontmatter;

const PESO_TITULO: f32 = 3.0;
const PESO_ETIQUETA: f32 = 2.0;
/// Similitud mínima para considerar dos notas relacionadas.
pub const UMBRAL_RELACIONADA: f32 = 0.08;

struct Doc {
    mtime: Option<SystemTime>,
    titulo: String,
    /// Frecuencia (ponderada) de cada término.
    tf: HashMap<String, f32>,
    /// Longitud en términos (para BM25).
    largo: f32,
}

/// Archivo a indexar: id de la nota y su ruta.
pub struct Entrada {
    pub id: String,
    pub ruta: PathBuf,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Relacionada {
    pub id: String,
    pub titulo: String,
    pub proyecto: String,
    pub puntuacion: f32,
    /// Palabras que más aportan a la similitud (para explicar el porqué).
    pub terminos: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Par {
    pub a: String,
    pub b: String,
    pub puntuacion: f32,
}

#[derive(Default)]
pub struct Indice {
    docs: HashMap<String, Doc>,
    /// Vectores TF‑IDF normalizados; se recalculan cuando cambia algún documento.
    vectores: Option<HashMap<String, Vec<(String, f32)>>>,
}

/// `equipo:proyecto/nota.md` → `equipo:proyecto`.
pub fn proyecto_de(id: &str) -> String {
    match id.split_once('/') {
        Some((p, _)) => p.to_string(),
        None => id.to_string(),
    }
}

impl Indice {
    /// Sincroniza con los archivos actuales. Devuelve cuántos se (re)leyeron.
    pub fn actualizar(&mut self, entradas: &[Entrada]) -> usize {
        let vivos: HashSet<&str> = entradas.iter().map(|e| e.id.as_str()).collect();
        let antes = self.docs.len();
        self.docs.retain(|id, _| vivos.contains(id.as_str()));
        let mut cambios = antes - self.docs.len();
        let mut leidos = 0;
        for e in entradas {
            let mtime = fs::metadata(&e.ruta).and_then(|m| m.modified()).ok();
            if let Some(d) = self.docs.get(&e.id)
                && d.mtime.is_some()
                && d.mtime == mtime
            {
                continue;
            }
            let Ok(texto) = fs::read_to_string(&e.ruta) else { continue };
            self.poner(&e.id, &texto, mtime);
            leidos += 1;
            cambios += 1;
        }
        if cambios > 0 {
            self.vectores = None;
        }
        leidos
    }

    /// Indexa un documento a partir de su texto completo (con front matter).
    fn poner(&mut self, id: &str, texto: &str, mtime: Option<SystemTime>) {
        let doc = frontmatter::parsear(texto);
        let titulo = doc.titulo.clone().unwrap_or_default();
        let mut tf: HashMap<String, f32> = HashMap::new();
        let mut largo = 0.0;
        let mut sumar = |t: String, peso: f32| {
            *tf.entry(t).or_default() += peso;
            largo += peso;
        };
        for t in terminos(&titulo) {
            sumar(t, PESO_TITULO);
        }
        for e in &doc.etiquetas {
            for t in terminos(e) {
                sumar(t, PESO_ETIQUETA);
            }
        }
        for t in terminos(&doc.cuerpo) {
            sumar(t, 1.0);
        }
        self.docs.insert(id.to_string(), Doc { mtime, titulo, tf, largo });
        self.vectores = None;
    }

    fn idf(&self) -> HashMap<&str, f32> {
        let mut df: HashMap<&str, f32> = HashMap::new();
        for d in self.docs.values() {
            for t in d.tf.keys() {
                *df.entry(t.as_str()).or_default() += 1.0;
            }
        }
        let n = self.docs.len() as f32;
        df.into_iter().map(|(t, f)| (t, (1.0 + n / f).ln())).collect()
    }

    fn vectores(&mut self) -> &HashMap<String, Vec<(String, f32)>> {
        self.calcular_vectores();
        self.vectores.as_ref().unwrap()
    }

    fn calcular_vectores(&mut self) {
        if self.vectores.is_none() {
            let idf = self.idf();
            let mut out = HashMap::new();
            for (id, d) in &self.docs {
                let mut v: Vec<(String, f32)> =
                    d.tf.iter().map(|(t, f)| (t.clone(), (1.0 + f.ln()) * idf.get(t.as_str()).copied().unwrap_or(0.0))).collect();
                let norma = v.iter().map(|(_, w)| w * w).sum::<f32>().sqrt();
                if norma > 0.0 {
                    v.iter_mut().for_each(|(_, w)| *w /= norma);
                }
                v.sort_by(|a, b| a.0.cmp(&b.0));
                out.insert(id.clone(), v);
            }
            self.vectores = Some(out);
        }
    }

    /// Notas más parecidas a `id`, de mayor a menor.
    pub fn relacionadas(&mut self, id: &str, max: usize) -> Vec<Relacionada> {
        self.calcular_vectores();
        let vs = self.vectores.as_ref().unwrap();
        let Some(base) = vs.get(id) else { return Vec::new() };
        let mut out: Vec<(f32, &String, Vec<(f32, &str)>)> = vs
            .iter()
            .filter(|(otro, _)| otro.as_str() != id)
            .filter_map(|(otro, v)| {
                let (p, aportes) = producto(base, v);
                (p >= UMBRAL_RELACIONADA).then_some((p, otro, aportes))
            })
            .collect();
        out.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        out.truncate(max);
        out.into_iter()
            .map(|(p, otro, mut aportes)| {
                aportes.sort_by(|a, b| b.0.total_cmp(&a.0));
                Relacionada {
                    id: otro.clone(),
                    titulo: self.docs.get(otro).map(|d| d.titulo.clone()).unwrap_or_default(),
                    proyecto: proyecto_de(otro),
                    puntuacion: (p * 1000.0).round() / 1000.0,
                    terminos: aportes.into_iter().take(3).map(|(_, t)| t.to_string()).collect(),
                }
            })
            .collect()
    }

    /// Pares de notas con similitud ≥ `umbral`; cada nota aporta como mucho `por_nota` vecinos.
    pub fn pares(&mut self, umbral: f32, por_nota: usize) -> Vec<Par> {
        let vs = self.vectores();
        // Índice invertido: término → [(doc, peso)].
        let mut postings: HashMap<&str, Vec<(&str, f32)>> = HashMap::new();
        for (id, v) in vs {
            for (t, w) in v {
                postings.entry(t.as_str()).or_default().push((id.as_str(), *w));
            }
        }
        let mut vistos: HashSet<(String, String)> = HashSet::new();
        let mut out = Vec::new();
        let mut ids: Vec<&String> = vs.keys().collect();
        ids.sort();
        for id in ids {
            let mut acum: HashMap<&str, f32> = HashMap::new();
            for (t, w) in &vs[id] {
                for (otro, w2) in &postings[t.as_str()] {
                    if *otro != id.as_str() {
                        *acum.entry(otro).or_default() += w * w2;
                    }
                }
            }
            let mut vecinos: Vec<(&str, f32)> = acum.into_iter().filter(|(_, p)| *p >= umbral).collect();
            vecinos.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
            for (otro, p) in vecinos.into_iter().take(por_nota) {
                let (a, b) = if id.as_str() < otro { (id.clone(), otro.to_string()) } else { (otro.to_string(), id.clone()) };
                if vistos.insert((a.clone(), b.clone())) {
                    out.push(Par { a, b, puntuacion: (p * 1000.0).round() / 1000.0 });
                }
            }
        }
        out
    }

    /// Búsqueda BM25: ids con su puntuación, de mayor a menor.
    pub fn buscar(&self, consulta: &str, max: usize) -> Vec<(String, f32)> {
        const K1: f32 = 1.2;
        const B: f32 = 0.75;
        let q: HashSet<String> = terminos(consulta).collect();
        if q.is_empty() || self.docs.is_empty() {
            return Vec::new();
        }
        let n = self.docs.len() as f32;
        let medio = self.docs.values().map(|d| d.largo).sum::<f32>() / n;
        let mut out: Vec<(String, f32)> = Vec::new();
        for (id, d) in &self.docs {
            let mut s = 0.0;
            for t in &q {
                let Some(f) = d.tf.get(t) else { continue };
                let df = self.docs.values().filter(|o| o.tf.contains_key(t)).count() as f32;
                let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
                s += idf * f * (K1 + 1.0) / (f + K1 * (1.0 - B + B * d.largo / medio.max(1.0)));
            }
            if s > 0.0 {
                out.push((id.clone(), s));
            }
        }
        out.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        out.truncate(max);
        out
    }
}

/// Producto escalar de dos vectores ordenados por término, con el aporte de cada término común.
fn producto<'a>(a: &'a [(String, f32)], b: &[(String, f32)]) -> (f32, Vec<(f32, &'a str)>) {
    let (mut i, mut j, mut total, mut aportes) = (0, 0, 0.0, Vec::new());
    while i < a.len() && j < b.len() {
        match a[i].0.cmp(&b[j].0) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                let p = a[i].1 * b[j].1;
                total += p;
                aportes.push((p, a[i].0.as_str()));
                i += 1;
                j += 1;
            }
        }
    }
    (total, aportes)
}

// ------------------------------------------------------------------ texto → términos

fn sin_acento(c: char) -> char {
    match c {
        'á' | 'à' | 'ä' | 'â' => 'a',
        'é' | 'è' | 'ë' | 'ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' => 'o',
        'ú' | 'ù' | 'ü' | 'û' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        c => c,
    }
}

/// Unifica plurales simples («notas» → «nota», «canciones» → «cancion»).
fn raiz(t: String) -> String {
    let n = t.chars().count();
    if n > 5 && t.ends_with("ones") {
        return t[..t.len() - 2].to_string();
    }
    if n > 4 && t.ends_with('s') && !t.ends_with("ss") && !t.ends_with("us") && !t.ends_with("is") {
        return t[..t.len() - 1].to_string();
    }
    t
}

/// Términos de un texto, en orden. Ignora enlaces (`http…`), números y palabras vacías.
pub fn terminos(texto: &str) -> impl Iterator<Item = String> + '_ {
    texto
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .flat_map(|p| p.split('\''))
        .map(|p| p.chars().flat_map(char::to_lowercase).map(sin_acento).collect::<String>())
        .filter(|p| p.chars().count() >= 3 && !p.chars().all(|c| c.is_ascii_digit()))
        .filter(|p| !p.starts_with("http") && !VACIAS.contains(&p.as_str()))
        .map(raiz)
}

/// Palabras vacías (ya sin acentos), más ruido típico de Markdown y de rutas.
const VACIAS: &[&str] = &[
    // español
    "aca", "ahi", "ajena", "ajeno", "algo", "alguien", "algun", "alguna", "algunas", "alguno", "algunos", "alla", "alli",
    "ambos", "ante", "antes", "aquel", "aquella", "aquellas", "aquello", "aquellos", "aqui", "asi", "aun", "aunque", "bajo",
    "bien", "cada", "casi", "cierta", "cierto", "como", "con", "conmigo", "contigo", "contra", "cual", "cuales", "cualquier",
    "cuando", "cuanto", "cuya", "cuyo", "del", "demas", "desde", "donde", "dos", "durante", "ella", "ellas", "ello", "ellos",
    "entonces", "entre", "era", "eran", "eres", "esa", "esas", "ese", "eso", "esos", "esta", "estaba", "estado", "estamos",
    "estan", "estar", "estas", "este", "esto", "estos", "estoy", "fue", "fueron", "gran", "haber", "habia", "hace", "hacer",
    "hacia", "han", "has", "hasta", "hay", "hecho", "las", "les", "los", "mas", "mejor", "menos", "mientras", "mis", "mismo",
    "mucho", "muy", "nada", "nadie", "ni", "nos", "nosotros", "nuestra", "nuestro", "nunca", "otra", "otras", "otro", "otros",
    "para", "pero", "poco", "por", "porque", "pues", "puede", "pueden", "que", "quien", "quienes", "sea", "segun", "ser",
    "sera", "si", "sido", "siempre", "sin", "sobre", "solo", "son", "soy", "sus", "tal", "tambien", "tampoco", "tan", "tanto",
    "tener", "tengo", "tiene", "tienen", "toda", "todas", "todo", "todos", "tras", "tus", "una", "unas", "uno", "unos",
    "usted", "ustedes", "vamos", "van", "veces", "vez", "vosotros", "voy", "yo",
    // inglés
    "about", "after", "all", "also", "and", "any", "are", "because", "been", "before", "being", "but", "can", "could",
    "did", "does", "doing", "for", "from", "had", "has", "have", "her", "here", "him", "his", "how", "into", "its", "just",
    "more", "most", "not", "now", "only", "other", "our", "out", "over", "she", "should", "some", "such", "than", "that",
    "the", "their", "them", "then", "there", "these", "they", "this", "those", "through", "too", "very", "was", "were",
    "what", "when", "where", "which", "while", "who", "why", "will", "with", "would", "you", "your",
    // Markdown, rutas, HTML
    "www", "com", "png", "jpg", "jpeg", "gif", "webp", "asset", "assets", "adjunto", "details", "summary", "nbsp", "todo",
];

#[cfg(test)]
mod pruebas {
    use super::*;

    fn indice(notas: &[(&str, &str)]) -> Indice {
        let mut i = Indice::default();
        for (id, texto) in notas {
            i.poner(id, texto, None);
        }
        i
    }

    #[test]
    fn terminos_normalizados() {
        let t: Vec<String> = terminos("Las Canciones de la Búsqueda: notas, http://x.com y 2026 — l'avion").collect();
        assert_eq!(t, ["cancion", "busqueda", "nota", "avion"]);
    }

    #[test]
    fn relacionadas_por_tema() {
        let mut i = indice(&[
            ("equipo:a/rust.md", "---\ntitle: Ownership en Rust\n---\nEl borrow checker de Rust y los lifetimes."),
            ("equipo:a/rust2.md", "---\ntitle: Lifetimes\ntags: [rust]\n---\nCómo pensar los lifetimes con el borrow checker."),
            ("equipo:b/pan.md", "---\ntitle: Receta de pan\n---\nHarina, agua, sal y levadura. Amasar y hornear."),
            ("equipo:b/pizza.md", "---\ntitle: Masa de pizza\n---\nHarina, agua y levadura; hornear muy caliente."),
        ]);
        let r = i.relacionadas("equipo:a/rust.md", 5);
        assert_eq!(r.len(), 1, "{r:?}");
        assert_eq!(r[0].id, "equipo:a/rust2.md");
        assert_eq!(r[0].proyecto, "equipo:a");
        assert!(r[0].terminos.contains(&"lifetime".to_string()), "{:?}", r[0].terminos);

        let r = i.relacionadas("equipo:b/pan.md", 5);
        assert_eq!(r.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["equipo:b/pizza.md"]);

        let pares = i.pares(0.1, 3);
        assert_eq!(pares.len(), 2, "{pares:?}");
        assert!(pares.iter().all(|p| p.a < p.b));
    }

    #[test]
    fn bm25_prefiere_el_titulo() {
        let i = indice(&[
            ("equipo:a/1.md", "---\ntitle: Presupuesto\n---\nCifras del año."),
            ("equipo:a/2.md", "---\ntitle: Reunión\n---\nSe habló del presupuesto un momento."),
            ("equipo:a/3.md", "---\ntitle: Otra\n---\nNada que ver."),
        ]);
        let r = i.buscar("presupuestos", 10);
        assert_eq!(r.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["equipo:a/1.md", "equipo:a/2.md"]);
    }

    #[test]
    fn incremental_por_fecha() {
        let dir = std::env::temp_dir().join(format!("md-notes-indice-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let ruta = |n: &str| dir.join(n);
        fs::write(ruta("a.md"), "uno dos tres").unwrap();
        fs::write(ruta("b.md"), "cuatro cinco").unwrap();
        let entradas = |nombres: &[&str]| nombres.iter().map(|n| Entrada { id: format!("equipo:p/{n}"), ruta: ruta(n) }).collect::<Vec<_>>();
        let mut i = Indice::default();
        assert_eq!(i.actualizar(&entradas(&["a.md", "b.md"])), 2);
        assert_eq!(i.actualizar(&entradas(&["a.md", "b.md"])), 0);
        // Borrar sale del índice sin releer nada.
        assert_eq!(i.actualizar(&entradas(&["a.md"])), 0);
        assert_eq!(i.docs.len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }
}
