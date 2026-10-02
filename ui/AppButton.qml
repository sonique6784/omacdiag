import QtQuick
import QtQuick.Controls as Controls
import "." as App

Controls.Button {
    id: control
    property bool prominent: false
    implicitHeight: 38
    leftPadding: 14
    rightPadding: 14
    font.pixelSize: 13
    font.bold: prominent
    background: Rectangle {
        radius: 7
        color: !control.enabled ? App.Theme.field : control.prominent ? (control.hovered ? Qt.lighter(App.Theme.accent, 1.12) : App.Theme.accent) : control.down || control.hovered ? App.Theme.selection : App.Theme.surface
        border.color: control.activeFocus ? App.Theme.accent : App.Theme.border
        border.width: 1
    }
    contentItem: Text {
        text: control.text
        font: control.font
        color: !control.enabled ? App.Theme.muted : control.prominent ? App.Theme.accentText : App.Theme.foreground
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
}
