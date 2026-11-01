pragma ComponentBehavior: Bound

import QtQuick
import qs.services
import "../../rail"

// One button for every AI plan; the glyph turns amber or red when any window
// is running high or critical.
Item {
    id: root

    property string edge: "right"
    property real scale: 1
    property string widgetId: "ai-usage"
    signal menuRequested(string id, rect ownerRect)

    implicitWidth: btn.implicitWidth
    implicitHeight: btn.implicitHeight

    RailButton {
        id: btn
        anchors.centerIn: parent
        edge: root.edge
        scale: root.scale
        icon: "data_usage"
        iconColor: AiUsage.worst === "critical" ? Theme.error
            : AiUsage.worst === "high" ? Qt.tint(Theme.primary, Qt.rgba(Theme.error.r, Theme.error.g, Theme.error.b, 0.55))
            : btn.foreground
        onClicked: root.menuRequested("aiusage", Qt.rect(btn.x, btn.y, btn.width, btn.height))
    }
}
