pragma ComponentBehavior: Bound

import QtQuick
import Quickshell.Bluetooth
import "../../rail"

// Bluetooth rail button: adapter state as the glyph; opens the bluetooth card.
Item {
    id: root

    property string edge: "right"
    property real scale: 1
    property string widgetId: "bluetooth"
    signal menuRequested(string id, rect ownerRect)

    implicitWidth: btn.implicitWidth
    implicitHeight: btn.implicitHeight

    function glyph() {
        const a = Bluetooth.defaultAdapter;
        if (!a)
            return "bluetooth_disabled";
        if (!a.enabled)
            return "bluetooth_disabled";
        return a.devices.values.some(d => d.connected) ? "bluetooth_connected" : "bluetooth";
    }

    RailButton {
        id: btn
        anchors.centerIn: parent
        edge: root.edge
        scale: root.scale
        icon: root.glyph()
        onClicked: root.menuRequested("bluetooth", Qt.rect(btn.x, btn.y, btn.width, btn.height))
    }
}
