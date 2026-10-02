import QtQuick
import QtQuick.Controls as Controls
import "." as App

Controls.TabButton {
    id: control
    implicitHeight: 42
    contentItem: Text {
        text: control.text
        color: control.checked ? App.Theme.accent : App.Theme.secondary
        font.pixelSize: 13
        font.bold: control.checked
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
    background: Rectangle {
        color: control.checked || control.hovered ? App.Theme.selection : App.Theme.surface
        border.color: control.checked ? App.Theme.accent : App.Theme.border
        border.width: 1
        radius: 7
    }
}
