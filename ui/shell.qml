import QtQuick
import Quickshell

ShellRoot {
    DiagnosticView {
        id: diagnostic
        opened: true
        onStopped: Qt.quit()
    }
    Connections {
        target: Quickshell
        function onLastWindowClosed() { diagnostic.close() }
    }
}
