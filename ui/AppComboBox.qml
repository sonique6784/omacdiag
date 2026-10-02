import QtQuick
import QtQuick.Controls as Controls
import "." as App

Controls.ComboBox {
    id: control
    implicitHeight: 38
    leftPadding: 12
    rightPadding: 28
    contentItem: Text {
        leftPadding: control.leftPadding
        rightPadding: control.rightPadding
        text: control.displayText
        color: control.enabled ? App.Theme.foreground : App.Theme.muted
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
    indicator: Text {
        x: control.width - width - 10
        anchors.verticalCenter: parent.verticalCenter
        text: "⌄"
        color: App.Theme.secondary
        font.pixelSize: 17
    }
    background: Rectangle {
        radius: 7
        color: App.Theme.field
        border.color: control.activeFocus ? App.Theme.accent : App.Theme.border
    }
    delegate: Controls.ItemDelegate {
        width: control.width
        text: control.textRole ? model[control.textRole] : modelData
        highlighted: control.highlightedIndex === index
        contentItem: Text {
            text: parent.text
            color: App.Theme.foreground
            verticalAlignment: Text.AlignVCenter
            leftPadding: 10
        }
        background: Rectangle { color: parent.highlighted ? App.Theme.selection : App.Theme.surface }
    }
    popup: Controls.Popup {
        y: control.height + 3
        width: control.width
        implicitHeight: contentItem.implicitHeight + 4
        padding: 2
        background: Rectangle { color: App.Theme.surface; border.color: App.Theme.border; radius: 7 }
        contentItem: ListView {
            clip: true
            implicitHeight: contentHeight
            model: control.popup.visible ? control.delegateModel : null
            currentIndex: control.highlightedIndex
        }
    }
}
