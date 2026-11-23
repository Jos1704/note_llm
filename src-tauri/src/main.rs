// Evita una consola extra en Windows en modo release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `md-notes --mcp`: servidor MCP por stdio para agentes de IA (sin ventanas).
    if std::env::args().any(|a| a == "--mcp") {
        if let Err(e) = md_notes_lib::mcp::servir() {
            eprintln!("md-notes --mcp: {e:#}");
            std::process::exit(1);
        }
        return;
    }
    md_notes_lib::run()
}
