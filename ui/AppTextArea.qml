import QtQuick
import QtQuick.Controls as Controls
import "." as App

Controls.TextArea {
    id: control
    leftPadding: 12
    rightPadding: 12
    topPadding: 10
    bottomPadding: 10
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
