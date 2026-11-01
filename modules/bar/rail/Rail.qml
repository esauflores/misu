pragma ComponentBehavior: Bound

import QtQuick
import qs.services

// The right-edge rail: a fixed-thickness band holding the top, centre and
// bottom zones (Config.rail). It is content inside the frame overlay, not a
// surface. A hidden rail slides off the edge; the overlay keeps a 1px strip
// there, so pointer proximity reveals it without re-reserving the edge.
Item {
    id: root

    required property bool revealed
    signal menuRequested(string id, rect ownerRect)

    readonly property var rail: Config.rail
    readonly property real band: rail.size
    readonly property bool shown: revealed || bandHover.hovered
    // Inter-widget gap and the room left at the screen's top and bottom.
    readonly property real gap: 4 * Math.min(1, rail.size / 48)
    readonly property real endMargin: 8
    // Widgets scale down on a rail thinner than the 48px reference.
    readonly property real widgetScale: Math.min(1, rail.size / 48)

    width: band
    height: parent.height
    anchors.right: parent.right

    HoverHandler { id: bandHover }

    Component {
        id: slot
        RailWidget {
            edge: "right"
            scale: root.widgetScale
            onMenuRequested: (id, ownerRect) => root.menuRequested(id, ownerRect)
        }
    }

    Item {
        x: root.shown ? 0 : root.band
        Behavior on x { NumberAnimation { duration: Motion.barReveal; easing.type: Motion.barRevealCurve } }
        y: root.endMargin + 1
        width: root.width
        height: root.height - root.endMargin * 2 - 3

        RailZone { id: top; anchors.fill: parent; ids: root.rail.top; align: "start"; spacing: root.gap; delegate: slot }
        RailZone {
            anchors.fill: parent; ids: root.rail.center; align: "center"; spacing: root.gap; delegate: slot
            leadExtent: top.implicitHeight
            tailExtent: bottom.implicitHeight
            z: 1
        }
        RailZone { id: bottom; anchors.fill: parent; ids: root.rail.bottom; align: "end"; spacing: root.gap; delegate: slot }
    }
}
