import QtQuick
import QtQuick.Controls as Controls
import "." as App

Controls.TextField {
    id: control
    implicitHeight: 38
    leftPadding: 12
    rightPadding: 12
    color: App.Theme.foreground
    placeholderTextColor: App.Theme.muted
    selectionColor: App.Theme.accent
    selectedTextColor: App.Theme.accentText
    background: Rectangle {
        radius: 7
        color: App.Theme.field
        border.color: control.activeFocus ? App.Theme.accent : App.Theme.border
        border.width: 1
    }
}
