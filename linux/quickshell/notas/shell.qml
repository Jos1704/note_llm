// Panel de nota rápida para Linux (Wayland), equivalente al widget del notch de macOS.
//
// Pasa el ratón por el borde superior, al centro de la pantalla (o usa el atajo), y baja un
// panel para capturar una nota. Guarda con `md-notes --guardar-rapida`, así que no hace falta
// tener Notas abierta. Instalación y atajo: ver README («Panel en Linux»).
//
//   qs -c notas                              # arrancarlo (exec-once en Hyprland)
//   qs -c notas ipc call notas alternar      # abrir/cerrar con foco (o `md-notes --capturar`)
//
// Colores: los de la app (paleta y modo), que la app copia a `~/.config/md-notes/tema.json`.

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Shapes
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

ShellRoot {
    id: raiz

    // --- ajustes
    readonly property string md: Quickshell.env("MD_NOTES_BIN") || (Quickshell.env("HOME") + "/.local/bin/md-notes")
    readonly property string dirConfig: Quickshell.env("MD_NOTES_CONFIG")
        || ((Quickshell.env("XDG_CONFIG_HOME") || (Quickshell.env("HOME") + "/.config")) + "/md-notes")
    readonly property int ancho: 480
    readonly property int alto: 330
    readonly property int franjaAncho: 200
    readonly property int franjaAlto: 4
    readonly property string fuente: "Inter Variable"

    // --- tema: mismas paletas que notch.html / estilos.css
    readonly property var paletas: ({
        bruma: { claro: ["#f6f7f4", "#2b322e", "#3f6b5b", "#ffffff"], oscuro: ["#171c1a", "#e4e9e5", "#8ec1ab", "#10221b"] },
        lavanda: { claro: ["#f7f6fa", "#2e2b38", "#5e5596", "#ffffff"], oscuro: ["#1a1820", "#e7e4ee", "#aba3dc", "#1a1530"] },
        marea: { claro: ["#f4f7f8", "#26313a", "#2f6680", "#ffffff"], oscuro: ["#161b1f", "#e2e8ec", "#86b4ce", "#0e1e28"] },
        eva: { claro: ["#f1ecfa", "#1c1430", "#3a6f00", "#ffffff"], oscuro: ["#161026", "#f1ecff", "#9cff3a", "#0e0a17"] },
        vidrio: { claro: ["#f0fafbff", "#1d2230", "#2a63c4", "#ffffff"], oscuro: ["#ed1a1e2c", "#eef1f8", "#8fb8ff", "#0b1426"] }
    })
    property string paleta: "bruma"
    property string modo: "auto"          // auto | light | dark
    property bool sistemaOscuro: true     // modo del sistema (gsettings color-scheme)
    readonly property bool oscuro: modo === "dark" || (modo !== "light" && sistemaOscuro)
    readonly property var semillas: (paletas[paleta] || paletas.bruma)[oscuro ? "oscuro" : "claro"]
    readonly property color fondo: semillas[0]
    readonly property color fondoSolido: Qt.rgba(fondo.r, fondo.g, fondo.b, 1)
    readonly property color texto: semillas[1]
    readonly property color acento: semillas[2]
    readonly property color sobre: semillas[3]
    readonly property color tenue: alfa(texto, 0.62)
    readonly property color campo: alfa(texto, 0.07)
    readonly property color campoFoco: alfa(texto, 0.11)
    readonly property color borde: paleta === "eva" ? (oscuro ? Qt.rgba(150 / 255, 115 / 255, 230 / 255, 0.45) : Qt.rgba(91 / 255, 61 / 255, 153 / 255, 0.35))
        : alfa(texto, 0.14)
    readonly property color rojo: oscuro ? "#f08a7e" : "#b4533f"
    readonly property int radio: paleta === "vidrio" ? 30 : 24

    function alfa(c, a) { return Qt.rgba(c.r, c.g, c.b, a); }

    FileView {
        id: archivoTema
        path: raiz.dirConfig + "/tema.json"
        watchChanges: true
        printErrors: false // sin tema.json (la app aún no se ha abierto): paleta por defecto
        onFileChanged: reload()
        onLoaded: {
            try {
                const t = JSON.parse(text());
                if (raiz.paletas[t.paleta]) raiz.paleta = t.paleta;
                raiz.modo = t.modo === "light" || t.modo === "dark" ? t.modo : "auto";
            } catch (e) {}
        }
    }

    // La vigilancia del archivo no ve un tema.json creado después, ni el reemplazo atómico que hace
    // la app al guardarlo: mientras el panel está abierto se relee cada poco.
    Timer {
        interval: 1500
        repeat: true
        running: raiz.pantallaAbierta !== null
        onTriggered: archivoTema.reload()
    }

    Process {
        id: modoSistema
        running: true
        command: ["gsettings", "get", "org.gnome.desktop.interface", "color-scheme"]
        stdout: StdioCollector { onStreamFinished: raiz.sistemaOscuro = text.indexOf("dark") >= 0 }
    }

    // --- estado compartido entre pantallas
    property var pantallaAbierta: null   // ShellScreen con el panel abierto (null: cerrado)
    property bool conFoco: false         // abierto con el atajo: foco directo al texto
    property bool ocupado: false         // captura en curso: no se pliega solo
    property bool oculto: false          // captura de pantalla en curso: el panel no debe salir
    property var proyectos: []           // [{id, nombre}]
    property var etiquetas: []           // nombres, para las sugerencias de #etiqueta
    property string ultimoProyecto: ""
    property string borradorTitulo: ""
    property string borradorCuerpo: ""

    function abrir(pantalla, foco) {
        conFoco = foco;
        pantallaAbierta = pantalla;
        cargarProyectos.running = true;
        cargarEtiquetas.running = true;
        archivoTema.reload();
        if (modo === "auto") modoSistema.running = true;
    }
    function cerrar() {
        pantallaAbierta = null;
        conFoco = false;
    }

    // Prefijos: igual que src/prefijos.ts (sin acentos ni mayúsculas; `@` usa la forma corta).
    function plano(s) {
        try { return s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase(); }
        catch (e) { return s.toLowerCase(); }
    }
    function slug(s) { return plano(s).trim().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, ""); }

    IpcHandler {
        target: "notas"
        function alternar(): void {
            if (raiz.pantallaAbierta) raiz.cerrar();
            else monitorActivo.running = true; // abre al saber en qué monitor estás
        }
        function abrir(): void { if (!raiz.pantallaAbierta) alternar(); }
        function cerrar(): void { raiz.cerrar(); }
    }

    function pantallaLlamada(nombre) {
        const lista = Quickshell.screens;
        for (let i = 0; i < lista.length; i++) if (lista[i].name === nombre) return lista[i];
        return lista.length ? lista[0] : null;
    }

    // Monitor con el foco en Hyprland (sin depender del módulo Quickshell.Hyprland). Sin
    // Hyprland, la primera pantalla.
    Process {
        id: monitorActivo
        command: ["sh", "-c", "hyprctl activeworkspace -j 2>/dev/null"]
        stdout: StdioCollector {
            onStreamFinished: {
                let nombre = "";
                try { nombre = JSON.parse(text).monitor ?? ""; } catch (e) {}
                raiz.abrir(raiz.pantallaLlamada(nombre), true);
            }
        }
    }

    Process {
        id: cargarProyectos
        command: [raiz.md, "--proyectos"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    raiz.proyectos = JSON.parse(text).filter(p => !p.id.endsWith(":bandeja"));
                } catch (e) {}
            }
        }
    }

    Process {
        id: cargarEtiquetas
        command: [raiz.md, "--etiquetas"]
        stdout: StdioCollector {
            onStreamFinished: {
                try { raiz.etiquetas = JSON.parse(text).map(e => e.nombre); } catch (e) {}
            }
        }
    }

    Variants {
        model: Quickshell.screens

        PanelWindow {
            id: ventana
            required property var modelData
            screen: modelData

            readonly property bool abierto: raiz.pantallaAbierta === modelData

            anchors.top: true
            exclusionMode: ExclusionMode.Ignore
            implicitWidth: raiz.ancho
            implicitHeight: raiz.alto
            color: "transparent"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.namespace: "notas-rapida"
            WlrLayershell.keyboardFocus: !abierto || raiz.oculto ? WlrKeyboardFocus.None
                : raiz.conFoco ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.OnDemand

            // Plegado solo recibe el ratón la franja de arriba; el resto deja pasar los clics.
            mask: Region { item: ventana.abierto && !raiz.oculto ? panel : franja }

            onAbiertoChanged: {
                if (abierto) {
                    titulo.text = raiz.borradorTitulo;
                    cuerpo.text = raiz.borradorCuerpo;
                    estado.mostrar("", "");
                    sugerencias = [];
                    if (raiz.conFoco) enfocar.restart();
                } else {
                    raiz.borradorTitulo = titulo.text;
                    raiz.borradorCuerpo = cuerpo.text;
                }
            }

            function vacio() { return !titulo.text.trim() && !cuerpo.text.trim(); }

            // --- sugerencias de #etiqueta / @proyecto (solo en el título y la primera línea)
            property var sugerencias: []      // [{mostrar, insertar}]
            property var campoSugerencias: null
            property int inicioToken: 0

            function actualizarSugerencias(campo, esCuerpo) {
                if (!campo.activeFocus) return;
                const pos = campo.cursorPosition;
                const t = campo.text;
                if (esCuerpo) {
                    const primera = t.search(/\S/);
                    const fin = primera < 0 ? -1 : t.indexOf("\n", primera);
                    if (fin >= 0 && pos > fin) { sugerencias = []; return; }
                }
                const m = /(^|\s)([#@])([^\s#@]*)$/.exec(t.slice(0, pos));
                if (!m) { sugerencias = []; return; }
                inicioToken = pos - m[2].length - m[3].length;
                campoSugerencias = campo;
                const q = raiz.plano(m[3]);
                if (m[2] === "#") {
                    sugerencias = raiz.etiquetas
                        .filter(e => raiz.plano(e).startsWith(q))
                        .slice(0, 6)
                        .map(e => ({ mostrar: "#" + e, insertar: "#" + e }));
                } else {
                    const s = raiz.slug(m[3]);
                    sugerencias = raiz.proyectos
                        .filter(p => raiz.slug(p.nombre).startsWith(s) || raiz.plano(p.nombre).includes(q))
                        .slice(0, 6)
                        .map(p => ({ mostrar: "@" + p.nombre, insertar: "@" + raiz.slug(p.nombre) }));
                }
            }
            function aceptarSugerencia(i) {
                const o = sugerencias[i];
                const c = campoSugerencias;
                if (!o || !c) return;
                const inicio = inicioToken;
                c.remove(inicio, c.cursorPosition);
                c.insert(inicio, o.insertar + " ");
                c.cursorPosition = inicio + o.insertar.length + 1;
                sugerencias = [];
                c.forceActiveFocus();
            }
            /** Tab acepta la primera sugerencia; Ctrl+V en el texto también pega imágenes. */
            function teclaEnCampo(e, esCuerpo) {
                if (e.key === Qt.Key_Tab && sugerencias.length) {
                    aceptarSugerencia(0);
                    e.accepted = true;
                } else if (esCuerpo && e.matches(StandardKey.Paste)) {
                    captura.ejecutar("--portapapeles", true);
                    e.accepted = true;
                }
            }

            Timer { id: enfocar; interval: 80; onTriggered: { cuerpo.forceActiveFocus(); cuerpo.cursorPosition = cuerpo.length; } }
            Timer { id: abrirTrasHover; interval: 100; onTriggered: if (zonaFranja.containsMouse) raiz.abrir(ventana.modelData, false) }
            Timer {
                id: cierreTrasSalir
                interval: 450
                onTriggered: if (!raiz.ocupado && !raiz.conFoco && (ventana.vacio() || !cuerpo.activeFocus && !titulo.activeFocus)) raiz.cerrar()
            }

            Item {
                id: franja
                width: raiz.franjaAncho
                height: raiz.franjaAlto
                anchors.top: parent.top
                anchors.horizontalCenter: parent.horizontalCenter
                MouseArea {
                    id: zonaFranja
                    anchors.fill: parent
                    hoverEnabled: true
                    onEntered: abrirTrasHover.restart()
                }
            }

            Rectangle {
                id: panel
                anchors.top: parent.top
                anchors.horizontalCenter: parent.horizontalCenter
                width: ventana.abierto ? raiz.ancho : raiz.franjaAncho
                height: ventana.abierto ? raiz.alto : raiz.franjaAlto
                opacity: ventana.abierto && !raiz.oculto ? 1 : 0
                color: raiz.fondo
                border.color: raiz.borde
                border.width: 1
                // Esquinas de arriba rectas: pegado al borde de la pantalla, como el notch.
                topLeftRadius: 0
                topRightRadius: 0
                bottomLeftRadius: raiz.radio
                bottomRightRadius: raiz.radio
                clip: true
                Behavior on width { NumberAnimation { duration: 340; easing.type: Easing.OutQuint } }
                Behavior on height { NumberAnimation { duration: 340; easing.type: Easing.OutQuint } }
                Behavior on opacity { NumberAnimation { duration: ventana.abierto ? 100 : 200 } }
                Behavior on color { ColorAnimation { duration: 200 } }

                // EVA-01: filo verde abajo.
                Rectangle {
                    visible: raiz.paleta === "eva"
                    anchors.bottom: parent.bottom
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: parent.width - 2 * raiz.radio
                    height: 2
                    color: raiz.acento
                }

                HoverHandler {
                    onHoveredChanged: hovered ? cierreTrasSalir.stop() : cierreTrasSalir.restart()
                }

                Shortcut { sequences: ["Ctrl+Return", "Ctrl+Enter"]; enabled: ventana.abierto; onActivated: guardar.enviar() }
                Shortcut {
                    sequence: "Escape"
                    enabled: ventana.abierto
                    onActivated: {
                        if (ventana.sugerencias.length) ventana.sugerencias = [];
                        else raiz.cerrar();
                    }
                }

                Column {
                    id: formulario
                    x: 18; y: 14
                    width: parent.width - 36
                    spacing: 8
                    opacity: ventana.abierto ? 1 : 0
                    Behavior on opacity { NumberAnimation { duration: 220 } }

                    Row {
                        width: parent.width
                        spacing: 8
                        TextField {
                            id: titulo
                            width: parent.width - proyecto.width - 8
                            placeholderText: "Título (opcional)"
                            placeholderTextColor: raiz.tenue
                            color: raiz.texto
                            selectionColor: raiz.alfa(raiz.acento, 0.35)
                            selectedTextColor: raiz.texto
                            font { family: raiz.fuente; pixelSize: 13; weight: Font.DemiBold }
                            padding: 7; leftPadding: 10
                            background: Rectangle { radius: 9; color: titulo.activeFocus ? raiz.campoFoco : raiz.campo; border.color: titulo.activeFocus ? raiz.borde : "transparent" }
                            onTextChanged: ventana.actualizarSugerencias(titulo, false)
                            onCursorPositionChanged: ventana.actualizarSugerencias(titulo, false)
                            onActiveFocusChanged: if (!activeFocus && ventana.campoSugerencias === titulo) ventana.sugerencias = []
                            Keys.onPressed: (e) => ventana.teclaEnCampo(e, false)
                        }
                        ComboBox {
                            id: proyecto
                            width: 150
                            model: [{ id: "", nombre: "Bandeja" }].concat(raiz.proyectos)
                            textRole: "nombre"
                            valueRole: "id"
                            font { family: raiz.fuente; pixelSize: 13 }
                            onModelChanged: {
                                const i = model.findIndex(p => p.id === raiz.ultimoProyecto);
                                currentIndex = i >= 0 ? i : 0;
                            }
                            onActivated: raiz.ultimoProyecto = currentValue
                            contentItem: Text {
                                leftPadding: 8
                                text: proyecto.displayText
                                color: raiz.tenue
                                font: proyecto.font
                                verticalAlignment: Text.AlignVCenter
                                elide: Text.ElideRight
                            }
                            indicator: Text {
                                x: proyecto.width - width - 8
                                anchors.verticalCenter: parent.verticalCenter
                                text: "▾"
                                color: raiz.tenue
                                font.pixelSize: 11
                            }
                            background: Rectangle { radius: 9; color: raiz.campo; implicitHeight: titulo.height }
                            delegate: ItemDelegate {
                                id: opcion
                                required property var modelData
                                required property int index
                                width: proyecto.width
                                highlighted: proyecto.highlightedIndex === index
                                contentItem: Text {
                                    text: opcion.modelData.nombre
                                    color: raiz.texto
                                    font: proyecto.font
                                    elide: Text.ElideRight
                                    verticalAlignment: Text.AlignVCenter
                                }
                                background: Rectangle { radius: 6; color: opcion.highlighted ? raiz.campoFoco : "transparent" }
                            }
                            popup.background: Rectangle { radius: 9; color: raiz.fondoSolido; border.color: raiz.borde }
                        }
                    }

                    ScrollView {
                        width: parent.width
                        height: raiz.alto - 14 - titulo.height - pie.height - 8 * 2 - 14
                            - (fichas.visible ? fichas.height + 8 : 0)
                        TextArea {
                            id: cuerpo
                            placeholderText: "Escribe una nota rápida… (#etiqueta o @proyecto en la primera línea)"
                            placeholderTextColor: raiz.tenue
                            color: raiz.texto
                            selectionColor: raiz.alfa(raiz.acento, 0.35)
                            selectedTextColor: raiz.texto
                            wrapMode: TextEdit.Wrap
                            font { family: raiz.fuente; pixelSize: 13 }
                            padding: 9; leftPadding: 10
                            background: Rectangle { radius: 9; color: cuerpo.activeFocus ? raiz.campoFoco : raiz.campo; border.color: cuerpo.activeFocus ? raiz.borde : "transparent" }
                            onTextChanged: ventana.actualizarSugerencias(cuerpo, true)
                            onCursorPositionChanged: ventana.actualizarSugerencias(cuerpo, true)
                            onActiveFocusChanged: if (!activeFocus && ventana.campoSugerencias === cuerpo) ventana.sugerencias = []
                            Keys.onPressed: (e) => ventana.teclaEnCampo(e, true)
                        }
                    }

                    // Fichas de sugerencias: clic o Tab (la primera) para aceptar.
                    Flow {
                        id: fichas
                        width: parent.width
                        spacing: 6
                        visible: ventana.sugerencias.length > 0
                        Repeater {
                            model: ventana.sugerencias
                            delegate: Rectangle {
                                required property var modelData
                                required property int index
                                width: textoFicha.implicitWidth + 20
                                height: 22
                                radius: 11
                                color: raiz.alfa(raiz.acento, zonaFicha.containsMouse ? 0.26 : 0.16)
                                border.color: index === 0 ? raiz.alfa(raiz.acento, 0.5) : "transparent"
                                Text {
                                    id: textoFicha
                                    anchors.centerIn: parent
                                    text: modelData.mostrar
                                    color: raiz.acento
                                    font { family: raiz.fuente; pixelSize: 12 }
                                }
                                MouseArea {
                                    id: zonaFicha
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    // Al pulsar (no al soltar) para que el campo no pierda el foco antes.
                                    onPressed: ventana.aceptarSugerencia(index)
                                }
                            }
                        }
                    }

                    Row {
                        id: pie
                        width: parent.width
                        spacing: 8
                        height: 30

                        // Mismos iconos que notch.html (trazos SVG de 24×24, dibujados a 16 px).
                        Repeater {
                            model: [
                                { accion: "--portapapeles", ayuda: "Pegar el portapapeles",
                                  trazo: "M9 3h6a1 1 0 0 1 1 1v2a1 1 0 0 1-1 1H9a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z M16 5h2a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h2" },
                                { accion: "--seleccion", ayuda: "Texto seleccionado",
                                  trazo: "M4 7V5h16v2 M9 19h6 M12 5v14" },
                                { accion: "--captura-pantalla", ayuda: "Captura de pantalla (con su texto)",
                                  trazo: "M6 2v14a2 2 0 0 0 2 2h14 M18 22V8a2 2 0 0 0-2-2H2" }
                            ]
                            delegate: Rectangle {
                                id: boton
                                required property var modelData
                                width: 28; height: 28; radius: 8
                                anchors.verticalCenter: parent.verticalCenter
                                color: zona.containsMouse ? raiz.campoFoco : "transparent"
                                opacity: raiz.ocupado ? 0.35 : 1
                                Shape {
                                    anchors.centerIn: parent
                                    width: 24; height: 24
                                    scale: 16 / 24
                                    preferredRendererType: Shape.CurveRenderer
                                    ShapePath {
                                        strokeColor: zona.containsMouse ? raiz.texto : raiz.tenue
                                        strokeWidth: 1.8
                                        fillColor: "transparent"
                                        capStyle: ShapePath.RoundCap
                                        joinStyle: ShapePath.RoundJoin
                                        PathSvg { path: boton.modelData.trazo }
                                    }
                                }
                                MouseArea {
                                    id: zona
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    onClicked: captura.ejecutar(boton.modelData.accion, false)
                                }
                                ToolTip.visible: zona.containsMouse
                                ToolTip.delay: 600
                                ToolTip.text: modelData.ayuda
                            }
                        }

                        Text {
                            id: estado
                            width: parent.width - 3 * 36 - guardar.width - 8
                            anchors.verticalCenter: parent.verticalCenter
                            elide: Text.ElideRight
                            font { family: raiz.fuente; pixelSize: 12 }
                            property string clase: ""
                            color: clase === "ok" ? raiz.acento : clase === "error" ? raiz.rojo : raiz.tenue
                            function mostrar(t, c) { text = t || "Ctrl+↩ guardar · Esc cerrar"; clase = c; }
                        }

                        Rectangle {
                            id: guardar
                            width: etiqueta.implicitWidth + 28; height: 28; radius: 14
                            anchors.verticalCenter: parent.verticalCenter
                            color: raiz.acento
                            opacity: guardarProceso.running ? 0.4 : zonaGuardar.containsMouse ? 0.88 : 1
                            Text {
                                id: etiqueta
                                anchors.centerIn: parent
                                text: raiz.paleta === "eva" ? "GUARDAR" : "Guardar"
                                color: raiz.sobre
                                font { family: raiz.fuente; pixelSize: 13; weight: Font.DemiBold; letterSpacing: raiz.paleta === "eva" ? 1.5 : 0 }
                            }
                            MouseArea { id: zonaGuardar; anchors.fill: parent; hoverEnabled: true; onClicked: guardar.enviar() }

                            function enviar() {
                                if (guardarProceso.running) return;
                                if (ventana.vacio()) { cuerpo.forceActiveFocus(); return; }
                                guardarProceso.command = [raiz.md, "--guardar-rapida", "--titulo", titulo.text.trim(), "--proyecto", proyecto.currentValue ?? "", "--", cuerpo.text];
                                guardarProceso.running = true;
                            }
                        }
                    }
                }
            }

            Process {
                id: guardarProceso
                stdout: StdioCollector { id: salidaGuardar }
                stderr: StdioCollector { id: errorGuardar }
                onExited: (codigo) => {
                    if (codigo !== 0) {
                        estado.mostrar(errorGuardar.text.trim() || "No se pudo guardar", "error");
                        return;
                    }
                    let t = "";
                    try { t = JSON.parse(salidaGuardar.text).titulo; } catch (e) {}
                    raiz.ultimoProyecto = proyecto.currentValue ?? "";
                    titulo.text = ""; cuerpo.text = "";
                    raiz.borradorTitulo = ""; raiz.borradorCuerpo = "";
                    estado.mostrar("Guardada: " + t, "ok");
                    cerrarTrasGuardar.restart();
                }
            }
            Timer { id: cerrarTrasGuardar; interval: 650; onTriggered: raiz.cerrar() }

            // Portapapeles, selección y captura: `md-notes` hace el trabajo y devuelve JSON.
            Process {
                id: captura
                stdout: StdioCollector { id: salidaCaptura }
                stderr: StdioCollector { id: errorCaptura }

                /** `silencioso`: Ctrl+V, sin el «…» de espera. */
                function ejecutar(accion, silencioso) {
                    if (raiz.ocupado) return;
                    raiz.ocupado = true;
                    raiz.oculto = accion === "--captura-pantalla";
                    if (!silencioso) estado.mostrar("…", "");
                    command = [raiz.md, accion];
                    running = true;
                }
                function insertar(t) {
                    if (!t) return;
                    if (cuerpo.selectedText) cuerpo.remove(cuerpo.selectionStart, cuerpo.selectionEnd);
                    const pos = cuerpo.cursorPosition;
                    const antes = cuerpo.text.slice(0, pos);
                    const bloque = t.startsWith("![") && antes && !antes.endsWith("\n\n") ? (antes.endsWith("\n") ? "\n" : "\n\n") + t : t;
                    cuerpo.insert(pos, bloque);
                    cuerpo.forceActiveFocus();
                }
                onExited: (codigo) => {
                    raiz.ocupado = false;
                    raiz.oculto = false;
                    if (codigo !== 0) {
                        estado.mostrar(errorCaptura.text.trim() || "No se pudo capturar", "error");
                        return;
                    }
                    estado.mostrar("", "");
                    let r = null;
                    try { r = JSON.parse(salidaCaptura.text); } catch (e) { return; }
                    if (!r) return; // captura cancelada
                    if (r.tipo === "vacio") estado.mostrar("El portapapeles está vacío", "error");
                    else if (r.tipo === "imagen") insertar("![Imagen](adjunto:" + r.nombre + ")");
                    else if (r.nombre) {
                        const ocr = (r.texto || "").trim();
                        insertar("![Captura](adjunto:" + r.nombre + ")" + (ocr ? "\n\n<details>\n<summary>Texto de la captura</summary>\n\n" + ocr + "\n\n</details>" : ""));
                    } else insertar(r.texto);
                }
            }
        }
    }
}
