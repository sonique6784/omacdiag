import QtQuick
Item {
    id: root
    property var shell: null
    property bool reopenPending: false

    function open(_payloadJson) {
        if (diagnostic.item && diagnostic.item.closing) {
            reopenPending = true
            return
        }
        if (!diagnostic.active) diagnostic.active = true
        if (diagnostic.item) diagnostic.item.open()
    }

    function close() {
        if (diagnostic.item) diagnostic.item.close()
    }

    function dismiss() {
        if (root.shell && typeof root.shell.hide === "function")
            root.shell.hide("io.github.sonique6784.omacdiag")
        else close()
    }

    Loader {
        id: diagnostic
        active: false
        source: "../ui/DiagnosticView.qml"
        onLoaded: item.open()
    }

    Connections {
        target: diagnostic.item
        function onCloseRequested() { root.dismiss() }
        function onStopped() {
            diagnostic.active = false
            if (root.reopenPending) {
                root.reopenPending = false
                Qt.callLater(function() { root.open("{}") })
            }
        }
    }
}
