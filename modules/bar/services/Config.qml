pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io

// Misu's settings. Everything has a built-in default; ~/.config/misu/config.json
// (optional, reloaded live) overrides any key, e.g.
//   { "fontScale": 1.2, "rail": { "size": 44, "center": ["clock", "mpris"] } }
// `rail` is merged key by key over the default, so a partial edit keeps the rest.
Singleton {
    id: root

    // Rail widget ids: "workspaces", "tray", or any plugin's rail id
    // (plugins/index.js).
    readonly property var defaultRail: ({
        size: 48,
        reveal: true,
        top: ["quick-settings", "workspaces"],
        center: ["clock", "ai-usage"],
        bottom: ["mpris", "tray", "notifications", "audio-output", "bluetooth", "network", "battery"]
    })

    property var user: ({})

    readonly property string configPath: (Quickshell.env("XDG_CONFIG_HOME") || (Quickshell.env("HOME") + "/.config"))
        + "/misu/config.json"

    FileView {
        path: root.configPath
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: {
            try {
                root.user = JSON.parse(text()) || ({});
            } catch (e) {
                console.warn("misu: bad config " + root.configPath + ": " + e);
            }
        }
        onLoadFailed: root.user = ({})
    }

    function pick(key, fallback) {
        const v = root.user[key];
        return (v === undefined || v === null) ? fallback : v;
    }

    readonly property var rail: Object.assign({}, root.defaultRail, root.user.rail || ({}))

    readonly property real fontScale: Math.max(0.7, Math.min(1.6, root.pick("fontScale", 1.3)))
    readonly property string fontFamily: root.pick("fontFamily", "Space Grotesk")
    readonly property real frameThickness: root.pick("frameThickness", 2)
    readonly property real frameCorner: root.pick("frameCorner", 8)
    readonly property real frameOpacity: root.pick("frameOpacity", 1)
    readonly property string markText: root.pick("markText", "力")
    readonly property string markImage: root.pick("markImage", "")
    readonly property bool clock24h: root.pick("clock24h", true)
    readonly property bool reduceMotion: root.pick("reduceMotion", false)
    // Tallest a rail card grows, as a fraction of the screen; past it the card scrolls.
    readonly property real cardMaxHeight: Math.max(0.3, Math.min(0.95, root.pick("cardMaxHeight", 0.7)))
}
