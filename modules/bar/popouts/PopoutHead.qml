pragma ComponentBehavior: Bound

import QtQuick
import qs.services

// A list card's head: the 力 title on the left, a dim note (a count) on the
// right.
Item {
    id: head

    property string title: ""
    property string note: ""
    property real s: 1

    width: parent ? parent.width : 0
    height: 20 * head.s

    PopoutTitle {
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        text: head.title
        s: head.s
    }
    Text {
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: head.note
        color: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)
        font.family: Theme.fontPrimary
        font.pixelSize: Theme.h6 * head.s
        font.letterSpacing: 0.6
    }
}
