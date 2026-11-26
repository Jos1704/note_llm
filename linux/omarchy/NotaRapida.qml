// Botón de la barra de Omarchy para Notas (md-notes).
//   Clic izquierdo: panel de nota rápida del borde superior (`md-notes --capturar`; si el panel
//                   de Quickshell no está corriendo, abre la ventana de nota rápida).
//   Clic derecho:   abre la app completa.
// Se registra en ~/.config/omarchy/shell.json:
//   { "id": "nota-rapida", "source": "~/.config/omarchy/bar/modules/NotaRapida.qml" }
import QtQuick
import Quickshell
import qs.Commons
import qs.Ui

Item {
  id: root

  property QtObject bar: null
  property string moduleName: ""
  property var settings: ({})

  readonly property string ejecutable: Quickshell.env("HOME") + "/.local/bin/md-notes"

  implicitWidth: button.implicitWidth
  implicitHeight: bar ? bar.barSize : Style.bar.sizeHorizontal

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: "" // nf-fa-sticky_note
    slotSize: Style.bar.statusSlot
    tooltipText: "Nota rápida (Super+Alt+N) · clic derecho: abrir Notas"
    onPressed: function (b) {
      if (b === Qt.RightButton) Quickshell.execDetached([root.ejecutable])
      else Quickshell.execDetached([root.ejecutable, "--capturar"])
    }
  }
}
