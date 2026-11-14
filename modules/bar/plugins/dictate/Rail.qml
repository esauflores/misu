pragma ComponentBehavior: Bound

import QtQuick
import qs.services
import "../../rail"

// Subtitles rail button: a subtitles glyph, filled while misu-dictate is
// listening or transcribing. Opens the subtitles card.
Item {
    id: root

    property string edge: "right"
    property real scale: 1
    property string widgetId: "dictate"
    signal menuRequested(string id, rect ownerRect)

    // hidden (RailWidget drops a zero-width item) until the dictate module is installed
    implicitWidth: Dictation.installed ? btn.implicitWidth : 0
    implicitHeight: btn.implicitHeight

    RailButton {
        id: btn
        anchors.centerIn: parent
        edge: root.edge
        scale: root.scale
        icon: "subtitles"
        iconFill: Dictation.active ? 1 : 0
        iconColor: Dictation.active ? Theme.error : btn.foreground
        onClicked: root.menuRequested("dictate", Qt.rect(btn.x, btn.y, btn.width, btn.height))
    }
}
