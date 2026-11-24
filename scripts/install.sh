#!/usr/bin/env bash
# Compila e instala Notas.
#   Linux: binario en ~/.local/bin/md-notes + entrada en el menú de aplicaciones.
#   macOS: Notas.app en /Applications.
set -euo pipefail
cd "$(dirname "$0")/.."

npm install --no-fund --no-audit

case "$(uname -s)" in
  Darwin)
    npx tauri build --bundles app
    app="src-tauri/target/release/bundle/macos/Notas.app"
    rm -rf "/Applications/Notas.app"
    cp -R "$app" /Applications/
    echo "✔ Instalada en /Applications/Notas.app"
    ;;
  Linux)
    npx tauri build --no-bundle
    install -Dm755 src-tauri/target/release/md-notes "$HOME/.local/bin/md-notes"
    install -Dm644 src-tauri/icons/128x128.png "$HOME/.local/share/icons/hicolor/128x128/apps/md-notes.png"
    install -Dm644 /dev/stdin "$HOME/.local/share/applications/md-notes.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Notas
Comment=Notas en Markdown por proyectos y etiquetas
Exec=$HOME/.local/bin/md-notes
Icon=md-notes
Categories=Office;Utility;
Terminal=false
StartupWMClass=md-notes
DESKTOP
    update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
    echo "✔ Instalada en ~/.local/bin/md-notes (búscala como «Notas» en el lanzador)"
    # Panel de nota rápida en el borde superior (Quickshell), equivalente al widget del notch.
    install -Dm644 linux/quickshell/notas/shell.qml "$HOME/.config/quickshell/notas/shell.qml"
    if command -v qs >/dev/null; then
      echo "✔ Panel instalado en ~/.config/quickshell/notas (arráncalo con: qs -c notas -d)"
    else
      echo "ℹ Panel copiado a ~/.config/quickshell/notas; instala Quickshell para usarlo"
    fi
    ;;
  *)
    echo "Sistema no soportado: $(uname -s)" >&2
    exit 1
    ;;
esac
