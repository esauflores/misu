pragma ComponentBehavior: Bound

import QtQuick
import qs.services
import "../../rail"

// Battery rail button: charge level when present, an unknown-battery mark when
// no battery is connected.
Item {
    id: root

    property string edge: "right"
    property real scale: 1
    property string widgetId: "battery"
    signal menuRequested(string id, rect ownerRect)

    implicitWidth: btn.implicitWidth
    implicitHeight: btn.implicitHeight

    function glyph() {
        if (!Battery.present) return "battery_unknown";
        const pct = Battery.pct;
        if (Battery.charging || Battery.full) {
            const steps = [20, 30, 50, 60, 80, 90];
            const c = steps.filter(v => v <= pct).pop();
            return pct > 95 || !c ? (pct > 95 ? "battery_charging_full" : "battery_charging_20") : "battery_charging_" + c;
        }
        return pct > 95 ? "battery_full" : "battery_" + Math.round(pct / 100 * 6) + "_bar";
    }

    RailButton {
        id: btn
        anchors.centerIn: parent
        edge: root.edge
        scale: root.scale
        icon: root.glyph()
        iconColor: Battery.present ? (Battery.low ? Theme.error : btn.foreground) : Theme.surfaceVariantText
        onClicked: root.menuRequested("battery", Qt.rect(btn.x, btn.y, btn.width, btn.height))
    }
}
