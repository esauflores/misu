pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import qs.services
import "../../rail"

// The rail clock: a SystemClock ticks the readout, HH:mm on one line on a
// horizontal bar, hour over minute on a vertical one. A click opens the
// calendar card.
Item {
    id: root

    property string edge: "right"
    property real scale: 1
    property string widgetId: "clock"
    signal menuRequested(string id, rect ownerRect)

    readonly property bool horizontal: root.edge === "top" || root.edge === "bottom"

    implicitWidth: btn.implicitWidth
    implicitHeight: btn.implicitHeight

    SystemClock {
        id: clock
        precision: SystemClock.Minutes
    }

    function timeText() {
        const d = clock.date;
        if (root.horizontal)
            return TimeFormat.format(d);
        const hhmm = TimeFormat.digits(d);
        return hhmm.slice(0, 2) + "\n" + hhmm.slice(2);
    }

    RailButton {
        id: btn
        anchors.centerIn: parent
        edge: root.edge
        scale: root.scale
        label: root.timeText()
        labelSize: 13
        labelLineHeight: 17.2
        onClicked: root.menuRequested("calendar", Qt.rect(btn.x, btn.y, btn.width, btn.height))
    }
}
