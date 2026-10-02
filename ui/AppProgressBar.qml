import QtQuick
import QtQuick.Controls as Controls
import "." as App

Controls.ProgressBar {
    id: control
    implicitHeight: 8
    background: Rectangle { radius: 4; color: App.Theme.field }
    contentItem: Item {
        Rectangle {
            width: parent.width * control.position
            height: parent.height
            radius: 4
            color: App.Theme.accent
        }
    }
}
