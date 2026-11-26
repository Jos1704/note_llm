//! Nota rápida sin ventanas, para paneles externos (el de Quickshell en Linux) y scripts.
//!
//! ```text
//! md-notes --proyectos                         → [{"id":"equipo:trabajo","nombre":"Trabajo"}, …]
//! md-notes --etiquetas                         → [{"nombre":"rust","color":"#6e9c84"}, …]
//! md-notes --portapapeles                      → {"tipo":"texto","texto":"…"} | {"tipo":"imagen","nombre":"…"} | {"tipo":"vacio"}
//! md-notes --seleccion                         → {"texto":"…"}
//! md-notes --captura-pantalla                  → {"nombre":"…","texto":"…"} | null (cancelada)
//! md-notes --guardar-rapida [--titulo T] [--proyecto ID] -- CUERPO   → {"titulo":"…"}
//! ```
//!
//! `--guardar-rapida` aplica lo mismo que la captura de la app (`crearNotaRapida` en `api.ts`):
//! prefijos `#etiqueta` / `@proyecto`, imágenes `adjunto:<nombre>` movidas a `assets/` y título por
//! defecto tomado de la primera línea. Los errores salen por stderr con código 1.

use serde::Serialize;

use crate::biblioteca::Biblioteca;
use crate::captura;
use crate::store::{Lugar, Proyecto, normalizar_etiqueta, slug};

pub const ARGS: &[&str] = &["--proyectos", "--etiquetas", "--portapapeles", "--seleccion", "--captura-pantalla", "--guardar-rapida"];

/// Si los argumentos piden una orden de esta línea de comandos, la ejecuta y devuelve `true`.
pub fn atender(args: &[String]) -> bool {
    let Some(orden) = args.iter().find(|a| ARGS.contains(&a.as_str())) else { return false };
    let r = match orden.as_str() {
        "--proyectos" => proyectos(),
        "--etiquetas" => etiquetas(),
        "--portapapeles" => json(&captura::portapapeles()),
        "--seleccion" => json(&captura::texto_seleccionado().map(|texto| serde_json::json!({ "texto": texto }))),
        "--captura-pantalla" => json(&captura::capturar_a_temporal()),
        _ => guardar(args),
    };
    match r {
        Ok(salida) => println!("{salida}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
    true
}

/// Linux: abre el panel de Quickshell (`qs -c notas`) si está corriendo. En macOS, `false`.
pub fn abrir_panel_externo() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("qs")
            .args(["-c", "notas", "ipc", "call", "notas", "alternar"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }
    #[cfg(not(target_os = "linux"))]
    false
}

fn json<T: Serialize>(r: &Result<T, String>) -> Result<String, String> {
    let v = r.as_ref().map_err(|e| e.clone())?;
    serde_json::to_string(v).map_err(|e| e.to_string())
}

fn biblioteca() -> Result<Biblioteca, String> {
    Biblioteca::abrir().map_err(|e| format!("{e:#}"))
}

fn lista_proyectos(b: &Biblioteca) -> Result<Vec<Proyecto>, String> {
    Ok(b.estado().map_err(|e| format!("{e:#}"))?.proyectos)
}

fn proyectos() -> Result<String, String> {
    let b = biblioteca()?;
    let lista: Vec<_> = lista_proyectos(&b)?
        .into_iter()
        .map(|p| serde_json::json!({ "id": p.id, "nombre": p.nombre, "disco": p.lugar == Lugar::Disco }))
        .collect();
    serde_json::to_string(&lista).map_err(|e| e.to_string())
}

/// Etiquetas existentes, para las sugerencias de `#etiqueta` del panel.
fn etiquetas() -> Result<String, String> {
    let b = biblioteca()?;
    let lista: Vec<_> = b
        .estado()
        .map_err(|e| format!("{e:#}"))?
        .etiquetas
        .into_iter()
        .map(|e| serde_json::json!({ "nombre": e.nombre, "color": e.color }))
        .collect();
    serde_json::to_string(&lista).map_err(|e| e.to_string())
}

fn valor<'a>(args: &'a [String], nombre: &str) -> Option<&'a str> {
    let fin = args.iter().position(|a| a == "--").unwrap_or(args.len());
    args[..fin].windows(2).find(|w| w[0] == nombre).map(|w| w[1].as_str())
}

fn guardar(args: &[String]) -> Result<String, String> {
    let titulo = valor(args, "--titulo").unwrap_or("");
    let elegido = valor(args, "--proyecto").filter(|p| !p.is_empty());
    let cuerpo = match args.iter().position(|a| a == "--") {
        Some(i) => args[i + 1..].join(" "),
        None => String::new(),
    };
    if titulo.trim().is_empty() && cuerpo.trim().is_empty() {
        return Err("La nota está vacía".into());
    }
    let b = biblioteca()?;
    let proyectos = lista_proyectos(&b)?;
    let p = extraer_prefijos(titulo, &cuerpo, &proyectos);
    let proyecto = match &p.proyecto {
        Some(p) => p.as_deref(),
        None => elegido,
    };
    let titulo = Some(p.titulo.trim().to_string())
        .filter(|t| !t.is_empty())
        .or_else(|| titulo_por_defecto(&p.cuerpo))
        .unwrap_or_else(|| "Nota rápida".into());

    let err = |e: anyhow::Error| format!("{e:#}");
    let nota = b.crear_nota(proyecto, &p.etiquetas).map_err(err)?;
    let ruta = b.ruta(Some(&nota.resumen.id), Lugar::Equipo).map_err(err)?;
    let mut cuerpo = p.cuerpo;
    for nombre in adjuntos(&cuerpo) {
        if let Ok(rel) = captura::adjuntar_a(&ruta, &nombre) {
            cuerpo = cuerpo.replace(&format!("(adjunto:{nombre})"), &format!("({rel})"));
        }
    }
    let guardada = b.guardar_nota(&nota.resumen.id, &titulo, &nota.resumen.etiquetas, None, None, &cuerpo).map_err(err)?;
    Ok(serde_json::json!({ "titulo": titulo, "id": guardada.id }).to_string())
}

fn adjuntos(cuerpo: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for parte in cuerpo.split("(adjunto:").skip(1) {
        let nombre: String = parte.chars().take_while(|c| *c != ')' && !c.is_whitespace()).collect();
        if !nombre.is_empty() && parte[nombre.len()..].starts_with(')') && !out.contains(&nombre) {
            out.push(nombre);
        }
    }
    out
}

/// Primera línea con texto (sin marcas de Markdown ni casillas), hasta 60 caracteres.
fn titulo_por_defecto(cuerpo: &str) -> Option<String> {
    let linea = cuerpo.lines().find(|l| {
        let t = l.trim_start();
        !t.is_empty() && !t.starts_with("![") && !t.starts_with('<')
    })?;
    let t = linea.trim_start_matches(['#', '>', '-', '*', '+', ' ', '\t']);
    let t = ["[ ]", "[x]", "[X]"].iter().find_map(|c| t.strip_prefix(c)).map(str::trim_start).unwrap_or(t);
    Some(t.chars().take(60).collect::<String>().trim().to_string()).filter(|t| !t.is_empty())
}

// ------------------------------------------------------------------ prefijos (port de `prefijos.ts`)

#[derive(Debug, PartialEq)]
pub struct Prefijos {
    pub titulo: String,
    pub cuerpo: String,
    pub etiquetas: Vec<String>,
    /// `None`: no había `@proyecto`. `Some(None)`: la Bandeja.
    pub proyecto: Option<Option<String>>,
}

fn buscar_proyecto<'a>(nombre: &str, proyectos: &'a [Proyecto]) -> Option<&'a Proyecto> {
    let s = slug(nombre);
    if s.is_empty() {
        return None;
    }
    proyectos
        .iter()
        .find(|p| slug(&p.nombre) == s)
        .or_else(|| proyectos.iter().find(|p| slug(p.id.split(':').nth(1).unwrap_or("")) == s))
}

fn extraer_de_linea(linea: &str, proyectos: &[Proyecto], r: &mut Prefijos) -> String {
    let cs: Vec<char> = linea.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        let inicio = i == 0 || cs[i - 1].is_whitespace();
        let signo = cs[i];
        if inicio && (signo == '#' || signo == '@') && cs.get(i + 1).is_some_and(|c| c.is_alphabetic() || *c == '_') {
            let mut fin = i + 1;
            while fin < cs.len() && (cs[fin].is_alphanumeric() || "_-/.".contains(cs[fin])) {
                fin += 1;
            }
            let nombre: String = cs[i + 1..fin].iter().collect();
            let aplicado = if signo == '#' {
                let e = normalizar_etiqueta(&nombre);
                if !e.is_empty() && !r.etiquetas.contains(&e) {
                    r.etiquetas.push(e.clone());
                }
                !e.is_empty()
            } else if let Some(p) = buscar_proyecto(&nombre, proyectos) {
                r.proyecto = Some((!p.id.ends_with(":bandeja")).then(|| p.id.clone()));
                true
            } else {
                false
            };
            if aplicado {
                i = fin;
                continue;
            }
            out.extend(&cs[i..fin]);
            i = fin;
            continue;
        }
        out.push(signo);
        i += 1;
    }
    // Espacios repetidos que deja quitar un token.
    let mut limpio = String::new();
    for c in out.chars() {
        if (c == ' ' || c == '\t') && limpio.ends_with([' ', '\t']) {
            limpio.pop();
            limpio.push(' ');
            continue;
        }
        limpio.push(c);
    }
    limpio.trim().to_string()
}

pub fn extraer_prefijos(titulo: &str, cuerpo: &str, proyectos: &[Proyecto]) -> Prefijos {
    let mut r = Prefijos { titulo: String::new(), cuerpo: String::new(), etiquetas: Vec::new(), proyecto: None };
    r.titulo = extraer_de_linea(titulo, proyectos, &mut r);
    let mut lineas: Vec<String> = cuerpo.split('\n').map(str::to_string).collect();
    if let Some(i) = lineas.iter().position(|l| !l.trim().is_empty()) {
        let t = lineas[i].trim_start();
        let especial = t.starts_with("![") || t.starts_with('<') || t.starts_with("```") || t.starts_with("# ") || t == "#";
        if !especial {
            let limpia = extraer_de_linea(&lineas[i], proyectos, &mut r);
            if !limpia.is_empty() {
                lineas[i] = limpia;
            } else {
                lineas.remove(i);
                if i < lineas.len() && lineas[i].trim().is_empty() {
                    lineas.remove(i);
                }
            }
        }
    }
    r.cuerpo = lineas.join("\n");
    r
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn proyectos() -> Vec<Proyecto> {
        let p = |id: &str, nombre: &str| Proyecto { id: id.into(), lugar: Lugar::Equipo, nombre: nombre.into(), notas: 0 };
        vec![p("equipo:bandeja", "Bandeja"), p("equipo:mi-app", "Mi App"), p("equipo:diseno", "Diseño")]
    }

    #[test]
    fn prefijos_como_en_la_app() {
        let ps = proyectos();
        let r = extraer_prefijos("Idea #Rust", "@diseno #ui\n\nTexto con #nota y user@mail.com", &ps);
        assert_eq!(r.titulo, "Idea");
        assert_eq!(r.etiquetas, ["rust", "ui"]);
        assert_eq!(r.proyecto, Some(Some("equipo:diseno".into())));
        assert_eq!(r.cuerpo, "Texto con #nota y user@mail.com");

        let r = extraer_prefijos("", "Llamar @mi-app mañana @nadie", &ps);
        assert_eq!(r.proyecto, Some(Some("equipo:mi-app".into())));
        assert_eq!(r.cuerpo, "Llamar mañana @nadie");

        let r = extraer_prefijos("", "@bandeja hola", &ps);
        assert_eq!(r.proyecto, Some(None));

        // `# Título` es un encabezado, no una etiqueta.
        let r = extraer_prefijos("", "# Título #x", &ps);
        assert!(r.etiquetas.is_empty());
        assert_eq!(r.proyecto, None);
    }

    #[test]
    fn titulo_y_adjuntos() {
        assert_eq!(titulo_por_defecto("\n![x](adjunto:a.png)\n- [ ] Comprar pan").as_deref(), Some("Comprar pan"));
        assert_eq!(titulo_por_defecto("## Hola").as_deref(), Some("Hola"));
        assert_eq!(adjuntos("![a](adjunto:a.png) ![b](adjunto:b.png) ![a](adjunto:a.png) (adjunto:mal"), ["a.png", "b.png"]);
    }
}
