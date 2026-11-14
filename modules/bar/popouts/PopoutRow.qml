pragma ComponentBehavior: Bound

import QtQuick
import qs.services

// A list card's row (launcher, clipboard): plain-ink fill when selected or
// hovered (the palette's primary reads blue on thin lines) and an ink edge
// on the selected one. Its children are the row's content; a click emits
// `tapped`.
Item {
    id: row

    property bool selected: false
    property real s: 1
    readonly property bool hovered: hover.hovered
    signal tapped()

    readonly property color ink: Theme.ink(Theme.effectiveSurface)

    Rectangle {
        anchors.fill: parent
        color: Qt.rgba(row.ink.r, row.ink.g, row.ink.b, row.selected ? 0.08 : row.hovered ? 0.04 : 0)
    }
    Rectangle {
        visible: row.selected
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        width: 2 * row.s
        color: row.ink
    }
    HoverHandler { id: hover }
    TapHandler { onTapped: row.tapped() }
}
