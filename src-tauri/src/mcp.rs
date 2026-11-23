//! Servidor MCP por stdio: `md-notes --mcp`.
//!
//! Permite que agentes de IA (Claude Code, Codex, opencode, Cursor…) lean y dejen
//! contexto en los proyectos de notas, sin necesidad de que la app esté abierta.
//! Trabaja directamente sobre los archivos, igual que la interfaz; la app recoge los
//! cambios al volver a enfocarla.
//!
//! Protocolo: JSON-RPC 2.0, un mensaje por línea en stdin/stdout. Nada más debe
//! escribirse en stdout; los avisos van a stderr.

use std::io::{self, BufRead, Write};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::biblioteca::{Biblioteca, Estado};
use crate::store::{Lugar, NotaResumen, Proyecto};

const VERSIONES: [&str; 4] = ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];
const TITULO_CONTEXTO: &str = "Contexto";
const TITULO_BITACORA: &str = "Bitácora";
/// Límite aproximado de caracteres que devuelve `contexto_proyecto` antes de resumir.
const LIMITE_CONTEXTO: usize = 60_000;

const INSTRUCCIONES: &str = "\
Notas es la memoria compartida entre agentes: cada proyecto es una carpeta de notas Markdown.

Flujo recomendado:
1. Al empezar a trabajar en algo, llama a `contexto_proyecto` con el nombre del proyecto \
(usa `listar_proyectos` si no sabes cómo se llama). Si no existe, créalo con `crear_proyecto` \
y pásale en `contexto` un resumen inicial: objetivo, stack, decisiones y pendientes.
2. La nota «Contexto» de cada proyecto es el resumen vivo: mantenla corta y al día con \
`actualizar_nota` (modo `reemplazar`) cuando cambie algo importante.
3. Usa `registrar` para dejar en la «Bitácora» lo que hiciste, decidiste o dejaste pendiente, \
para que el siguiente agente (o tú en otra sesión) pueda continuar.
4. Para temas largos (diseño, investigación, guías) crea notas aparte con `crear_nota`.

No copies secretos (tokens, contraseñas, claves) en las notas.";

pub fn servir() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for linea in stdin.lock().lines() {
        let linea = linea?;
        if linea.trim().is_empty() {
            continue;
        }
        let respuesta = match serde_json::from_str::<Value>(&linea) {
            Ok(Value::Array(lote)) => {
                let r: Vec<Value> = lote.iter().filter_map(atender).collect();
                (!r.is_empty()).then(|| Value::Array(r))
            }
            Ok(msg) => atender(&msg),
            Err(e) => Some(error_rpc(Value::Null, -32700, &format!("JSON inválido: {e}"))),
        };
        if let Some(r) = respuesta {
            writeln!(stdout, "{r}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

/// Responde a una petición; `None` para notificaciones y respuestas.
fn atender(msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned()?;
    let Some(metodo) = msg.get("method").and_then(Value::as_str) else {
        return None; // respuesta del cliente a algo que no pedimos
    };
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let resultado = match metodo {
        "initialize" => Ok(inicializar(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": herramientas() })),
        "tools/call" => Ok(llamar(&params)),
        "resources/list" => Ok(json!({ "resources": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        otro => Err((-32601, format!("método no soportado: {otro}"))),
    };
    Some(match resultado {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err((codigo, m)) => error_rpc(id, codigo, &m),
    })
}

fn error_rpc(id: Value, codigo: i64, mensaje: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": codigo, "message": mensaje } })
}

fn inicializar(params: &Value) -> Value {
    let pedida = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
    let version = if VERSIONES.contains(&pedida) { pedida } else { VERSIONES[VERSIONES.len() - 1] };
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "md-notes", "title": "Notas", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCCIONES,
    })
}

// ------------------------------------------------------------------ herramientas

fn herramientas() -> Value {
    let proyecto = json!({
        "type": "string",
        "description": "Nombre del proyecto (p. ej. «Mi app») o su id (p. ej. «equipo:mi-app»)."
    });
    let etiquetas = json!({ "type": "array", "items": { "type": "string" }, "description": "Etiquetas de la nota." });
    let solo_lectura = json!({ "readOnlyHint": true, "openWorldHint": false });
    let escritura = json!({ "readOnlyHint": false, "destructiveHint": false, "openWorldHint": false });
    json!([
        {
            "name": "listar_proyectos",
            "title": "Listar proyectos",
            "description": "Lista los proyectos de notas (nombre, id, lugar y número de notas).",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": solo_lectura,
        },
        {
            "name": "contexto_proyecto",
            "title": "Contexto de un proyecto",
            "description": "Devuelve el contexto completo de un proyecto: primero la nota «Contexto», luego la «Bitácora» y el resto de notas, de la más reciente a la más antigua. Llámala al empezar a trabajar en un proyecto. Si el proyecto no existe, lo indica.",
            "inputSchema": {
                "type": "object",
                "properties": { "proyecto": proyecto },
                "required": ["proyecto"],
            },
            "annotations": solo_lectura,
        },
        {
            "name": "listar_notas",
            "title": "Listar notas",
            "description": "Lista notas (id, título, etiquetas, extracto), opcionalmente filtradas por proyecto y/o etiqueta.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "proyecto": proyecto,
                    "etiqueta": { "type": "string" },
                    "limite": { "type": "integer", "minimum": 1, "description": "Máximo de notas (50 por defecto)." },
                },
            },
            "annotations": solo_lectura,
        },
        {
            "name": "buscar",
            "title": "Buscar en las notas",
            "description": "Busca texto (sin distinguir mayúsculas) en el título, etiquetas y contenido de todas las notas.",
            "inputSchema": {
                "type": "object",
                "properties": { "texto": { "type": "string" }, "proyecto": proyecto },
                "required": ["texto"],
            },
            "annotations": solo_lectura,
        },
        {
            "name": "leer_nota",
            "title": "Leer nota",
            "description": "Devuelve el contenido Markdown completo de una nota por su id.",
            "inputSchema": {
                "type": "object",
                "properties": { "id": { "type": "string", "description": "Id de la nota, p. ej. «equipo:mi-app/contexto.md»." } },
                "required": ["id"],
            },
            "annotations": solo_lectura,
        },
        {
            "name": "crear_proyecto",
            "title": "Crear proyecto",
            "description": "Crea un proyecto si no existe (si ya existe, lo devuelve sin cambiar nada). Con `contexto`, crea también su nota «Contexto» con ese resumen inicial.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "nombre": { "type": "string" },
                    "contexto": { "type": "string", "description": "Markdown: objetivo, stack, decisiones, pendientes…" },
                    "lugar": { "type": "string", "enum": ["equipo", "disco"], "description": "«disco» (externo, si está conectado) o «equipo» (esta computadora). Por defecto, el disco si está conectado." },
                },
                "required": ["nombre"],
            },
            "annotations": escritura,
        },
        {
            "name": "crear_nota",
            "title": "Crear nota",
            "description": "Crea una nota nueva en un proyecto existente.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "proyecto": proyecto,
                    "titulo": { "type": "string" },
                    "cuerpo": { "type": "string", "description": "Contenido en Markdown (sin front matter)." },
                    "etiquetas": etiquetas,
                },
                "required": ["proyecto", "titulo", "cuerpo"],
            },
            "annotations": escritura,
        },
        {
            "name": "actualizar_nota",
            "title": "Actualizar nota",
            "description": "Modifica una nota existente. `modo`: «reemplazar» sustituye el cuerpo; «anexar» lo agrega al final. Los campos omitidos no cambian. El id puede cambiar si cambia el título.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "cuerpo": { "type": "string" },
                    "modo": { "type": "string", "enum": ["reemplazar", "anexar"], "description": "Por defecto, «reemplazar»." },
                    "titulo": { "type": "string" },
                    "etiquetas": etiquetas,
                },
                "required": ["id"],
            },
            "annotations": json!({ "readOnlyHint": false, "destructiveHint": true, "idempotentHint": false, "openWorldHint": false }),
        },
        {
            "name": "registrar",
            "title": "Registrar en la bitácora",
            "description": "Agrega una entrada fechada a la nota «Bitácora» del proyecto (la crea si no existe): qué se hizo, qué se decidió y qué queda pendiente.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "proyecto": proyecto,
                    "entrada": { "type": "string", "description": "Markdown breve." },
                    "agente": { "type": "string", "description": "Quién escribe, p. ej. «Claude Code» o «Codex»." },
                },
                "required": ["proyecto", "entrada"],
            },
            "annotations": escritura,
        },
    ])
}

fn llamar(params: &Value) -> Value {
    let nombre = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let r = Biblioteca::abrir().and_then(|b| ejecutar(&b, nombre, &args));
    match r {
        Ok(texto) => json!({ "content": [{ "type": "text", "text": texto }] }),
        Err(e) => json!({ "content": [{ "type": "text", "text": format!("Error: {e:#}") }], "isError": true }),
    }
}

fn ejecutar(b: &Biblioteca, herramienta: &str, a: &Value) -> Result<String> {
    match herramienta {
        "listar_proyectos" => listar_proyectos(b),
        "contexto_proyecto" => contexto_proyecto(b, texto(a, "proyecto")?),
        "listar_notas" => listar_notas(b, opcional(a, "proyecto"), opcional(a, "etiqueta"), a.get("limite").and_then(Value::as_u64)),
        "buscar" => buscar(b, texto(a, "texto")?, opcional(a, "proyecto")),
        "leer_nota" => leer_nota(b, texto(a, "id")?),
        "crear_proyecto" => crear_proyecto(b, texto(a, "nombre")?, opcional(a, "contexto"), opcional(a, "lugar")),
        "crear_nota" => {
            let p = buscar_proyecto(&b.estado()?, texto(a, "proyecto")?)?.id.clone();
            let n = nueva_nota(b, &p, texto(a, "titulo")?, texto(a, "cuerpo")?, &lista(a, "etiquetas"))?;
            Ok(format!("Nota creada: «{}» ({})", n.titulo, n.id))
        }
        "actualizar_nota" => actualizar_nota(b, a),
        "registrar" => registrar(b, texto(a, "proyecto")?, texto(a, "entrada")?, opcional(a, "agente")),
        otra => bail!("herramienta desconocida: {otra}"),
    }
}

fn texto<'a>(a: &'a Value, clave: &str) -> Result<&'a str> {
    opcional(a, clave).with_context(|| format!("falta el argumento «{clave}»"))
}

fn opcional<'a>(a: &'a Value, clave: &str) -> Option<&'a str> {
    a.get(clave).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty())
}

fn lista(a: &Value, clave: &str) -> Vec<String> {
    a.get(clave)
        .and_then(Value::as_array)
        .map(|v| v.iter().filter_map(Value::as_str).map(String::from).collect())
        .unwrap_or_default()
}

// ------------------------------------------------------------------ implementación

fn listar_proyectos(b: &Biblioteca) -> Result<String> {
    let est = b.estado()?;
    if est.proyectos.is_empty() {
        return Ok("No hay proyectos todavía. Crea uno con `crear_proyecto`.".into());
    }
    let mut out = String::from("Proyectos:\n");
    for p in &est.proyectos {
        out.push_str(&format!("- {} — id: `{}` · {} · {} notas\n", p.nombre, p.id, lugar(p.lugar), p.notas));
    }
    if let Some(d) = est.disco.as_ref().filter(|d| !d.conectado) {
        out.push_str(&format!("\n(El disco {} no está conectado: sus proyectos no aparecen.)\n", d.ruta));
    }
    Ok(out)
}

fn contexto_proyecto(b: &Biblioteca, proyecto: &str) -> Result<String> {
    let est = b.estado()?;
    let Some(p) = encontrar_proyecto(&est, proyecto) else {
        return Ok(format!(
            "No existe el proyecto «{proyecto}». Créalo con `crear_proyecto` (y un `contexto` inicial) si vas a trabajar en él.\n\n{}",
            listar_proyectos(b)?
        ));
    };
    let mut notas: Vec<&NotaResumen> = est.notas.iter().filter(|n| n.proyecto == p.id).collect();
    // Ya vienen de la más reciente a la más antigua; «Contexto» y «Bitácora» primero.
    notas.sort_by_key(|n| prioridad(&n.titulo));

    let mut out = format!("# Proyecto: {} (`{}`, {})\n\n", p.nombre, p.id, lugar(p.lugar));
    if notas.is_empty() {
        out.push_str("El proyecto está vacío. Escribe su nota «Contexto» con `crear_nota`.\n");
        return Ok(out);
    }
    let mut resumidas = Vec::new();
    for n in notas {
        if out.len() > LIMITE_CONTEXTO {
            resumidas.push(n);
            continue;
        }
        let nota = b.leer_nota(&n.id)?;
        out.push_str(&format!("---\n\n## {}\n\n{}\n\n{}\n\n", n.titulo, cabecera(n), nota.cuerpo.trim()));
    }
    if !resumidas.is_empty() {
        out.push_str("---\n\nOtras notas (léelas con `leer_nota`):\n");
        for n in resumidas {
            out.push_str(&linea_resumen(n));
        }
    }
    Ok(out)
}

fn listar_notas(b: &Biblioteca, proyecto: Option<&str>, etiqueta: Option<&str>, limite: Option<u64>) -> Result<String> {
    let est = b.estado()?;
    let p = proyecto.map(|p| buscar_proyecto(&est, p)).transpose()?;
    let etiqueta = etiqueta.map(crate::store::normalizar_etiqueta);
    let notas: Vec<&NotaResumen> = est
        .notas
        .iter()
        .filter(|n| p.is_none_or(|p| n.proyecto == p.id))
        .filter(|n| etiqueta.as_ref().is_none_or(|e| n.etiquetas.contains(e)))
        .take(limite.unwrap_or(50) as usize)
        .collect();
    if notas.is_empty() {
        return Ok("No hay notas que coincidan.".into());
    }
    Ok(notas.into_iter().map(linea_resumen).collect())
}

fn buscar(b: &Biblioteca, texto: &str, proyecto: Option<&str>) -> Result<String> {
    let est = b.estado()?;
    let p = proyecto.map(|p| buscar_proyecto(&est, p)).transpose()?;
    let aguja = texto.to_lowercase();
    let ids = b.buscar(texto)?;
    let notas: Vec<&NotaResumen> = est
        .notas
        .iter()
        .filter(|n| p.is_none_or(|p| n.proyecto == p.id))
        .filter(|n| {
            ids.contains(&n.id)
                || n.titulo.to_lowercase().contains(&aguja)
                || n.etiquetas.iter().any(|e| e.contains(&aguja))
        })
        .collect();
    if notas.is_empty() {
        return Ok(format!("Sin resultados para «{texto}»."));
    }
    Ok(notas.into_iter().map(linea_resumen).collect())
}

fn leer_nota(b: &Biblioteca, id: &str) -> Result<String> {
    let n = b.leer_nota(id)?;
    Ok(format!("# {}\n\n{}\n\n{}", n.resumen.titulo, cabecera(&n.resumen), n.cuerpo.trim()))
}

fn crear_proyecto(b: &Biblioteca, nombre: &str, contexto: Option<&str>, lugar_pedido: Option<&str>) -> Result<String> {
    let est = b.estado()?;
    if let Some(p) = encontrar_proyecto(&est, nombre) {
        let mut out = format!("El proyecto «{}» ya existía (`{}`); no se cambió nada.", p.nombre, p.id);
        if contexto.is_some() {
            out.push_str(" Para actualizar su contexto usa `contexto_proyecto` y luego `actualizar_nota`.");
        }
        return Ok(out);
    }
    let disco = est.disco.as_ref().is_some_and(|d| d.conectado);
    let lugar = match lugar_pedido {
        Some("equipo") => Lugar::Equipo,
        Some("disco") if !disco => bail!("el disco no está conectado; usa «equipo»"),
        Some("disco") => Lugar::Disco,
        Some(otro) => bail!("lugar desconocido: {otro} (usa «equipo» o «disco»)"),
        None if disco => Lugar::Disco,
        None => Lugar::Equipo,
    };
    let id = b.crear_proyecto(nombre, lugar)?;
    let mut out = format!("Proyecto creado: «{}» (`{id}`, {}).", nombre.trim(), self::lugar(lugar));
    if let Some(c) = contexto {
        let n = nueva_nota(b, &id, TITULO_CONTEXTO, c, &["contexto".into()])?;
        out.push_str(&format!(" Nota de contexto: `{}`.", n.id));
    }
    Ok(out)
}

fn nueva_nota(b: &Biblioteca, proyecto: &str, titulo: &str, cuerpo: &str, etiquetas: &[String]) -> Result<NotaResumen> {
    let n = b.crear_nota(Some(proyecto), etiquetas)?;
    b.guardar_nota(&n.resumen.id, titulo, etiquetas, None, None, &con_salto(cuerpo))
}

fn actualizar_nota(b: &Biblioteca, a: &Value) -> Result<String> {
    let id = texto(a, "id")?;
    let n = b.leer_nota(id)?;
    let titulo = opcional(a, "titulo").unwrap_or(&n.resumen.titulo);
    let etiquetas = if a.get("etiquetas").is_some() { lista(a, "etiquetas") } else { n.resumen.etiquetas.clone() };
    let cuerpo = match (a.get("cuerpo").and_then(Value::as_str), opcional(a, "modo")) {
        (None, _) => n.cuerpo.clone(),
        (Some(c), None | Some("reemplazar")) => con_salto(c),
        (Some(c), Some("anexar")) => format!("{}\n\n{}", n.cuerpo.trim_end(), con_salto(c.trim())),
        (Some(_), Some(otro)) => bail!("modo desconocido: {otro} (usa «reemplazar» o «anexar»)"),
    };
    let r = b.guardar_nota(id, titulo, &etiquetas, n.hoja.as_deref(), n.pauta.as_deref(), &cuerpo)?;
    Ok(format!("Nota actualizada: «{}» ({})", r.titulo, r.id))
}

fn registrar(b: &Biblioteca, proyecto: &str, entrada: &str, agente: Option<&str>) -> Result<String> {
    let est = b.estado()?;
    let p = buscar_proyecto(&est, proyecto)?;
    let ahora = chrono::Local::now().format("%Y-%m-%d %H:%M");
    let quien = agente.map(|a| format!(" · {a}")).unwrap_or_default();
    let bloque = format!("### {ahora}{quien}\n\n{}\n", entrada.trim());

    let existente = est.notas.iter().find(|n| n.proyecto == p.id && clave(&n.titulo) == clave(TITULO_BITACORA));
    let r = match existente {
        Some(n) => {
            let nota = b.leer_nota(&n.id)?;
            let cuerpo = format!("{}\n\n{bloque}", nota.cuerpo.trim_end());
            b.guardar_nota(&n.id, &n.titulo, &n.etiquetas, nota.hoja.as_deref(), nota.pauta.as_deref(), &cuerpo)?
        }
        None => nueva_nota(b, &p.id, TITULO_BITACORA, &bloque, &["bitacora".into()])?,
    };
    Ok(format!("Entrada agregada a la bitácora de «{}» ({}).", p.nombre, r.id))
}

// ------------------------------------------------------------------ utilidades

/// Busca un proyecto por id o por nombre (sin distinguir mayúsculas ni acentos).
/// Si hay dos con el mismo nombre, gana el del disco (viene primero en el estado).
fn encontrar_proyecto<'a>(est: &'a Estado, consulta: &str) -> Option<&'a Proyecto> {
    let consulta = consulta.trim();
    est.proyectos.iter().find(|p| p.id == consulta).or_else(|| {
        let c = clave(consulta);
        est.proyectos.iter().find(|p| clave(&p.nombre) == c || p.id.split_once(':').is_some_and(|(_, carpeta)| carpeta == c))
    })
}

fn buscar_proyecto<'a>(est: &'a Estado, consulta: &str) -> Result<&'a Proyecto> {
    encontrar_proyecto(est, consulta)
        .with_context(|| format!("no existe el proyecto «{consulta}» (créalo con `crear_proyecto`)"))
}

fn clave(texto: &str) -> String {
    crate::store::slug(texto)
}

fn prioridad(titulo: &str) -> u8 {
    match clave(titulo) {
        t if t == clave(TITULO_CONTEXTO) => 0,
        t if t == clave(TITULO_BITACORA) => 1,
        _ => 2,
    }
}

fn lugar(l: Lugar) -> &'static str {
    match l {
        Lugar::Equipo => "este equipo",
        Lugar::Disco => "disco",
    }
}

fn fecha(ms: u64) -> String {
    chrono::DateTime::from_timestamp_millis(ms as i64)
        .map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default()
}

fn cabecera(n: &NotaResumen) -> String {
    let etiquetas = if n.etiquetas.is_empty() {
        String::new()
    } else {
        format!(" · etiquetas: {}", n.etiquetas.join(", "))
    };
    format!("_id: `{}` · modificada: {}{etiquetas}_", n.id, fecha(n.modificado))
}

fn linea_resumen(n: &NotaResumen) -> String {
    let etiquetas = if n.etiquetas.is_empty() { String::new() } else { format!(" [{}]", n.etiquetas.join(", ")) };
    format!("- **{}**{etiquetas} — `{}` · {}\n  {}\n", n.titulo, n.id, fecha(n.modificado), n.extracto)
}

fn con_salto(texto: &str) -> String {
    let t = texto.trim_end();
    if t.is_empty() { String::new() } else { format!("{t}\n") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::dir_temporal;

    #[test]
    fn protocolo_basico() {
        let init = atender(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}})).unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
        let rara = atender(&json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"1999-01-01"}})).unwrap();
        assert_eq!(rara["result"]["protocolVersion"], VERSIONES[VERSIONES.len() - 1]);
        assert!(atender(&json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
        let lista = atender(&json!({"jsonrpc":"2.0","id":3,"method":"tools/list"})).unwrap();
        assert_eq!(lista["result"]["tools"].as_array().unwrap().len(), 9);
        let nada = atender(&json!({"jsonrpc":"2.0","id":4,"method":"no/existe"})).unwrap();
        assert_eq!(nada["error"]["code"], -32601);
    }

    #[test]
    fn flujo_de_un_agente() {
        let base = dir_temporal("mcp");
        let b = Biblioteca::con_rutas(base.join("equipo"), base.join("config/ajustes.json")).unwrap();
        let ej = |h: &str, a: Value| ejecutar(&b, h, &a).unwrap();

        assert!(ej("contexto_proyecto", json!({"proyecto": "Mi App"})).contains("No existe"));
        let r = ej("crear_proyecto", json!({"nombre": "Mi App", "contexto": "Objetivo: probar."}));
        assert!(r.contains("equipo:mi-app"), "{r}");
        assert!(ej("crear_proyecto", json!({"nombre": "mi app"})).contains("ya existía"));

        ej("crear_nota", json!({"proyecto": "equipo:mi-app", "titulo": "Diseño", "cuerpo": "Usamos Rust.", "etiquetas": ["rust"]}));
        ej("registrar", json!({"proyecto": "Mi App", "entrada": "Primer paso.", "agente": "Codex"}));
        ej("registrar", json!({"proyecto": "Mi App", "entrada": "Segundo paso."}));

        let ctx = ej("contexto_proyecto", json!({"proyecto": "MI APP"}));
        let (i_ctx, i_bit, i_dis) = (ctx.find("## Contexto").unwrap(), ctx.find("## Bitácora").unwrap(), ctx.find("## Diseño").unwrap());
        assert!(i_ctx < i_bit && i_bit < i_dis, "{ctx}");
        assert!(ctx.contains("Primer paso.") && ctx.contains("Segundo paso.") && ctx.contains("· Codex"));
        assert_eq!(ctx.matches("## Bitácora").count(), 1, "una sola bitácora");

        ej("actualizar_nota", json!({"id": "equipo:mi-app/contexto.md", "cuerpo": "- pendiente: tests", "modo": "anexar"}));
        let n = b.leer_nota("equipo:mi-app/contexto.md").unwrap();
        assert_eq!(n.cuerpo, "Objetivo: probar.\n\n- pendiente: tests\n");
        assert_eq!(n.resumen.etiquetas, vec!["contexto"], "las etiquetas se conservan");

        assert!(ej("buscar", json!({"texto": "rust"})).contains("Diseño"));
        assert!(ej("listar_notas", json!({"etiqueta": "bitacora"})).contains("Bitácora"));
        assert!(ejecutar(&b, "crear_nota", &json!({"proyecto": "Otro", "titulo": "x", "cuerpo": ""})).is_err());
        std::fs::remove_dir_all(base).unwrap();
    }
}
