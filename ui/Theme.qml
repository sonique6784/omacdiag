pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

QtObject {
    id: theme
    readonly property string themePath: (Quickshell.env("XDG_STATE_HOME") || (Quickshell.env("HOME") + "/.local/state")) + "/omarchy/current/theme/colors.toml"
    property bool light: false
    property color background: "#1a1b26"
    property color surface: "#24283b"
    property color field: "#13141c"
    property color border: "#414868"
    property color foreground: "#c0caf5"
    property color secondary: "#b4bee6"
    property color muted: "#888fa8"
    property color accent: "#7aa2f7"
    property color success: "#9ece6a"
    property color warning: "#e0af68"
    property color danger: "#f7768e"
    readonly property color accentText: contrastText(accent)
    readonly property color selection: Qt.rgba(accent.r, accent.g, accent.b, light ? 0.16 : 0.23)

    function contrastText(color) {
        return (0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b) > 0.55 ? "#111111" : "#ffffff";
    }
    function parse(raw) {
        var values = {};
        String(raw).split("\n").forEach(line => {
            var match = line.match(/^\s*([A-Za-z_]+)\s*=\s*["']([^"']+)["']/);
            if (match) values[match[1]] = match[2];
        });
        if (!/^#[0-9a-fA-F]{6}$/.test(values.background || "") || !/^#[0-9a-fA-F]{6}$/.test(values.foreground || "")) return;
        function color(key, fallback) { return /^#[0-9a-fA-F]{6}$/.test(values[key] || "") ? values[key] : fallback; }
        light = values.mode === "light";
        background = values.background;
        surface = color("lighter_background", light ? "#f1f1f1" : "#24283b");
        field = color("dark_background", values.background);
        border = color("muted", light ? "#b0b0b0" : "#414868");
        foreground = color("bright_foreground", values.foreground);
        secondary = color("light_foreground", values.foreground);
        muted = color("dark_foreground", values.foreground);
        accent = color("accent", values.foreground);
        success = color("green", accent);
        warning = color("yellow", accent);
        danger = color("red", accent);
    }
    property FileView colorsFile: FileView {
        id: colorsFile
        path: theme.themePath
        printErrors: false
        onLoaded: theme.parse(text())
    }
    // Omarchy replaces the current theme directory when switching themes.
    // Reloading the stable path also works when a watcher retains the old inode.
    property Timer refresh: Timer {
        interval: 2500
        repeat: true
        running: true
        onTriggered: colorsFile.reload()
    }
}
