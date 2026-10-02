import QtQuick
import QtQuick.Layouts
import "." as App

Rectangle {
    id: card
    color: App.Theme.surface
    radius: 10
    border.color: App.Theme.border
    border.width: 1
    implicitHeight: contents.implicitHeight + 32
    default property alias content: contents.data
    ColumnLayout {
        id: contents
        anchors.fill: parent
        anchors.margins: 16
        spacing: 10
    }
}
