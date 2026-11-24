//! Relaciones entre notas y tareas.
//!
//! - Enlaces `[[Título]]`, `[[Título|texto]]` y `[[Título#sección]]` (compatibles con Obsidian).
//!   Un destino se resuelve por título o por nombre de archivo, sin importar mayúsculas ni
//!   acentos; si hay varias notas con el mismo nombre, gana la del mismo proyecto.
//! - «Enlazan aquí»: qué notas apuntan a una nota, con la línea que la enlaza.
//! - Renombrar: al cambiar el título de una nota se pueden actualizar los `[[…]]` que la nombran.
//! - Tareas: `- [ ] texto` en cualquier nota, con fecha opcional `📅 2026-12-01` o `@2026-12-01`.
//!
//! Se ignora lo que está dentro de bloques de código y de `código en línea`.

use std::collections::HashMap;

use anyhow::{Result, bail};
use serde::Serialize;

use crate::frontmatter;
use crate::store::{Nota, slug};

// ------------------------------------------------------------------ recorrido del texto

/// Líneas fuera de bloques de código (``` o ~~~), con su número (base 0).
fn lineas_de_texto(cuerpo: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut valla: Option<&str> = None;
    cuerpo.lines().enumerate().filter(move |(_, l)| {
        let t = l.trim_start();
        let marca = if t.starts_with("```") { Some("```") } else if t.starts_with("~~~") { Some("~~~") } else { None };
        match (valla, marca) {
            (None, Some(m)) => {
                valla = Some(m);
                false
            }
            (Some(v), Some(m)) if v == m => {
                valla = None;
                false
            }
            (Some(_), _) => false,
            (None, None) => true,
        }
    })
}

/// La línea con el `código en línea` tapado por espacios (mismas posiciones).
fn sin_codigo_en_linea(linea: &str) -> String {
    let mut out = String::with_capacity(linea.len());
    let mut dentro = false;
    for c in linea.chars() {
        if c == '`' {
            dentro = !dentro;
            out.push(' ');
        } else {
            out.push(if dentro { ' ' } else { c });
        }
    }
    out
}

// ------------------------------------------------------------------ wikilinks

/// Un `[[…]]` dentro de una línea: rango en bytes del destino (sin `|texto` ni `#sección`).
struct Encontrado {
    destino: std::ops::Range<usize>,
}

fn buscar_en_linea(linea: &str) -> Vec<Encontrado> {
    let visible = sin_codigo_en_linea(linea);
    let mut out = Vec::new();
    let mut desde = 0;
    while let Some(i) = visible[desde..].find("[[") {
        let ini = desde + i + 2;
        let Some(j) = visible[ini..].find("]]") else { break };
        let fin = ini + j;
        let dentro = &visible[ini..fin];
        if !dentro.contains('[') && !dentro.trim().is_empty() {
            let corte = dentro.find(['|', '#']).unwrap_or(dentro.len());
            let destino = &dentro[..corte];
            let espacios_izq = destino.len() - destino.trim_start().len();
            let d_ini = ini + espacios_izq;
            let d_fin = ini + destino.trim_end().len();
            if d_fin > d_ini {
                out.push(Encontrado { destino: d_ini..d_fin });
            }
        }
        desde = fin + 2;
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct Wikilink {
    pub destino: String,
    pub linea: usize,
}

pub fn wikilinks(cuerpo: &str) -> Vec<Wikilink> {
    let mut out = Vec::new();
    for (n, linea) in lineas_de_texto(cuerpo) {
        for e in buscar_en_linea(linea) {
            out.push(Wikilink { destino: linea[e.destino].to_string(), linea: n });
        }
    }
    out
}

/// Clave con la que se comparan destinos y títulos.
pub fn clave(texto: &str) -> String {
    slug(texto)
}

fn nombre_archivo(id: &str) -> &str {
    let ultimo = id.rsplit(['/', ':']).next().unwrap_or(id);
    ultimo.strip_suffix(".md").unwrap_or(ultimo)
}

/// Resuelve destinos `[[…]]` a ids de nota.
pub struct Resolutor {
    por_clave: HashMap<String, Vec<(String, String)>>,
}

impl Resolutor {
    pub fn nuevo(notas: &[Nota]) -> Self {
        let mut por_clave: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for n in notas {
            let r = &n.resumen;
            let mut claves = vec![clave(&r.titulo), clave(nombre_archivo(&r.id))];
            claves.dedup();
            for k in claves.into_iter().filter(|k| !k.is_empty()) {
                por_clave.entry(k).or_default().push((r.id.clone(), r.proyecto.clone()));
            }
        }
        Self { por_clave }
    }

    pub fn resolver(&self, destino: &str, desde_proyecto: &str) -> Option<&str> {
        let candidatos = self.por_clave.get(&clave(destino))?;
        candidatos
            .iter()
            .find(|(_, p)| p == desde_proyecto)
            .or_else(|| candidatos.first())
            .map(|(id, _)| id.as_str())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Saliente {
    pub destino: String,
    pub id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entrante {
    pub id: String,
    pub titulo: String,
    pub proyecto: String,
    /// La línea que contiene el enlace, recortada.
    pub contexto: String,
    pub linea: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Enlaces {
    pub salientes: Vec<Saliente>,
    pub entrantes: Vec<Entrante>,
}

fn recortar(texto: &str, max: usize) -> String {
    let t = texto.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let mut s: String = t.chars().take(max).collect();
    s.push('…');
    s
}

pub fn enlaces(notas: &[Nota], id: &str) -> Enlaces {
    let resolutor = Resolutor::nuevo(notas);
    let mut salientes: Vec<Saliente> = Vec::new();
    if let Some(nota) = notas.iter().find(|n| n.resumen.id == id) {
        for w in wikilinks(&nota.cuerpo) {
            if salientes.iter().any(|s| clave(&s.destino) == clave(&w.destino)) {
                continue;
            }
            let destino_id = resolutor.resolver(&w.destino, &nota.resumen.proyecto).map(str::to_string);
            salientes.push(Saliente { destino: w.destino, id: destino_id });
        }
    }
    let mut entrantes = Vec::new();
    for n in notas.iter().filter(|n| n.resumen.id != id) {
        let lineas: Vec<&str> = n.cuerpo.lines().collect();
        if let Some(w) = wikilinks(&n.cuerpo)
            .into_iter()
            .find(|w| resolutor.resolver(&w.destino, &n.resumen.proyecto) == Some(id))
        {
            entrantes.push(Entrante {
                id: n.resumen.id.clone(),
                titulo: n.resumen.titulo.clone(),
                proyecto: n.resumen.proyecto.clone(),
                contexto: recortar(lineas.get(w.linea).unwrap_or(&""), 200),
                linea: w.linea,
            });
        }
    }
    entrantes.sort_by(|a, b| a.titulo.to_lowercase().cmp(&b.titulo.to_lowercase()));
    Enlaces { salientes, entrantes }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Arista {
    pub a: String,
    pub b: String,
}

/// Todos los wikilinks resueltos entre notas, sin dirección ni repetidos (para el grafo).
pub fn todas_las_aristas(notas: &[Nota]) -> Vec<Arista> {
    let resolutor = Resolutor::nuevo(notas);
    let mut vistos = std::collections::HashSet::new();
    let mut out = Vec::new();
    for n in notas {
        for w in wikilinks(&n.cuerpo) {
            let Some(destino) = resolutor.resolver(&w.destino, &n.resumen.proyecto) else { continue };
            let origen = n.resumen.id.as_str();
            if destino == origen {
                continue;
            }
            let (a, b) = if origen < destino { (origen, destino) } else { (destino, origen) };
            if vistos.insert((a.to_string(), b.to_string())) {
                out.push(Arista { a: a.to_string(), b: b.to_string() });
            }
        }
    }
    out
}

/// Cuántos `[[…]]` de todas las notas nombran `titulo`.
pub fn contar_a(notas: &[Nota], titulo: &str) -> usize {
    let k = clave(titulo);
    if k.is_empty() {
        return 0;
    }
    notas.iter().flat_map(|n| wikilinks(&n.cuerpo)).filter(|w| clave(&w.destino) == k).count()
}

/// Cambia los `[[viejo…]]` por `[[nuevo…]]` (conserva `|texto` y `#sección`). `None` si no cambia nada.
pub fn reemplazar_destino(texto: &str, viejo: &str, nuevo: &str) -> Option<String> {
    let k = clave(viejo);
    if k.is_empty() || nuevo.trim().is_empty() {
        return None;
    }
    let en_codigo: std::collections::HashSet<usize> = {
        let fuera: std::collections::HashSet<usize> = lineas_de_texto(texto).map(|(n, _)| n).collect();
        (0..texto.lines().count()).filter(|n| !fuera.contains(n)).collect()
    };
    let mut cambiado = false;
    let mut out = String::with_capacity(texto.len());
    for (n, linea) in texto.split_inclusive('\n').enumerate() {
        if en_codigo.contains(&n) {
            out.push_str(linea);
            continue;
        }
        let mut nueva = String::new();
        let mut ultimo = 0;
        for e in buscar_en_linea(linea) {
            if clave(&linea[e.destino.clone()]) == k {
                nueva.push_str(&linea[ultimo..e.destino.start]);
                nueva.push_str(nuevo.trim());
                ultimo = e.destino.end;
                cambiado = true;
            }
        }
        nueva.push_str(&linea[ultimo..]);
        out.push_str(&nueva);
    }
    cambiado.then_some(out)
}

// ------------------------------------------------------------------ tareas

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tarea {
    pub id: String,
    pub titulo: String,
    pub proyecto: String,
    /// Línea dentro del cuerpo (base 0).
    pub linea: usize,
    pub hecha: bool,
    pub texto: String,
    /// `AAAA-MM-DD`
    pub fecha: Option<String>,
}

/// Si la línea es una tarea: (hecha, texto con la fecha, posición en bytes del carácter de la casilla).
fn tarea_en(linea: &str) -> Option<(bool, &str, usize)> {
    let sangria = linea.len() - linea.trim_start().len();
    let resto = &linea[sangria..];
    let resto2 = resto.strip_prefix("- ").or_else(|| resto.strip_prefix("* ")).or_else(|| resto.strip_prefix("+ "))?;
    let bullet = resto.len() - resto2.len();
    let marca = resto2.get(..3)?;
    let hecha = match marca {
        "[ ]" => false,
        "[x]" | "[X]" => true,
        _ => return None,
    };
    let texto = resto2[3..].strip_prefix(' ').unwrap_or(&resto2[3..]);
    if !resto2[3..].is_empty() && !resto2[3..].starts_with(' ') {
        return None;
    }
    Some((hecha, texto, sangria + bullet + 1))
}

fn es_fecha(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b.iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() })
}

/// Separa la fecha (`📅 AAAA-MM-DD` o `@AAAA-MM-DD`) del texto de una tarea.
fn separar_fecha(texto: &str) -> (String, Option<String>) {
    let mut fecha = None;
    let mut partes: Vec<&str> = Vec::new();
    let mut tokens = texto.split_whitespace().peekable();
    while let Some(t) = tokens.next() {
        if fecha.is_none() {
            if t == "📅" && tokens.peek().is_some_and(|f| es_fecha(f)) {
                fecha = tokens.next().map(str::to_string);
                continue;
            }
            if let Some(f) = t.strip_prefix("📅").or_else(|| t.strip_prefix('@'))
                && es_fecha(f)
            {
                fecha = Some(f.to_string());
                continue;
            }
        }
        partes.push(t);
    }
    (partes.join(" "), fecha)
}

pub fn tareas_de(nota: &Nota) -> Vec<Tarea> {
    lineas_de_texto(&nota.cuerpo)
        .filter_map(|(n, l)| {
            let (hecha, texto, _) = tarea_en(l)?;
            let (texto, fecha) = separar_fecha(texto);
            if texto.is_empty() && fecha.is_none() {
                return None; // casilla vacía (p. ej. recién escrita)
            }
            Some(Tarea {
                id: nota.resumen.id.clone(),
                titulo: nota.resumen.titulo.clone(),
                proyecto: nota.resumen.proyecto.clone(),
                linea: n,
                hecha,
                texto,
                fecha,
            })
        })
        .collect()
}

/// Marca o desmarca la tarea de la línea `linea` del cuerpo. `texto` es el que se mostró al
/// usuario: si la línea cambió desde entonces, no se toca nada.
pub fn marcar_en_archivo(archivo: &str, linea: usize, texto: &str, hecha: bool) -> Result<String> {
    let n = frontmatter::lineas_antes_del_cuerpo(archivo) + linea;
    let mut lineas: Vec<&str> = archivo.split_inclusive('\n').collect();
    let Some(original) = lineas.get(n).copied() else { bail!("la tarea ya no existe") };
    let sin_fin = original.trim_end_matches(['\n', '\r']);
    let Some((_, actual, pos)) = tarea_en(sin_fin) else { bail!("la línea ya no es una tarea") };
    if separar_fecha(actual).0 != texto {
        bail!("la tarea cambió; vuelve a cargar la lista");
    }
    let nueva = format!("{}{}{}", &original[..pos], if hecha { 'x' } else { ' ' }, &original[pos + 1..]);
    lineas[n] = &nueva;
    Ok(lineas.concat())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::store::{Lugar, NotaResumen};

    fn nota(id: &str, proyecto: &str, titulo: &str, cuerpo: &str) -> Nota {
        Nota {
            resumen: NotaResumen {
                id: id.into(),
                proyecto: proyecto.into(),
                lugar: Lugar::Equipo,
                titulo: titulo.into(),
                etiquetas: vec![],
                modificado: 0,
                extracto: String::new(),
            },
            cuerpo: cuerpo.into(),
            hoja: None,
            pauta: None,
        }
    }

    #[test]
    fn encuentra_wikilinks_fuera_de_codigo() {
        let c = "Ver [[Mi Nota]] y [[otra|alias]] y [[Tercera#Sección]].\n`[[no]]`\n```\n[[tampoco]]\n```\n[[ ]] [[a[b]]";
        let d: Vec<String> = wikilinks(c).into_iter().map(|w| w.destino).collect();
        assert_eq!(d, ["Mi Nota", "otra", "Tercera"]);
    }

    #[test]
    fn resuelve_por_titulo_o_archivo_y_prefiere_el_mismo_proyecto() {
        let notas = [
            nota("equipo:a/idea.md", "equipo:a", "Idea", ""),
            nota("equipo:b/idea.md", "equipo:b", "Idea", ""),
            nota("equipo:b/diseno-web.md", "equipo:b", "Diseño Web", ""),
        ];
        let r = Resolutor::nuevo(&notas);
        assert_eq!(r.resolver("idea", "equipo:b"), Some("equipo:b/idea.md"));
        assert_eq!(r.resolver("IDEA", "equipo:z"), Some("equipo:a/idea.md"));
        assert_eq!(r.resolver("diseño web", "equipo:a"), Some("equipo:b/diseno-web.md"));
        assert_eq!(r.resolver("diseno-web", "equipo:a"), Some("equipo:b/diseno-web.md"));
        assert_eq!(r.resolver("no existe", "equipo:a"), None);
    }

    #[test]
    fn enlaces_entrantes_y_salientes() {
        let notas = [
            nota("equipo:p/a.md", "equipo:p", "A", "Enlaza a [[B]] y a [[Nada]]"),
            nota("equipo:p/b.md", "equipo:p", "B", "Sin enlaces"),
            nota("equipo:p/c.md", "equipo:p", "C", "primera\n  - lee [[b|la nota B]] luego"),
        ];
        let e = enlaces(&notas, "equipo:p/b.md");
        assert!(e.salientes.is_empty());
        let entrantes: Vec<(&str, &str, usize)> = e.entrantes.iter().map(|x| (x.titulo.as_str(), x.contexto.as_str(), x.linea)).collect();
        assert_eq!(entrantes, [("A", "Enlaza a [[B]] y a [[Nada]]", 0), ("C", "- lee [[b|la nota B]] luego", 1)]);
        let e = enlaces(&notas, "equipo:p/a.md");
        let s: Vec<(&str, Option<&str>)> = e.salientes.iter().map(|x| (x.destino.as_str(), x.id.as_deref())).collect();
        assert_eq!(s, [("B", Some("equipo:p/b.md")), ("Nada", None)]);
        assert_eq!(contar_a(&notas, "b"), 2);
    }

    #[test]
    fn renombrar_conserva_alias_y_seccion() {
        let t = "---\ntitle: X\n---\n[[Vieja]] [[vieja|alias]] [[Vieja#Parte]] [[Otra]]\n```\n[[Vieja]]\n```\n`[[Vieja]]`\n";
        let r = reemplazar_destino(t, "Vieja", "Nueva").unwrap();
        assert_eq!(r, "---\ntitle: X\n---\n[[Nueva]] [[Nueva|alias]] [[Nueva#Parte]] [[Otra]]\n```\n[[Vieja]]\n```\n`[[Vieja]]`\n");
        assert!(reemplazar_destino("sin enlaces", "Vieja", "Nueva").is_none());
    }

    #[test]
    fn tareas_con_fechas() {
        let n = nota(
            "equipo:p/t.md",
            "equipo:p",
            "T",
            "- [ ] Llamar a Ana 📅 2026-12-01\n  * [x] Hecha @2026-11-20\n- [ ]\n- [ ] Sin fecha @ana\n- [y] no\n-[ ] no\n```\n- [ ] en código\n```",
        );
        let t: Vec<(usize, bool, String, Option<String>)> =
            tareas_de(&n).into_iter().map(|t| (t.linea, t.hecha, t.texto, t.fecha)).collect();
        let s = |x: &str| x.to_string();
        assert_eq!(
            t,
            [
                (0, false, s("Llamar a Ana"), Some(s("2026-12-01"))),
                (1, true, s("Hecha"), Some(s("2026-11-20"))),
                (3, false, s("Sin fecha @ana"), None),
            ]
        );
    }

    #[test]
    fn marcar_tarea_respeta_front_matter_y_comprueba_el_texto() {
        let archivo = "---\ntitle: T\n---\nIntro\n- [ ] Llamar 📅 2026-12-01\r\n- [x] Otra\n";
        let r = marcar_en_archivo(archivo, 1, "Llamar", true).unwrap();
        assert_eq!(r, "---\ntitle: T\n---\nIntro\n- [x] Llamar 📅 2026-12-01\r\n- [x] Otra\n");
        let r = marcar_en_archivo(&r, 2, "Otra", false).unwrap();
        assert!(r.ends_with("- [ ] Otra\n"));
        assert!(marcar_en_archivo(archivo, 1, "Otro texto", true).is_err());
        assert!(marcar_en_archivo(archivo, 0, "Intro", true).is_err());
        // Sin front matter
        assert_eq!(marcar_en_archivo("- [ ] a\n", 0, "a", true).unwrap(), "- [x] a\n");
    }
}
