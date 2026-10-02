import QtQuick
import QtQuick.Window
import Quickshell

FloatingWindow {
    id: patterns
    visible: false
    color: "black"
    property int patternIndex: 0
    property int screenIndex: 0
    screen: Quickshell.screens[Math.min(screenIndex, Quickshell.screens.length - 1)]
    implicitWidth: screen ? screen.width : 1024
    implicitHeight: screen ? screen.height : 768
    function show() {
        visible = true;
        Qt.callLater(() => {
            if (contentItem.Window.window) contentItem.Window.window.showFullScreen();
            surface.forceActiveFocus();
        });
    }
    Rectangle {
        id: surface
        anchors.fill: parent
        focus: true
        color: ["#ffffff", "#000000", "#ff0000", "#00ff00", "#0000ff", "#000000", "#ffffff"][patterns.patternIndex]
        Keys.onEscapePressed: patterns.visible = false
        Keys.onSpacePressed: patterns.patternIndex = (patterns.patternIndex + 1) % 7
        Rectangle {
            anchors.fill: parent
            visible: patterns.patternIndex === 5
            gradient: Gradient { orientation: Gradient.Horizontal; GradientStop { position: 0; color: "black" } GradientStop { position: 1; color: "white" } }
        }
        Canvas {
            anchors.fill: parent
            visible: patterns.patternIndex === 6
            onVisibleChanged: if (visible) requestPaint()
            onWidthChanged: requestPaint()
            onHeightChanged: requestPaint()
            onPaint: {
                var ctx = getContext("2d"); ctx.fillStyle = "white"; ctx.fillRect(0, 0, width, height);
                ctx.strokeStyle = "black"; ctx.lineWidth = 1;
                for (var x = 0; x < width; x += 8) { ctx.beginPath(); ctx.moveTo(x + .5, 0); ctx.lineTo(x + .5, height); ctx.stroke(); }
                for (var y = 0; y < height; y += 8) { ctx.beginPath(); ctx.moveTo(0, y + .5); ctx.lineTo(width, y + .5); ctx.stroke(); }
            }
        }
        Rectangle {
            anchors.bottom: parent.bottom; anchors.horizontalCenter: parent.horizontalCenter
            width: label.implicitWidth + 32; height: 42; color: "#dd202a2d"; radius: 8
            Text { id: label; anchors.centerIn: parent; text: "Space: next pattern · Esc: close · Click: next"; color: "white" }
        }
        MouseArea { anchors.fill: parent; onClicked: patterns.patternIndex = (patterns.patternIndex + 1) % 7 }
    }
}
