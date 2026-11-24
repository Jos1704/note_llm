//! Front matter YAML mínimo para las notas.
//!
//! Solo se interpretan `title` y `tags`; cualquier otra clave se conserva tal cual
//! al volver a guardar, para no romper notas creadas con otras herramientas.
//!
//! ```text
//! ---
//! title: "Mi nota"
//! tags: [trabajo, rust]
//! ---
//!
//! Cuerpo en Markdown…
//! ```

#[derive(Debug, Default, PartialEq)]
pub struct Documento {
    pub titulo: Option<String>,
    pub etiquetas: Vec<String>,
    /// Estilo de hoja de la nota (`blanca`, `negra` o `amarilla`).
    pub hoja: Option<String>,
    /// Pauta de la hoja (`ninguna`, `rayas` o `puntos`).
    pub pauta: Option<String>,
    /// Líneas del front matter que no son `title`, `tags`, `hoja` ni `pauta`.
    pub extra: Vec<String>,
    pub cuerpo: String,
}

pub fn parsear(texto: &str) -> Documento {
    let Some((cabecera, cuerpo)) = separar(texto) else {
        return Documento { cuerpo: texto.to_string(), ..Default::default() };
    };

    let mut doc = Documento { cuerpo: cuerpo.to_string(), ..Default::default() };
    let lineas: Vec<&str> = cabecera.lines().collect();
    let mut i = 0;
    while i < lineas.len() {
        let linea = lineas[i];
        // Líneas de continuación (indentadas o de lista) de la clave actual.
        let mut fin = i + 1;
        while fin < lineas.len() && es_continuacion(lineas[fin]) {
            fin += 1;
        }
        match clave_valor(linea) {
            Some(("title", valor)) => doc.titulo = Some(escalar(valor)),
            Some(("hoja", valor)) => doc.hoja = opcional(escalar(valor)),
            Some(("pauta", valor)) => doc.pauta = opcional(escalar(valor)),
            Some(("tags", valor)) => {
                doc.etiquetas = if valor.is_empty() {
                    lineas[i + 1..fin]
                        .iter()
                        .filter_map(|l| l.trim().strip_prefix('-'))
                        .map(escalar)
                        .collect()
                } else if let Some(lista) = valor.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
                    lista.split(',').map(escalar).collect()
                } else {
                    valor.split(',').map(escalar).collect()
                };
                doc.etiquetas.retain(|e| !e.is_empty());
            }
            _ => doc.extra.extend(lineas[i..fin].iter().map(|l| l.to_string())),
        }
        i = fin;
    }
    doc
}

pub fn serializar(
    titulo: &str,
    etiquetas: &[String],
    hoja: Option<&str>,
    pauta: Option<&str>,
    extra: &[String],
    cuerpo: &str,
) -> String {
    let mut out = String::from("---\n");
    out.push_str(&format!("title: {}\n", serde_json::to_string(titulo).unwrap_or_default()));
    out.push_str(&format!("tags: [{}]\n", etiquetas.join(", ")));
    if let Some(h) = hoja {
        out.push_str(&format!("hoja: {}\n", serde_json::to_string(h).unwrap_or_default()));
    }
    if let Some(p) = pauta {
        out.push_str(&format!("pauta: {}\n", serde_json::to_string(p).unwrap_or_default()));
    }
    for linea in extra {
        out.push_str(linea);
        out.push('\n');
    }
    out.push_str("---\n\n");
    out.push_str(cuerpo);
    out
}

/// Devuelve (cabecera, cuerpo) si el texto empieza con un bloque `---`.
/// Número de líneas del archivo antes del cuerpo (front matter incluido). Sirve para pasar de
/// una línea del cuerpo a una línea del archivo.
pub fn lineas_antes_del_cuerpo(texto: &str) -> usize {
    match separar(texto) {
        Some((_, cuerpo)) => texto[..texto.len() - cuerpo.len()].matches('\n').count(),
        None => 0,
    }
}

fn separar(texto: &str) -> Option<(&str, &str)> {
    let resto = texto.strip_prefix("---\n").or_else(|| texto.strip_prefix("---\r\n"))?;
    let mut pos = 0;
    for linea in resto.split_inclusive('\n') {
        let limpia = linea.trim_end();
        if limpia == "---" || limpia == "..." {
            let cuerpo = &resto[pos + linea.len()..];
            let cuerpo = cuerpo
                .strip_prefix("\r\n")
                .or_else(|| cuerpo.strip_prefix('\n'))
                .unwrap_or(cuerpo);
            return Some((&resto[..pos], cuerpo));
        }
        pos += linea.len();
    }
    None
}

fn es_continuacion(linea: &str) -> bool {
    linea.starts_with([' ', '\t']) || linea.starts_with("- ") || linea.trim().is_empty()
}

fn clave_valor(linea: &str) -> Option<(&str, &str)> {
    let (clave, valor) = linea.split_once(':')?;
    let valida = !clave.is_empty()
        && clave.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    valida.then(|| (clave, valor.trim()))
}

/// Convierte una cadena a `Some` solo si no está vacía.
fn opcional(valor: String) -> Option<String> {
    if valor.is_empty() { None } else { Some(valor) }
}

fn escalar(valor: &str) -> String {
    let v = valor.trim();
    if v.starts_with('"') {
        if let Ok(s) = serde_json::from_str::<String>(v) {
            return s;
        }
        return v.trim_matches('"').to_string();
    }
    if let Some(s) = v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return s.replace("''", "'");
    }
    v.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_front_matter() {
        let d = parsear("# Hola\n\ntexto");
        assert_eq!(d.titulo, None);
        assert_eq!(d.cuerpo, "# Hola\n\ntexto");
    }

    #[test]
    fn lista_en_linea_y_extra() {
        let d = parsear("---\ntitle: \"Nota: \\\"uno\\\"\"\ntags: [a, b]\nhoja: amarilla\npauta: puntos\nautor: yo\n---\n\ncuerpo\n");
        assert_eq!(d.titulo.as_deref(), Some("Nota: \"uno\""));
        assert_eq!(d.etiquetas, vec!["a", "b"]);
        assert_eq!(d.hoja.as_deref(), Some("amarilla"));
        assert_eq!(d.pauta.as_deref(), Some("puntos"));
        assert_eq!(d.extra, vec!["autor: yo"]);
        assert_eq!(d.cuerpo, "cuerpo\n");
    }

    #[test]
    fn lista_en_bloque() {
        let d = parsear("---\ntitle: Plano\ntags:\n  - uno\n  - 'dos'\n---\nx");
        assert_eq!(d.titulo.as_deref(), Some("Plano"));
        assert_eq!(d.etiquetas, vec!["uno", "dos"]);
        assert_eq!(d.cuerpo, "x");
    }

    #[test]
    fn ida_y_vuelta() {
        let texto = serializar(
            "Título \"raro\"",
            &["x".into(), "y".into()],
            Some("negra"),
            Some("rayas"),
            &["autor: yo".into()],
            "# Cuerpo\n",
        );
        let d = parsear(&texto);
        assert_eq!(d.titulo.as_deref(), Some("Título \"raro\""));
        assert_eq!(d.etiquetas, vec!["x", "y"]);
        assert_eq!(d.hoja.as_deref(), Some("negra"));
        assert_eq!(d.pauta.as_deref(), Some("rayas"));
        assert_eq!(d.extra, vec!["autor: yo"]);
        assert_eq!(d.cuerpo, "# Cuerpo\n");
    }
}
