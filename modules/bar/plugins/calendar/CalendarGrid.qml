pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import qs.services
import "../.."

// The calendar card's month grid, flat on the card: the month's name (a click
// goes back to today's month) with the arrows that page months at its right,
// the weekday initials, then the days, today filled. Days of the months
// either side are left out. Snaps back to today's month on a day roll.
Item {
    id: root

    property real s: 1
    property bool open: false

    readonly property var loc: Qt.locale()
    readonly property int weekStart: root.loc.firstDayOfWeek % 7
    readonly property color ink: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceText)
    readonly property color inkDim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)

    implicitHeight: cal.implicitHeight

    SystemClock {
        id: clock
        precision: SystemClock.Minutes
        enabled: root.open
    }

    readonly property date today: clock.date
    property int viewYear: today.getFullYear()
    property int viewMonth: today.getMonth()
    property int rollKey: -1
    onTodayChanged: {
        const k = root.today.getFullYear() * 10000 + root.today.getMonth() * 100 + root.today.getDate();
        if (k !== root.rollKey) {
            root.rollKey = k;
            root.home();
        }
    }
    readonly property bool atHome: root.viewYear === root.today.getFullYear() && root.viewMonth === root.today.getMonth()
    function home() {
        root.viewYear = root.today.getFullYear();
        root.viewMonth = root.today.getMonth();
    }

    readonly property int offset: {
        const firstDow = new Date(root.viewYear, root.viewMonth, 1).getDay();
        return (firstDow - root.weekStart + 7) % 7;
    }
    readonly property int monthLen: new Date(root.viewYear, root.viewMonth + 1, 0).getDate()
    readonly property int rows: Math.ceil((offset + monthLen) / 7)

    function shiftMonth(delta) {
        let m = root.viewMonth + delta;
        let y = root.viewYear;
        while (m < 0)  { m += 12; y -= 1; }
        while (m > 11) { m -= 12; y += 1; }
        root.viewMonth = m;
        root.viewYear = y;
    }

    Column {
        id: cal
        width: root.width
        spacing: 6 * root.s

        // month, arrows
        Item {
            width: parent.width
            height: 26 * root.s

            Text {
                anchors.left: parent.left
                anchors.leftMargin: 7 * root.s
                anchors.verticalCenter: parent.verticalCenter
                text: root.loc.standaloneMonthName(root.viewMonth, Locale.LongFormat) + " " + root.viewYear
                color: monthHover.hovered && !root.atHome ? root.ink : (root.atHome ? root.ink : root.inkDim)
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h5 * root.s
                font.weight: Font.DemiBold
                HoverHandler { id: monthHover; enabled: !root.atHome; cursorShape: Qt.PointingHandCursor }
                TapHandler { enabled: !root.atHome; onTapped: root.home() }
            }
            Row {
                anchors.right: parent.right
                anchors.rightMargin: 4 * root.s
                anchors.verticalCenter: parent.verticalCenter
                spacing: 12 * root.s
                Repeater {
                    model: [{ icon: "chevron_left", by: -1 }, { icon: "chevron_right", by: 1 }]
                    MaterialIcon {
                        id: arrow
                        required property var modelData
                        name: arrow.modelData.icon
                        size: 20 * root.s
                        color: arrowArea.containsMouse ? root.ink : root.inkDim
                        MouseArea {
                            id: arrowArea
                            anchors.fill: parent
                            anchors.margins: -5 * root.s
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.shiftMonth(arrow.modelData.by)
                        }
                    }
                }
            }
        }

        // weekday initials
        Row {
            id: weekdays
            width: parent.width
            Repeater {
                model: 7
                delegate: Item {
                    required property int index
                    width: weekdays.width / 7
                    height: 20 * root.s
                    Text {
                        anchors.centerIn: parent
                        text: root.loc.standaloneDayName((root.weekStart + parent.index) % 7, Locale.NarrowFormat)
                        color: root.inkDim
                        font.family: Theme.fontPrimary
                        font.pixelSize: Theme.h6 * root.s
                        font.weight: Font.Medium
                    }
                }
            }
        }

        // days
        Grid {
            id: dayGrid
            width: parent.width
            columns: 7
            rowSpacing: 2 * root.s

            Repeater {
                model: root.rows * 7
                delegate: Item {
                    id: cell
                    required property int index
                    readonly property int dayNum: index - root.offset + 1
                    readonly property bool inMonth: dayNum >= 1 && dayNum <= root.monthLen
                    readonly property bool current: inMonth && root.atHome && dayNum === root.today.getDate()
                    width: dayGrid.width / 7
                    height: 28 * root.s

                    Rectangle {
                        anchors.centerIn: parent
                        width: 26 * root.s
                        height: width
                        radius: Theme.radiusWidget
                        visible: cell.current
                        color: Theme.primary
                    }
                    Text {
                        anchors.centerIn: parent
                        visible: cell.inMonth
                        text: cell.dayNum
                        color: cell.current ? Theme.inkOn(Theme.primary, Theme.primaryText, 3.0) : root.ink
                        font.family: Theme.fontPrimary
                        font.pixelSize: Theme.h5 * root.s
                        font.weight: cell.current ? Font.Bold : Font.Normal
                        font.features: ({ "tnum": 1 })
                    }
                }
            }
        }
    }
}
