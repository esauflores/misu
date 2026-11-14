pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import qs.services
import "../../popouts"
import "../.."

// Calendar card, flat like the others: the 力 CALENDAR title, today in large
// type (the date, its weekday and month, the time at the right), a hairline,
// then the month grid. The clock ticks only while open.
Item {
    id: root

    property real s: 1
    property bool open: false

    readonly property real pad: 11 * root.s
    readonly property real indent: 7 * root.s
    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property color inkDim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)
    readonly property color line: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.14)
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
        spacing: 8 * root.s

        PopoutTitle {
            text: "CALENDAR"
            s: root.s
        }

        // today
        Item {
            width: parent.width
            height: day.implicitHeight

            Text {
                id: day
                anchors.left: parent.left
                anchors.leftMargin: root.indent
                text: clock.date.getDate()
                color: root.ink
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h1 * root.s
                font.weight: Font.Medium
                font.features: ({ "tnum": 1 })
            }
            Column {
                anchors.left: day.right
                anchors.leftMargin: 10 * root.s
                anchors.verticalCenter: day.verticalCenter
                spacing: 1 * root.s
                Text {
                    text: root.loc.standaloneDayName(clock.date.getDay(), Locale.LongFormat)
                    color: root.ink
                    font.family: Theme.fontPrimary
                    font.pixelSize: Theme.h4 * root.s
                    font.weight: Font.DemiBold
                }
                Text {
                    text: root.loc.standaloneMonthName(clock.date.getMonth(), Locale.LongFormat) + " " + clock.date.getFullYear()
                    color: root.inkDim
                    font.family: Theme.fontPrimary
                    font.pixelSize: Theme.h6 * root.s
                }
            }
            Text {
                anchors.right: parent.right
                anchors.rightMargin: root.indent
                anchors.verticalCenter: day.verticalCenter
                text: TimeFormat.format(clock.date)
                color: root.inkDim
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h3 * root.s
                font.features: ({ "tnum": 1 })
            }
        }

        Rectangle {
            width: parent.width
            height: 1
            color: root.line
        }

        CalendarGrid {
            width: parent.width
            s: root.s
            open: root.open
        }
    }
}
