pragma ComponentBehavior: Bound

import QtQuick
import Quickshell.Bluetooth
import Quickshell.Io
import "../../popouts"
import "../.."
import qs.services

// Bluetooth popout: a frame-edge card (the shared PopoutCard skin, so it opens,
// melts and dismisses exactly like the music card) leading with the hero for the
// focused device, then flat rows (your other devices, then named ones nearby
// while scanning) to switch, connect or
// pair the rest; power and scan ride the head. Everything is live off
// Quickshell.Bluetooth and the Audio graph; connecting a bonded device is
// instant, pairing a fresh one shells the reliable bluetoothctl pair/trust/
// connect chain. Material clarity, TUI density; never grabs the keyboard.
Item {
    id: root

    property real s: 1
    property bool open: false

    readonly property real pad: 11 * root.s
    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property color inkDim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)
    readonly property color line: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.14)

    readonly property var adapter: Bluetooth.defaultAdapter
    readonly property bool hasAdapter: root.adapter !== null
    readonly property bool adapterOn: root.hasAdapter && root.adapter.enabled
    readonly property bool blocked: root.hasAdapter && typeof BluetoothAdapterState !== "undefined"
        && root.adapter.state === BluetoothAdapterState.Blocked
    readonly property bool discovering: root.adapterOn && root.adapter.discovering

    readonly property var devices: (root.adapterOn && Bluetooth.devices) ? Bluetooth.devices.values : []
    readonly property var known: root.devices.filter(d => d && (d.paired || d.bonded))
    // a scanned device with no name of its own carries its address as the
    // name (7B-A3-4C-…); those are beacons and strangers' gadgets, not
    // something to pair with from here, so they stay out of the list.
    function nameless(d) {
        return !d.name || /^([0-9A-F]{2}[-:]){5}[0-9A-F]{2}$/i.test(d.name);
    }
    readonly property var found: root.devices.filter(d => d && !(d.paired || d.bonded) && !root.nameless(d))

    property string focusAddr: ""
    readonly property var connectedDev: {
        for (let i = 0; i < root.devices.length; i++)
            if (root.devices[i] && root.devices[i].connected)
                return root.devices[i];
        return null;
    }
    function deviceByAddr(a) {
        if (!a)
            return null;
        for (let i = 0; i < root.devices.length; i++)
            if (root.devices[i] && ((root.devices[i].address || "") + "").toUpperCase() === a.toUpperCase())
                return root.devices[i];
        return null;
    }
    readonly property var focusDev: root.deviceByAddr(root.focusAddr) || root.connectedDev
        || (root.known.length > 0 ? root.known[0] : null)

    // the rows under the hero: your other paired devices, then (while
    // scanning) the named devices nearby, the first few until "N more".
    function notFocus(d) {
        const fa = root.focusDev ? ((root.focusDev.address || "") + "").toUpperCase() : "";
        return ((d.address || "") + "").toUpperCase() !== fa;
    }
    readonly property var railKnown: root.known.filter(d => root.notFocus(d))
    readonly property var railFound: root.discovering ? root.found.filter(d => root.notFocus(d)) : []
    readonly property var railDevices: root.railKnown.concat(root.railFound)
    readonly property int shortList: 6
    property bool showAll: false
    readonly property var shownFound: root.showAll ? root.railFound
        : root.railFound.slice(0, root.shortList)

    property string busyAddr: ""
    property string errorText: ""
    readonly property bool focusBusy: root.focusDev
        && root.busyAddr === ((root.focusDev.address || "") + "")

    onOpenChanged: {
        BtLink.watch(root.open);
        if (!root.open) {
            root.stopScan();
            root.showAll = false;
            root.focusAddr = "";
            root.busyAddr = "";
            root.errorText = "";
        }
    }
    Component.onDestruction: { BtLink.watch(false); root.stopScan(); }
    onDiscoveringChanged: { if (root.discovering) scanStop.restart(); else scanStop.stop(); }

    // --- actions --------------------------------------------------------------
    function toggleAdapter() {
        BtLink.togglePower();
    }

    function toggleScan() {
        if (!root.adapterOn)
            return;
        BtLink.setDiscovering(root, root.adapter, !root.discovering);
    }
    function stopScan() { BtLink.setDiscovering(root, root.adapter, false); scanStop.stop(); }
    Timer { id: scanStop; interval: 30000; onTriggered: root.stopScan() }

    function primary(d) {
        if (!d)
            return;
        if (d.connected) {
            d.disconnect();
            return;
        }
        if (d.blocked)
            d.blocked = false;
        // Paired and unpaired both go through the same script. Device1.Connect
        // on an existing bond was the dead end in #144/#156: it fails at once
        // when BlueZ holds keys the device has forgotten, and nothing here ever
        // cleared that. BtLink.linkCommand pairs if needed, retries, and
        // rebuilds a bond that will not carry a connection.
        root.link(d);
    }
    function link(d) {
        if (!d || linkProc.running)
            return;
        root.busyAddr = d.address;
        root.errorText = "";
        linkProc.command = BtLink.linkCommand(d.address);
        linkProc.running = false;
        linkProc.running = true;
    }
    Process {
        id: linkProc
        property string collected: ""
        // stderr as well as stdout: the script redirects bluetoothctl's own
        // stderr, but a failure in the script itself (no bash, no
        // bluetoothctl) only ever lands here, and losing it is what left the
        // user with a generic message and no way to tell why.
        stdout: StdioCollector { onStreamFinished: linkProc.collected += this.text }
        stderr: StdioCollector { onStreamFinished: linkProc.collected += this.text }
        onExited: code => {
            if (code !== 0) {
                const lines = linkProc.collected.trim().split("\n");
                const msg = lines.length ? lines[lines.length - 1].trim() : "";
                root.errorText = msg.length ? msg
                    : "Could not connect. Put the device in pairing mode and try again.";
            } else {
                root.errorText = "";
            }
            linkProc.collected = "";
            root.busyAddr = "";
        }
    }
    function tapDevice(d) {
        if (!d)
            return;
        root.focusAddr = d.address;
        root.errorText = "";
        if (!d.connected && !d.pairing && root.busyAddr !== ((d.address || "") + ""))
            root.primary(d);
    }

    implicitWidth: 244 * root.s
    implicitHeight: content.implicitHeight + root.pad * 2

    // the shared card skin: same frame + click-swallow the music card uses.
    PopoutCard { anchors.fill: parent }

    // a device row, flat like the network card's: class glyph, name, and on
    // the right its state (connecting, battery, connected; "Pair" on hover for
    // a new one). Tap focuses it in the hero and connects or pairs.
    component DeviceRow: Item {
        id: row
        property var dev: null
        readonly property bool conn: row.dev && row.dev.connected
        readonly property bool pend: row.dev && (row.dev.pairing
            || root.busyAddr === ((row.dev.address || "") + ""))
        readonly property bool fresh: row.dev && !(row.dev.paired || row.dev.bonded)
        readonly property int battery: BtLink.batteryLevel(row.dev)
        width: parent ? parent.width : 0
        height: 26 * root.s
        Rectangle {
            anchors.fill: parent
            radius: 3 * root.s
            color: rTap.pressed ? Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.16)
                : (rHover.hovered ? Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.08) : "transparent")
            Behavior on color { ColorAnimation { duration: Motion.fast } }
        }
        MaterialIcon {
            id: rIcon
            anchors.left: parent.left
            anchors.leftMargin: 7 * root.s
            anchors.verticalCenter: parent.verticalCenter
            size: 14 * root.s
            name: BtLink.glyphFor(row.dev)
            color: row.conn ? Theme.primary : root.inkDim
        }
        Text {
            anchors.left: rIcon.right
            anchors.leftMargin: 7 * root.s
            anchors.right: rState.left
            anchors.rightMargin: 6 * root.s
            anchors.verticalCenter: parent.verticalCenter
            text: BtLink.label(row.dev)
            color: root.ink
            font.family: Theme.fontPrimary
            font.pixelSize: Theme.h5 * root.s
            font.weight: row.conn ? Font.DemiBold : Font.Normal
            elide: Text.ElideRight
        }
        Text {
            id: rState
            anchors.right: parent.right
            anchors.rightMargin: 8 * root.s
            anchors.verticalCenter: parent.verticalCenter
            text: row.pend ? "…"
                : row.conn ? (row.battery >= 0 ? row.battery + "%" : "Connected")
                : row.fresh && rHover.hovered ? "Pair"
                : ""
            color: root.inkDim
            font.family: Theme.fontPrimary
            font.pixelSize: Theme.h6 * root.s
        }
        HoverHandler { id: rHover; cursorShape: Qt.PointingHandCursor }
        MouseArea { id: rTap; anchors.fill: parent; onClicked: root.tapDevice(row.dev) }
    }

    // --- layout ---------------------------------------------------------------
    Column {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: root.pad
        spacing: 8 * root.s

        // head: label, scan, power.
        Item {
            width: parent.width
            height: 20 * root.s
            PopoutTitle {
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                text: "BLUETOOTH"
                s: root.s
            }
            Row {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                spacing: 5 * root.s
                PopoutHeadButton {
                    visible: root.adapterOn
                    s: root.s
                    glyph: "restart_alt"
                    active: root.discovering
                    spin: root.discovering
                    onClicked: root.toggleScan()
                }
                PopoutHeadButton {
                    s: root.s
                    glyph: "power_settings_new"
                    active: root.adapterOn
                    onClicked: root.toggleAdapter()
                }
            }
        }

        // OFF / BLOCKED plate.
        Column {
            width: parent.width
            spacing: 7 * root.s
            visible: !root.adapterOn
            topPadding: 6 * root.s
            bottomPadding: 4 * root.s
            MaterialIcon {
                anchors.horizontalCenter: parent.horizontalCenter
                size: 27 * root.s
                name: "bluetooth"
                color: root.inkDim
                opacity: 0.55
            }
            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: root.blocked ? "Bluetooth is blocked" : "Bluetooth is off"
                color: root.ink
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h5 * root.s
                font.weight: Font.DemiBold
            }
            PopoutAction {
                anchors.horizontalCenter: parent.horizontalCenter
                s: root.s
                label: root.blocked ? "Unblock" : "Turn on"
                onClicked: root.toggleAdapter()
            }
        }

        // HERO for the focused device.
        BtHero {
            width: parent.width
            visible: root.adapterOn && root.focusDev !== null
            s: root.s
            device: root.focusDev
            busy: root.focusBusy
            onPrimaryActed: root.primary(root.focusDev)
            onForgetActed: {
                if (root.focusDev) {
                    root.focusDev.forget();
                    root.focusAddr = "";
                }
            }
        }

        // pairing error.
        Text {
            width: parent.width
            visible: root.errorText.length > 0
            horizontalAlignment: Text.AlignHCenter
            text: root.errorText
            color: Theme.error
            wrapMode: Text.WordWrap
            font.family: Theme.fontPrimary
            font.pixelSize: Theme.h6 * root.s
        }

        // your other paired devices.
        Rectangle {
            width: parent.width
            height: 1
            color: root.line
            visible: root.adapterOn && root.focusDev !== null && root.railKnown.length > 0
        }
        Column {
            width: parent.width
            spacing: 2 * root.s
            visible: root.adapterOn && root.railKnown.length > 0
            Repeater {
                model: root.railKnown
                delegate: DeviceRow {
                    required property var modelData
                    dev: modelData
                }
            }
        }

        // nearby devices while scanning.
        Rectangle {
            width: parent.width
            height: 1
            color: root.line
            visible: root.adapterOn && root.railFound.length > 0
                && (root.focusDev !== null || root.railKnown.length > 0)
        }
        Column {
            width: parent.width
            spacing: 2 * root.s
            visible: root.adapterOn && root.railFound.length > 0
            Repeater {
                model: root.shownFound
                delegate: DeviceRow {
                    required property var modelData
                    dev: modelData
                }
            }
        }
        Text {
            width: parent.width
            visible: root.adapterOn && root.railFound.length > root.shortList
            leftPadding: 7 * root.s
            text: root.showAll ? "Fewer"
                : "%1 more".arg(root.railFound.length - root.shortList)
            color: moreHover.hovered ? root.ink : root.inkDim
            font.family: Theme.fontPrimary
            font.pixelSize: Theme.h6 * root.s
            Behavior on color { ColorAnimation { duration: 120 } }
            HoverHandler { id: moreHover; cursorShape: Qt.PointingHandCursor }
            TapHandler { onTapped: root.showAll = !root.showAll }
        }

        // EMPTY plate.
        Column {
            width: parent.width
            spacing: 7 * root.s
            visible: root.adapterOn && root.focusDev === null && root.railDevices.length === 0
            topPadding: 6 * root.s
            bottomPadding: 4 * root.s
            MaterialIcon {
                anchors.horizontalCenter: parent.horizontalCenter
                size: 27 * root.s
                name: "bluetooth"
                color: root.inkDim
                opacity: 0.55
            }
            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: root.discovering ? "Scanning…" : "No devices yet"
                color: root.ink
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h5 * root.s
                font.weight: Font.DemiBold
            }
            PopoutAction {
                anchors.horizontalCenter: parent.horizontalCenter
                s: root.s
                enabled: !root.discovering
                label: root.discovering ? "Scanning…" : "Scan"
                onClicked: root.toggleScan()
            }
        }
    }
}
