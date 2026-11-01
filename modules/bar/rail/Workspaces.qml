pragma ComponentBehavior: Bound

import QtQuick
import qs.services

// misu: workspaces as a strip of dots, the shown one in the primary colour.
// Click switches workspace, vertical wheel cycles. Contract 03 sec 2.3/2.4.
Item {
    id: root

    required property string edge
    required property real scale

    readonly property bool horizontal: edge === "top" || edge === "bottom"
    readonly property real cross: 48 * scale

    // misu: a fixed strip of 5 workspaces, always all five. Sway destroys an
    // empty workspace as soon as it loses focus, so deriving the dots from
    // Wm.workspaces would make the strip change length. The names are
    // synthesized instead; `workspace number N` creates a workspace that does
    // not exist yet, so clicking any dot works. Workspaces past the fifth are
    // appended while sway has them (it only keeps non-empty or shown ones).
    readonly property int fixedCount: 5
    readonly property string _namesKey: {
        const names = [];
        const seen = {};
        for (let i = 1; i <= root.fixedCount; ++i) {
            names.push(String(i));
            seen[String(i)] = true;
        }
        const extra = [];
        const ws = Wm.workspaces || [];
        for (let i = 0; i < ws.length; ++i) {
            const w = ws[i];
            const name = w && !w.special ? (w.name || "") : "";
            if (name === "" || seen[name])
                continue;
            seen[name] = true;
            extra.push(name);
        }
        extra.sort((a, b) => Number(a) - Number(b));
        return names.concat(extra).join("\u0000");
    }
    // A string model only notifies (and rebuilds the dots) when the set of
    // names actually changes.
    readonly property var entries: _namesKey.split("\u0000")

    // Active set, keyed by name like the dots: each output's shown workspace.
    // misu: each workspace's own `active` flag (shown on its output).
    readonly property var activeIds: {
        const s = {};
        const ws = Wm.workspaces || [];
        for (let i = 0; i < ws.length; ++i)
            if (ws[i] && ws[i].active)
                s[ws[i].name] = true;
        return s;
    }

    implicitWidth: horizontal ? strip.implicitWidth : Math.max(cross, strip.implicitWidth)
    implicitHeight: horizontal ? Math.max(cross, strip.implicitHeight) : strip.implicitHeight

    Grid {
        id: strip
        anchors.centerIn: parent
        columns: root.horizontal ? root.entries.length : 1
        spacing: 4 * root.scale

        Repeater {
            model: root.entries
            delegate: Item {
                id: pill
                required property string modelData
                readonly property bool isActive: root.activeIds[pill.modelData] === true
                readonly property real dotSize: 6 * root.scale
                readonly property real extent: dotSize + 2 * (root.horizontal ? 5 : 6) * root.scale
                readonly property real pillCross: 32 * root.scale

                width: root.horizontal ? extent : pillCross
                height: root.horizontal ? pillCross : extent

                Rectangle {
                    anchors.centerIn: parent
                    width: pill.dotSize
                    height: pill.dotSize
                    radius: width / 2
                    color: pill.isActive ? Theme.primary
                        : Qt.rgba(Theme.surfaceText.r, Theme.surfaceText.g, Theme.surfaceText.b, 0.28)
                    Behavior on color {
                        enabled: !Motion.reduce
                        ColorAnimation { duration: Motion.fast }
                    }
                }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Wm.focusWorkspace(pill.modelData)
                }
            }
        }
    }

    // Vertical wheel: up -> previous workspace, down -> next.
    WheelHandler {
        onWheel: event => Wm.cycleWorkspace(event.angleDelta.y > 0 ? -1 : 1)
    }
}
