pragma ComponentBehavior: Bound

import QtQuick
import "../../rail"
import "../.."

// Media rail button: a music note that opens the now-playing card for the
// active MPRIS player. While it plays, the note becomes equalizer bars.
Item {
    id: root

    property string edge: "right"
    property real scale: 1
    property string widgetId: "mpris"
    signal menuRequested(string id, rect ownerRect)

    MprisPlayer { id: mpris }

    implicitWidth: btn.implicitWidth
    implicitHeight: btn.implicitHeight

    RailButton {
        id: btn
        anchors.centerIn: parent
        edge: root.edge
        scale: root.scale
        onClicked: root.menuRequested("mpris", Qt.rect(btn.x, btn.y, btn.width, btn.height))

        MaterialIcon {
            name: mpris.playing ? "graphic_eq" : "music_note"
            color: btn.iconColor
            size: btn.glyphPx
        }
    }
}
