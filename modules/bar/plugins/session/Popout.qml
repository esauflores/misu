pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import "../../popouts"
import "../.."
import qs.services

// Session card (Super+Esc): the 力 SESSION title with the uptime, then Lock,
// Suspend, Log out, Restart and Power off. Up/Down (Tab, J/K) to move, Enter
// or a click to run, or the action's letter. Log out, Restart and Power off
// arm on the first press and run on the second, so a stray key ends nothing.
// Takes the keyboard while open (keyboard: "exclusive" in index.js).
Item {
    id: root

    property real s: 1
    property bool open: false
    signal requestClose()

    readonly property real pad: 12 * root.s
    readonly property real rowH: 42 * root.s
    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property color inkDim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)
    readonly property color line: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.14)
    // selection in plain ink, as in the launcher; armed rows go red
    readonly property color selBg: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.08)
    readonly property color hoverBg: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.04)

    readonly property var actions: [
        { key: "L", label: "Lock", icon: "lock", confirm: false, cmd: ["misu-lock"] },
        { key: "S", label: "Suspend", icon: "bedtime", confirm: false, cmd: ["systemctl", "suspend"] },
        { key: "E", label: "Log out", icon: "logout", confirm: true, cmd: ["wm", "msg", "quit"] },
        { key: "R", label: "Restart", icon: "restart_alt", confirm: true, cmd: ["systemctl", "reboot"] },
        { key: "P", label: "Power off", icon: "power_settings_new", confirm: true, cmd: ["systemctl", "poweroff"] }
    ]

    property int current: 0
    property int armed: -1       // the action waiting for its second press

    function run(i) {
        const a = root.actions[i];
        if (!a) return;
        root.current = i;
        if (a.confirm && root.armed !== i) {
            root.armed = i;
            disarm.restart();
            return;
        }
        root.armed = -1;
        root.requestClose();
        Quickshell.execDetached(a.cmd);
    }
    function move(by) {
        const n = root.actions.length;
        root.current = (root.current + by + n) % n;
        root.armed = -1;
    }

    // An armed action gives up after a few seconds.
    Timer { id: disarm; interval: 4000; onTriggered: root.armed = -1 }

    // uptime, read on open
    property string uptime: ""
    FileView {
        id: uptimeFile
        path: "/proc/uptime"
        printErrors: false
        onLoaded: {
            const secs = parseFloat(uptimeFile.text().split(" ")[0]) || 0;
            const d = Math.floor(secs / 86400), h = Math.floor(secs % 86400 / 3600), m = Math.floor(secs % 3600 / 60);
            root.uptime = "up " + (d ? d + "d " : "") + (d || h ? h + "h " : "") + m + "m";
        }
    }

    onOpenChanged: if (root.open) {
        root.current = 0;
        root.armed = -1;
        uptimeFile.reload();
        Qt.callLater(() => keys.forceActiveFocus());
    }

    implicitWidth: 260 * root.s
    implicitHeight: content.implicitHeight + root.pad * 2

    PopoutCard { anchors.fill: parent }

    // keyboard: arrows, letters, Enter
    Item {
        id: keys
        focus: true
        Keys.onUpPressed: root.move(-1)
        Keys.onDownPressed: root.move(1)
        Keys.onTabPressed: root.move(1)
        Keys.onBacktabPressed: root.move(-1)
        Keys.onReturnPressed: root.run(root.current)
        Keys.onEnterPressed: root.run(root.current)
        Keys.onPressed: event => {
            if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier)) return;
            if (event.key === Qt.Key_J) { root.move(1); event.accepted = true; return; }
            if (event.key === Qt.Key_K) { root.move(-1); event.accepted = true; return; }
            const t = event.text.toUpperCase();
            const i = root.actions.findIndex(a => a.key === t);
            if (i >= 0) { root.run(i); event.accepted = true; }
        }
    }

    Column {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: root.pad
        spacing: 10 * root.s

        Item {
            width: parent.width
            height: 20 * root.s
            PopoutTitle {
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                text: "SESSION"
                s: root.s
            }
            Text {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                text: root.uptime
                color: root.inkDim
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h6 * root.s
                font.letterSpacing: 0.6
            }
        }

        Column {
            width: parent.width
            Repeater {
                model: root.actions
                delegate: Item {
                    id: row
                    required property var modelData
                    required property int index
                    readonly property bool selected: row.index === root.current
                    readonly property bool isArmed: row.index === root.armed
                    // the armed row reads in the warning red
                    readonly property color tone: row.isArmed ? Theme.error : root.ink

                    width: parent.width
                    height: root.rowH

                    Rectangle {
                        anchors.fill: parent
                        color: row.isArmed ? Qt.rgba(Theme.error.r, Theme.error.g, Theme.error.b, 0.14)
                            : row.selected ? root.selBg
                            : hover.hovered ? root.hoverBg : "transparent"
                    }
                    Rectangle {
                        visible: row.selected || row.isArmed
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.bottom: parent.bottom
                        width: 2 * root.s
                        color: row.tone
                    }
                    MaterialIcon {
                        id: glyph
                        anchors.left: parent.left
                        anchors.leftMargin: 14 * root.s
                        anchors.verticalCenter: parent.verticalCenter
                        name: row.modelData.icon
                        size: 18 * root.s
                        color: row.selected || row.isArmed ? row.tone : root.inkDim
                    }
                    Text {
                        anchors.left: glyph.right
                        anchors.right: keycap.left
                        anchors.leftMargin: 12 * root.s
                        anchors.rightMargin: 8 * root.s
                        anchors.verticalCenter: parent.verticalCenter
                        text: row.isArmed ? row.modelData.label + "? Press again" : row.modelData.label
                        color: row.isArmed ? Theme.error : row.selected ? root.ink : Theme.surfaceText
                        font.family: Theme.fontPrimary
                        font.pixelSize: Theme.h4 * root.s
                        font.weight: row.selected || row.isArmed ? Font.Medium : Font.Normal
                        elide: Text.ElideRight
                    }
                    // the action's letter
                    Rectangle {
                        id: keycap
                        anchors.right: parent.right
                        anchors.rightMargin: 10 * root.s
                        anchors.verticalCenter: parent.verticalCenter
                        width: 18 * root.s
                        height: 18 * root.s
                        color: "transparent"
                        border.width: 1
                        border.color: row.selected || row.isArmed ? row.tone : root.line
                        Text {
                            anchors.centerIn: parent
                            text: row.modelData.key
                            color: row.selected || row.isArmed ? row.tone : root.inkDim
                            font.family: Theme.fontPrimary
                            font.pixelSize: Theme.h6 * root.s
                            font.weight: Font.Medium
                        }
                    }
                    HoverHandler { id: hover }
                    TapHandler { onTapped: root.run(row.index) }
                }
            }
        }

        Rectangle { width: parent.width; height: 1; color: root.line }
        Item {
            width: parent.width
            height: hints.implicitHeight
            PopoutKeyHints {
                id: hints
                visible: root.armed < 0
                s: root.s
                keys: [["↵", "run"], ["↑↓", "move"], ["esc", "close"]]
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: root.armed >= 0
                text: "Press again to confirm"
                color: Theme.error
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h6 * root.s
            }
        }
    }
}
