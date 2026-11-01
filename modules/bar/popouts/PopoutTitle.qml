pragma ComponentBehavior: Bound

import QtQuick
import qs.services
import ".."

// A card's head label led by the 力 seal: the plugin cards' shared title,
// uppercase in the dim ink.
Row {
    id: title

    property string text: ""
    property real s: 1
    property color color: Theme.surfaceVariantText

    spacing: 6 * title.s

    BrandMark {
        anchors.verticalCenter: parent.verticalCenter
        size: 10 * title.s
        color: Theme.vermLit
    }
    Text {
        anchors.verticalCenter: parent.verticalCenter
        text: title.text
        color: title.color
        font.family: Theme.fontPrimary
        font.pixelSize: 9 * title.s
        font.letterSpacing: 1.6
        font.weight: Font.Medium
    }
}
