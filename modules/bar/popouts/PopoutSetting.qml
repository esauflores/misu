pragma ComponentBehavior: Bound

import QtQuick
import ".."
import qs.services

// A setting row shared by the popout cards: its name, dim, on the left and
// its choices as one segmented control on the right, the current one filled
// to a bone plate. options: [{ id, label }]; `picked` gives the chosen id.
// The caller sets `width`.
Item {
    id: root

    property real s: 1
    property string name: ""
    property var options: []
    property string value: ""
    signal picked(string id)

    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property color inkDim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)
    readonly property color line: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.14)

    implicitHeight: 28 * root.s

    Text {
        anchors.left: parent.left
        anchors.leftMargin: 7 * root.s
        anchors.right: box.left
        anchors.rightMargin: 8 * root.s
        anchors.verticalCenter: parent.verticalCenter
        text: root.name
        elide: Text.ElideRight
        color: root.inkDim
        font.family: Theme.fontPrimary
        font.pixelSize: Theme.h5 * root.s
    }
    Rectangle {
        id: box
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        width: seg.width
        height: 21 * root.s
        radius: 3 * root.s
        color: "transparent"
        border.width: Theme.borderWidth
        border.color: root.line

        Row {
            id: seg
            Repeater {
                model: root.options
                Item {
                    id: opt
                    required property var modelData
                    readonly property bool on: root.value === opt.modelData.id
                    width: optText.implicitWidth + 16 * root.s
                    height: 21 * root.s
                    Rectangle {
                        anchors.fill: parent
                        radius: 3 * root.s
                        color: opt.on ? Theme.inverseSurface
                            : (optHover.hovered ? Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.10) : "transparent")
                        Behavior on color { ColorAnimation { duration: Motion.fast } }
                    }
                    Text {
                        id: optText
                        anchors.centerIn: parent
                        text: opt.modelData.label
                        color: opt.on ? Theme.inverseOnSurface : root.ink
                        font.family: Theme.fontPrimary
                        font.pixelSize: Theme.h6 * root.s
                    }
                    HoverHandler { id: optHover; cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: root.picked(opt.modelData.id) }
                }
            }
        }
    }
}
