pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import qs.services
import "../../popouts"
import "../.."

// Calendar card: a centred head (the 力 seal and today's weekday) over the
// month grid (QsCalendarEmbed, which pages months and snaps back to today on a
// day roll). The clock ticks only while open, so the weekday rolls at midnight.
Item {
    id: root

    property real s: 1
    property bool open: false

    readonly property real pad: 12 * root.s
    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property var loc: Qt.locale()

    implicitWidth: 272 * root.s
    implicitHeight: content.implicitHeight + root.pad * 2

    SystemClock {
        id: clock
        precision: SystemClock.Minutes
        enabled: root.open
    }

    PopoutCard { anchors.fill: parent }

    Column {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: root.pad
        spacing: 10 * root.s

        // the 力 seal and today's weekday; the date is marked in the grid below.
        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 6 * root.s
            BrandMark {
                anchors.verticalCenter: parent.verticalCenter
                size: 11 * root.s
                color: Theme.vermLit
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: root.loc.standaloneDayName(clock.date.getDay(), Locale.LongFormat).toUpperCase()
                color: Theme.vermLit
                font.family: Theme.fontPrimary
                font.pixelSize: 10 * root.s
                font.letterSpacing: 1.5
            }
        }

        CalendarGrid {
            width: parent.width
            s: root.s
            open: root.open
            framed: false
        }
    }
}
