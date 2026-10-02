import QtQuick
import QtQuick.Controls as Controls
import "." as App

Controls.CheckBox {
    id: control
    spacing: 10
    indicator: Rectangle {
        implicitWidth: 19
        implicitHeight: 19
        x: control.leftPadding
        y: (control.height - height) / 2
        radius: 4
        color: control.checked ? App.Theme.accent : App.Theme.field
        border.color: control.activeFocus || control.hovered ? App.Theme.accent : App.Theme.border
        Text { anchors.centerIn: parent; visible: control.checked; text: "✓"; color: App.Theme.accentText; font.bold: true }
    }
    contentItem: Text {
        leftPadding: control.indicator.width + control.spacing
        text: control.text
        color: control.enabled ? App.Theme.foreground : App.Theme.muted
        verticalAlignment: Text.AlignVCenter
        wrapMode: Text.Wrap
    }
}
