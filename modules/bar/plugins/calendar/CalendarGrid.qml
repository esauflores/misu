pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import qs.services
import "../.."

// Calendar-only embed: the calendar grid from MenuClock, without the weather
// section. MenuClock stacks it above MenuWeather.
Item {
    id: root

    property real s: 1
    property bool open: false
    // false drops the bordered tile for a soft tinted fill, for hosts that
    // already sit on a framed card (the calendar plugin).
    property bool framed: true

    readonly property var loc: Qt.locale()
    readonly property int weekStart: root.loc.firstDayOfWeek % 7

    implicitHeight: calCard.implicitHeight

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
            root.viewYear = root.today.getFullYear();
            root.viewMonth = root.today.getMonth();
        }
    }

    readonly property int offset: {
        const firstDow = new Date(root.viewYear, root.viewMonth, 1).getDay();
        return (firstDow - root.weekStart + 7) % 7;
    }
    readonly property int monthLen: new Date(root.viewYear, root.viewMonth + 1, 0).getDate()
    readonly property int prevMonthLen: new Date(root.viewYear, root.viewMonth, 0).getDate()
    readonly property int rows: Math.ceil((offset + monthLen) / 7)

    function shiftMonth(delta) {
        let m = root.viewMonth + delta;
        let y = root.viewYear;
        while (m < 0)  { m += 12; y -= 1; }
        while (m > 11) { m -= 12; y += 1; }
        root.viewMonth = m;
        root.viewYear = y;
    }

    function isToday(day) {
        return day === root.today.getDate()
            && root.viewMonth === root.today.getMonth()
            && root.viewYear === root.today.getFullYear();
    }

    Rectangle {
        id: calCard
        width: root.width
        radius: Theme.radiusWidget
        color: root.framed ? Theme.surface : Qt.rgba(Theme.surfaceText.r, Theme.surfaceText.g, Theme.surfaceText.b, 0.05)
        border.width: root.framed ? Theme.borderWidth : 0
        border.color: Theme.outline
        implicitHeight: cal.implicitHeight + 2 * (10 * root.s)
        SumiEdge { visible: root.framed }

        Column {
            id: cal
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: 10 * root.s
            spacing: 6 * root.s

            // Month nav header
            Item {
                width: parent.width
                height: 28 * root.s

                MaterialIcon {
                    id: prevNav
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    size: 18 * root.s
                    name: "chevron_left"
                    color: Theme.inkOn(Theme.effectiveSurface, prevArea.containsMouse ? Theme.surfaceText : Theme.surfaceVariantText, 3.0)
                    MouseArea {
                        id: prevArea
                        anchors.fill: parent; anchors.margins: -6 * root.s
                        hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                        onClicked: root.shiftMonth(-1)
                    }
                }
                Text {
                    anchors.centerIn: parent
                    text: root.loc.standaloneMonthName(root.viewMonth, Locale.LongFormat) + " " + root.viewYear
                    color: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceText)
                    font.family: Theme.fontPrimary
                    font.pixelSize: Theme.fontMd * root.s
                    font.weight: Font.Bold
                }
                MaterialIcon {
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    size: 18 * root.s
                    name: "chevron_right"
                    color: Theme.inkOn(Theme.effectiveSurface, nextArea.containsMouse ? Theme.surfaceText : Theme.surfaceVariantText, 3.0)
                    MouseArea {
                        id: nextArea
                        anchors.fill: parent; anchors.margins: -6 * root.s
                        hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                        onClicked: root.shiftMonth(1)
                    }
                }
            }

            // Weekday header row
            Row {
                id: weekdays
                width: parent.width
                Repeater {
                    model: 7
                    delegate: Item {
                        required property int index
                        width: weekdays.width / 7
                        height: 18 * root.s
                        Text {
                            anchors.centerIn: parent
                            text: root.loc.standaloneDayName((root.weekStart + parent.index) % 7, Locale.NarrowFormat)
                            color: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)
                            font.family: Theme.fontPrimary
                            font.pixelSize: Theme.fontSm * root.s
                            font.weight: Font.Medium
                        }
                    }
                }
            }

            // Day grid
            Grid {
                id: dayGrid
                width: parent.width
                columns: 7
                rowSpacing: 2 * root.s
                columnSpacing: 0

                Repeater {
                    model: root.rows * 7
                    delegate: Item {
                        id: cell
                        required property int index
                        readonly property int dayNum: index - root.offset + 1
                        readonly property bool inMonth: dayNum >= 1 && dayNum <= root.monthLen
                        readonly property bool current: inMonth && root.isToday(dayNum)
                        readonly property int ghostNum: dayNum < 1
                            ? root.prevMonthLen + dayNum
                            : dayNum - root.monthLen
                        width: dayGrid.width / 7
                        height: 26 * root.s

                        Rectangle {
                            anchors.centerIn: parent
                            width: 24 * root.s; height: 24 * root.s
                            radius: Theme.radiusWidget
                            visible: cell.current
                            color: Theme.primary
                        }
                        Text {
                            anchors.centerIn: parent
                            text: cell.inMonth ? cell.dayNum : cell.ghostNum
                            color: cell.current ? Theme.inkOn(Theme.primary, Theme.primaryText, 3.0)
                                : cell.inMonth ? Theme.inkOn(Theme.effectiveSurface, Theme.surfaceText)
                                               : Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)
                            opacity: cell.inMonth ? 1.0 : 0.4
                            font.family: Theme.fontPrimary
                            font.pixelSize: Theme.fontSm * root.s
                            font.weight: cell.current ? Font.Bold : Font.Normal
                        }
                    }
                }
            }
        }
    }
}
