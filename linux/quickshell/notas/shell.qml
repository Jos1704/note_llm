// Panel de nota rápida para Linux (Wayland), equivalente al widget del notch de macOS.
//
// Pasa el ratón por el borde superior, al centro de la pantalla (o usa el atajo), y baja un
// panel para capturar una nota. Guarda con `md-notes --guardar-rapida`, así que no hace falta
// tener Notas abierta. Instalación y atajo: ver README («Panel en Linux»).
//
//   qs -c notas                              # arrancarlo (exec-once en Hyprland)
//   qs -c notas ipc call notas alternar      # abrir/cerrar con foco (o `md-notes --capturar`)

import QtQuick
import QtQuick.Controls.Basic
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

ShellRoot {
    id: raiz

    // --- ajustes
    readonly property string md: Quickshell.env("MD_NOTES_BIN") || (Quickshell.env("HOME") + "/.local/bin/md-notes")
    readonly property int ancho: 480
    readonly property int alto: 330
    readonly property int franjaAncho: 200
    readonly property int franjaAlto: 4
    readonly property color texto: Qt.rgba(1, 1, 1, 0.92)
    readonly property color tenue: Qt.rgba(1, 1, 1, 0.5)
    readonly property color campo: Qt.rgba(1, 1, 1, 0.07)
    readonly property color campoFoco: Qt.rgba(1, 1, 1, 0.11)
    readonly property color borde: Qt.rgba(1, 1, 1, 0.1)
    readonly property color acento: "#8fc4ad"
    readonly property color rojo: "#f08a7e"
    readonly property string fuente: "Inter Variable"

    // --- estado compartido entre pantallas
    property var pantallaAbierta: null   // ShellScreen con el panel abierto (null: cerrado)
    property bool conFoco: false         // abierto con el atajo: foco directo al texto
    property bool ocupado: false         // captura en curso: no se pliega solo
    property bool oculto: false          // captura de pantalla en curso: el panel no debe salir
    property var proyectos: []           // [{id, nombre}]
    property string ultimoProyecto: ""
    property string borradorTitulo: ""
    property string borradorCuerpo: ""

    function abrir(pantalla, foco) {
        conFoco = foco;
        pantallaAbierta = pantalla;
        cargarProyectos.running = true;
    }
    function cerrar() {
        pantallaAbierta = null;
        conFoco = false;
    }

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
            WlrLayershell.keyboardFocus: !abierto || raiz.ocupado ? WlrKeyboardFocus.None
                : raiz.conFoco ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.OnDemand

            // Plegado solo recibe el ratón la franja de arriba; el resto deja pasar los clics.
            mask: Region { item: ventana.abierto && !raiz.oculto ? panel : franja }

            onAbiertoChanged: {
                if (abierto) {
                    titulo.text = raiz.borradorTitulo;
                    cuerpo.text = raiz.borradorCuerpo;
                    estado.mostrar("", "");
                    if (raiz.conFoco) enfocar.restart();
                } else {
                    raiz.borradorTitulo = titulo.text;
                    raiz.borradorCuerpo = cuerpo.text;
                }
            }

            function vacio() { return !titulo.text.trim() && !cuerpo.text.trim(); }

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
                color: "#000"
                radius: 24
                clip: true
                Behavior on width { NumberAnimation { duration: 340; easing.type: Easing.OutQuint } }
                Behavior on height { NumberAnimation { duration: 340; easing.type: Easing.OutQuint } }
                Behavior on opacity { NumberAnimation { duration: ventana.abierto ? 100 : 200 } }

                // Esquinas de arriba rectas (pegado al borde de la pantalla, como el notch).
                Rectangle { anchors.top: parent.top; width: parent.width; height: parent.radius; color: parent.color }

                HoverHandler {
                    onHoveredChanged: hovered ? cierreTrasSalir.stop() : cierreTrasSalir.restart()
                }

                Shortcut { sequences: ["Ctrl+Return", "Ctrl+Enter"]; enabled: ventana.abierto; onActivated: guardar.enviar() }
                Shortcut { sequence: "Escape"; enabled: ventana.abierto; onActivated: raiz.cerrar() }

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
                            font { family: raiz.fuente; pixelSize: 13; weight: Font.DemiBold }
                            padding: 7; leftPadding: 10
                            background: Rectangle { radius: 9; color: titulo.activeFocus ? raiz.campoFoco : raiz.campo; border.color: titulo.activeFocus ? raiz.borde : "transparent" }
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
                            background: Rectangle { radius: 9; color: raiz.campo; implicitHeight: titulo.height }
                        }
                    }

                    ScrollView {
                        width: parent.width
                        height: raiz.alto - 14 - titulo.height - pie.height - 8 * 2 - 14
                        TextArea {
                            id: cuerpo
                            placeholderText: "Escribe una nota rápida… (#etiqueta o @proyecto en la primera línea)"
                            placeholderTextColor: raiz.tenue
                            color: raiz.texto
                            wrapMode: TextEdit.Wrap
                            font { family: raiz.fuente; pixelSize: 13 }
                            padding: 9; leftPadding: 10
                            background: Rectangle { radius: 9; color: cuerpo.activeFocus ? raiz.campoFoco : raiz.campo; border.color: cuerpo.activeFocus ? raiz.borde : "transparent" }
                        }
                    }

                    Row {
                        id: pie
                        width: parent.width
                        spacing: 8
                        height: 30

                        Repeater {
                            model: [
                                { accion: "--portapapeles", icono: "⧉", ayuda: "Pegar el portapapeles" },
                                { accion: "--seleccion", icono: "T", ayuda: "Texto seleccionado" },
                                { accion: "--captura-pantalla", icono: "⌗", ayuda: "Captura de pantalla (con su texto)" }
                            ]
                            delegate: Rectangle {
                                required property var modelData
                                width: 28; height: 28; radius: 8
                                anchors.verticalCenter: parent.verticalCenter
                                color: zona.containsMouse ? raiz.campoFoco : "transparent"
                                opacity: raiz.ocupado ? 0.35 : 1
                                Text {
                                    anchors.centerIn: parent
                                    text: modelData.icono
                                    color: zona.containsMouse ? raiz.texto : raiz.tenue
                                    font { family: raiz.fuente; pixelSize: 14 }
                                }
                                MouseArea {
                                    id: zona
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    onClicked: captura.ejecutar(modelData.accion)
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
                            font { family: raiz.fuente; pixelSize: 11.5 }
                            property string clase: ""
                            color: clase === "ok" ? raiz.acento : clase === "error" ? raiz.rojo : raiz.tenue
                            function mostrar(t, c) { text = t || "Ctrl+↩ guardar · Esc cerrar"; clase = c; }
                        }

                        Rectangle {
                            id: guardar
                            width: etiqueta.implicitWidth + 28; height: 28; radius: 14
                            anchors.verticalCenter: parent.verticalCenter
                            color: raiz.texto
                            opacity: guardarProceso.running ? 0.4 : zonaGuardar.containsMouse ? 0.88 : 1
                            Text { id: etiqueta; anchors.centerIn: parent; text: "Guardar"; color: "#000"; font { family: raiz.fuente; pixelSize: 13; weight: Font.DemiBold } }
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

                function ejecutar(accion) {
                    if (raiz.ocupado) return;
                    raiz.ocupado = true;
                    raiz.oculto = accion === "--captura-pantalla";
                    estado.mostrar("…", "");
                    command = [raiz.md, accion];
                    running = true;
                }
                function insertar(t) {
                    if (!t) return;
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
