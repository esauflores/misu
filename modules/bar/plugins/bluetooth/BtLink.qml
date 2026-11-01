pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Bluetooth

// Bluetooth on top of Quickshell.Bluetooth: device names, glyphs, battery,
// connection age, the pair-trust-connect command a tap runs, adapter power,
// and reference-counted discovery.
Singleton {
    id: root

    function devices() { return Bluetooth.devices ? Bluetooth.devices.values : []; }
    function deviceFor(mac) {
        const want = (mac || "").toUpperCase();
        for (const d of root.devices())
            if (d && (d.address || "").toUpperCase() === want) return d;
        return null;
    }

    function label(d) {
        if (!d) return "";
        return d.name || d.deviceName || d.address || "";
    }
    function batteryLevel(d) {
        return d && d.batteryAvailable ? Math.round(d.battery * 100) : -1;
    }
    function _kind(d) { return d && d.icon ? String(d.icon) : ""; }
    function glyphFor(d) {
        const k = root._kind(d);
        if (k.indexOf("audio") >= 0 || k.indexOf("head") >= 0) return "headphones";
        if (k.indexOf("mouse") >= 0) return "mouse";
        if (k.indexOf("keyboard") >= 0) return "keyboard";
        if (k.indexOf("phone") >= 0) return "smartphone";
        if (k.indexOf("gaming") >= 0 || k.indexOf("joystick") >= 0) return "sports_esports";
        if (k.indexOf("watch") >= 0) return "watch";
        return "bluetooth";
    }
    function typeLabel(d) {
        const k = root._kind(d);
        if (k.indexOf("headset") >= 0) return "Headset";
        if (k.indexOf("headphone") >= 0 || k.indexOf("audio") >= 0) return "Audio";
        if (k.indexOf("mouse") >= 0) return "Mouse";
        if (k.indexOf("keyboard") >= 0) return "Keyboard";
        if (k.indexOf("phone") >= 0) return "Phone";
        if (k.indexOf("gaming") >= 0) return "Controller";
        return "Device";
    }

    // address -> ms epoch when we first saw it connected
    property var connectedSince: ({})
    property bool watching: false
    function watch(on) { root.watching = on; root.sweep(); }
    function sweep() {
        const next = {};
        const now = Date.now();
        for (const d of root.devices())
            if (d && d.connected) next[d.address] = root.connectedSince[d.address] || now;
        root.connectedSince = next;
    }
    Timer { interval: 30000; repeat: true; running: true; onTriggered: root.sweep() }
    Connections {
        target: Bluetooth.devices
        function onValuesChanged() { root.sweep(); }
    }
    function durationFor(addr) {
        const t = root.connectedSince[addr];
        return t ? Math.max(0, Date.now() - t) : -1;
    }
    function durationText(ms) {
        if (ms < 0) return "";
        const m = Math.floor(ms / 60000);
        if (m < 1) return "just now";
        if (m < 60) return m + "m";
        const h = Math.floor(m / 60);
        return h + "h " + (m % 60) + "m";
    }

    // Pair (if needed), trust and connect, with one retry; the last line of
    // output on failure is shown to the user.
    function linkCommand(addr) {
        const a = String(addr).replace(/[^0-9A-Fa-f:]/g, "");
        return ["sh", "-c",
            "a=" + a + "; bluetoothctl info $a | grep -q 'Paired: yes' || bluetoothctl --timeout 20 pair $a >/dev/null;"
            + " bluetoothctl trust $a >/dev/null;"
            + " bluetoothctl --timeout 15 connect $a | grep -q 'successful' || { sleep 2; bluetoothctl --timeout 15 connect $a | tail -n1; bluetoothctl info $a | grep -q 'Connected: yes'; }"];
    }

    function togglePower() {
        const a = Bluetooth.defaultAdapter;
        if (a) a.enabled = !a.enabled;
    }

    // The adapter scans while any owner asks.
    property var discoveryOwners: []
    function setDiscovering(owner, adapter, on) {
        const list = root.discoveryOwners.filter(o => o !== owner);
        if (on) list.push(owner);
        root.discoveryOwners = list;
        if (adapter) adapter.discovering = list.length > 0 && adapter.enabled;
    }
}
