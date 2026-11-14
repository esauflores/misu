pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import qs.services

// misu: workspaces as a strip of dots, the shown one in the primary colour.
// Click switches workspace, vertical wheel cycles. Contract 03 sec 2.3/2.4.
Item {
    id: root

    required property string edge
    required property real scale

    readonly property bool horizontal: edge === "top" || edge === "bottom"
    readonly property real cross: 48 * scale

    // Five permanent dots, plus later workspaces while they hold windows or
    // are shown (wm always has nine).
    readonly property int fixedCount: 5
    readonly property string _namesKey: {
        const extra = new Set(Wm.workspaces.filter(w => w.number > root.fixedCount
            && (w.occupied || w.active || w.focused)).map(w => w.number));
        const fixed = Array.from({ length: root.fixedCount }, (_, i) => i + 1);
        return fixed.concat([...extra].sort((a, b) => a - b)).join("\u0000");
    }
    // A string model only notifies (and rebuilds the dots) when the set of
    // names actually changes.
    readonly property var entries: _namesKey.split("\u0000")

    // Active set, keyed by name like the dots: each output's shown workspace.
    // misu: each workspace's own `active` flag (shown on its output), on a
    // screen the user sees.
    readonly property var activeIds: {
        const s = {};
        for (const w of Wm.workspaces)
            if (w.active)
                s[w.name] = true;
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
