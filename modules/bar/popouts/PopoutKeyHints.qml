pragma ComponentBehavior: Bound

import QtQuick
import qs.services
import ".."

// A card's key-hint footer: each [key, what] pair as a hairline keycap and a
// dim label, e.g. [["↵", "open"], ["esc", "close"]]. A key written
// "icon:<name>" draws that Material Symbol instead, e.g. "icon:open_with". Used by the launcher,
// clipboard, wallpaper and session cards.
Row {
    id: hints

    property var keys: []
    property real s: 1

    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property color inkDim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)

    spacing: 14 * hints.s

    Repeater {
        model: hints.keys
        delegate: Row {
            id: hint
            required property var modelData
            spacing: 5 * hints.s
            readonly property bool isIcon: hint.modelData[0].startsWith("icon:")
            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: Math.max(height, (hint.isIcon ? keyIcon.implicitWidth : keyText.implicitWidth) + 8 * hints.s)
                height: 16 * hints.s
                color: "transparent"
                border.width: 1
                border.color: Qt.rgba(hints.ink.r, hints.ink.g, hints.ink.b, 0.14)
                Text {
                    id: keyText
                    anchors.centerIn: parent
                    visible: !hint.isIcon
                    text: hint.isIcon ? "" : hint.modelData[0]
                    color: hints.ink
                    font.family: Theme.fontPrimary
                    font.pixelSize: Theme.h6 * hints.s
                }
                MaterialIcon {
                    id: keyIcon
                    anchors.centerIn: parent
                    visible: hint.isIcon
                    name: hint.isIcon ? hint.modelData[0].slice(5) : ""
                    size: 11 * hints.s
                    color: hints.ink
                }
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: hint.modelData[1]
                color: hints.inkDim
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h6 * hints.s
            }
        }
    }
}
