pragma ComponentBehavior: Bound

import QtQuick
import qs.services
import "../../rail"

// Notifications rail button: a bell (struck through under Do Not Disturb) that
// opens the notifications card, with a small red dot near its top-right corner
// while there are notifications the card has not shown yet.
Item {
    id: root

    property string edge: "right"
    property real scale: 1
    property string widgetId: "notifications"
    signal menuRequested(string id, rect ownerRect)

    implicitWidth: btn.implicitWidth
    implicitHeight: btn.implicitHeight

    RailButton {
        id: btn
        anchors.centerIn: parent
        edge: root.edge
        scale: root.scale
        // seen notifications leave the plain bell; only unread ones get the dot.
        icon: Notifs.dnd ? "notifications_off" : "notifications"
        onClicked: root.menuRequested("notifications", Qt.rect(btn.x, btn.y, btn.width, btn.height))
    }

    Rectangle {
        visible: Notifs.hasUnread
        width: 5 * btn.glyphScale
        height: 5 * btn.glyphScale
        radius: width / 2
        color: Theme.error
        anchors.horizontalCenter: btn.horizontalCenter
        anchors.verticalCenter: btn.verticalCenter
        anchors.horizontalCenterOffset: 5 * btn.glyphScale
        anchors.verticalCenterOffset: -5 * btn.glyphScale
    }
}
