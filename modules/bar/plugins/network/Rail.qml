pragma ComponentBehavior: Bound

import QtQuick
import qs.services
import "../../rail"

// Network rail button: wired, wifi strength (>75 excellent, >50 good, >25 ok,
// >0 weak) or the radio state; opens the network card.
Item {
    id: root

    property string edge: "right"
    property real scale: 1
    property string widgetId: "network"
    signal menuRequested(string id, rect ownerRect)

    implicitWidth: btn.implicitWidth
    implicitHeight: btn.implicitHeight

    function glyph() {
        if (Network.kind === "ethernet")
            return "lan";
        if (Network.kind === "wifi") {
            const s = Network.level * 100;
            return s > 75 ? "network_wifi" : s > 50 ? "network_wifi_3_bar"
                : s > 25 ? "network_wifi_2_bar" : s > 0 ? "network_wifi_1_bar" : "signal_wifi_0_bar";
        }
        return Network.wifiRadio ? "signal_wifi_statusbar_not_connected" : "wifi_off";
    }

    RailButton {
        id: btn
        anchors.centerIn: parent
        edge: root.edge
        scale: root.scale
        icon: root.glyph()
        onClicked: root.menuRequested("network", Qt.rect(btn.x, btn.y, btn.width, btn.height))
    }
}
