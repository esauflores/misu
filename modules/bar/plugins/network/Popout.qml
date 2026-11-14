pragma ComponentBehavior: Bound

import QtQuick
import "../../popouts"
import "../.."
import qs.services

// Network popout: a frame-edge card (shared PopoutCard, so it opens and melts
// like the music card) leading with the current link (wired or the joined
// Wi-Fi, tap to disconnect), then the strongest networks as flat rows (all of
// them behind "N more") -- tap an open or saved
// one to join, or expand a secured one for its password. Wi-Fi power and scan
// ride the head. All live off the daemon `network` topic via the Network
// singleton; the surface takes the keyboard only while a password or hidden-
// network field is open (`wantsKeyboard`; any other click leaves the window's
// focus alone), and dismisses on an outside click.
Item {
    id: root

    property real s: 1
    property bool open: false

    // a hidden network has no scanned AP, so it joins through the daemon intent
    // with 802-11-wireless.hidden set; these drive the inline join card.
    property bool hiddenOpen: false
    property string hiddenSsid: ""
    property string hiddenPassword: ""
    property int hiddenPendingId: -1
    property bool hiddenConnecting: false
    property bool hiddenError: false

    // password rows expanded; with the hidden-network card, what makes the
    // surface take the keyboard (CardManager reads wantsKeyboard).
    property int pwRowsOpen: 0
    readonly property bool wantsKeyboard: root.open && (root.hiddenOpen || root.pwRowsOpen > 0)

    function openHidden() {
        root.hiddenOpen = true;
        root.hiddenError = false;
        Qt.callLater(function() { if (hiddenSsidField.visible) hiddenSsidField.forceActiveFocus(); });
    }
    function cancelHidden() {
        root.hiddenOpen = false;
        root.hiddenConnecting = false;
        root.hiddenPendingId = -1;
        root.hiddenSsid = "";
        root.hiddenPassword = "";
        root.hiddenError = false;
        hiddenEye.show = false;
    }
    function submitHidden() {
        if (root.hiddenSsid === "" || root.hiddenConnecting)
            return;
        root.hiddenError = false;
        root.hiddenConnecting = true;
        root.hiddenPendingId = Network.connectWifi(root.hiddenSsid, root.hiddenPassword, "", true);
    }
    Connections {
        target: Network
        function onReplied(id, ok, error) {
            if (id !== root.hiddenPendingId)
                return;
            root.hiddenConnecting = false;
            root.hiddenPendingId = -1;
            if (ok)
                root.cancelHidden();
            else {
                root.hiddenError = true;
                root.hiddenPassword = "";
            }
        }
    }

    readonly property real pad: 11 * root.s
    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property color inkDim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)
    readonly property color line: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.14)

    readonly property bool wifiOn: Network.wifiRadio
    readonly property bool wiredUp: Network.kind === "ethernet"
    readonly property bool wifiUp: Network.kind === "wifi"
    readonly property string activeSsid: Network.activeSsid
    readonly property bool anyLink: root.wifiOn || root.wiredUp
    property bool scanning: false

    // available networks: one row per SSID+band, the strongest AP kept and the
    // list sorted by signal descending. keying by SSID alone dropped a dual-band
    // SSID's 5 GHz BSS; only the connected band row is filtered, so a link on
    // 2.4 GHz still leaves the same SSID's 5 GHz row pickable.
    readonly property var availableNets: {
        const aps = Network.accessPoints;
        let activeBand = "";
        for (let a = 0; a < aps.length; a++) {
            if (aps[a] && aps[a].active) {
                activeBand = aps[a].band || "";
                break;
            }
        }
        const best = ({});
        for (let i = 0; i < aps.length; i++) {
            const ap = aps[i];
            if (!ap || !ap.ssid)
                continue;
            if (ap.ssid === root.activeSsid && (activeBand === "" || (ap.band || "") === activeBand))
                continue;
            const key = ap.ssid + "\u0000" + (ap.band || "");
            const prev = best[key];
            if (!prev || (ap.strength || 0) > (prev.strength || 0))
                best[key] = ap;
        }
        const out = [];
        for (const k in best)
            out.push(best[k]);
        out.sort((x, y) => (y.strength || 0) - (x.strength || 0));
        return out;
    }

    // SSIDs seen on more than one band in the scan: a row shows its band chip
    // only for these, so single-band networks read exactly as before. Counted
    // over every scanned AP, not the rendered list, so the 5 GHz row of the
    // SSID you are already on (whose 2.4 GHz row is filtered out) stays chipped.
    readonly property var multiBandSsids: {
        const bands = ({});
        const aps = Network.accessPoints;
        for (let i = 0; i < aps.length; i++) {
            const ap = aps[i];
            if (!ap || !ap.ssid || !ap.band)
                continue;
            if (!bands[ap.ssid])
                bands[ap.ssid] = ({});
            bands[ap.ssid][ap.band] = true;
        }
        const multi = ({});
        for (const s in bands)
            if (Object.keys(bands[s]).length > 1)
                multi[s] = true;
        return multi;
    }
    onAvailableNetsChanged: if (root.availableNets.length > 0) root.scanning = false

    function secured(ap) { return ap && ap.security && ap.security !== "None"; }

    // signal as bars, the same steps as the rail icon.
    function bars(strength) {
        const s = strength || 0;
        return s > 75 ? "network_wifi" : s > 50 ? "network_wifi_3_bar"
            : s > 25 ? "network_wifi_2_bar" : s > 0 ? "network_wifi_1_bar" : "signal_wifi_0_bar";
    }

    // the list shows the strongest few until "N more" is tapped; it folds back
    // each time the card closes.
    readonly property int shortList: 6
    property bool showAll: false
    readonly property var shownNets: root.showAll ? root.availableNets
        : root.availableNets.slice(0, root.shortList)

    // misu: the list opens on the last scan's results and the rescan starts once
    // the card has finished revealing, so fresh results never re-sort the list
    // mid-slide.
    onOpenChanged: {
        if (root.open) {
            Network.refreshState();
            openScan.restart();
        } else {
            openScan.stop();
            root.showAll = false;
            root.scanning = false;
            scanClear.stop();
            if (!root.hiddenConnecting)
                root.cancelHidden();
        }
    }
    Timer { id: openScan; interval: Motion.dur(150) + 50; onTriggered: root.scan() }

    Timer { id: rescan; interval: 30000; repeat: true; running: root.open && root.wifiOn; onTriggered: Network.refresh() }
    Timer { id: scanClear; interval: 15000; onTriggered: root.scanning = false }

    function toggleWifi() { Network.setWifiEnabled(!root.wifiOn); }
    function scan() {
        if (!root.wifiOn)
            return;
        root.scanning = true;
        Network.refresh();
        scanClear.restart();
    }

    implicitWidth: 250 * root.s
    implicitHeight: content.implicitHeight + root.pad * 2

    PopoutCard { anchors.fill: parent }

    // an available-network row: signal + SSID (+ lock), tap to join or, when a
    // password is needed, expand an inline field.
    component ApRow: Column {
        id: apr
        required property var ap
        property bool expanded: false
        property bool connecting: false
        property bool errorShown: false
        property int pendingId: -1
        readonly property bool sec: root.secured(apr.ap)
        readonly property bool needsPw: apr.sec && !apr.ap.saved

        width: parent ? parent.width : 0
        spacing: 4 * root.s

        onExpandedChanged: root.pwRowsOpen += apr.expanded ? 1 : -1
        Component.onDestruction: if (apr.expanded) root.pwRowsOpen -= 1
        // closing the card folds an idle password row, so reopening it does
        // not take the keyboard.
        Connections {
            target: root
            function onOpenChanged() {
                if (!root.open && !apr.connecting) {
                    apr.expanded = false;
                    pwField.text = "";
                }
            }
        }

        function doConnect() {
            apr.errorShown = false;
            apr.connecting = true;
            apr.pendingId = Network.connectWifi(apr.ap.ssid, apr.needsPw ? pwField.text : "", apr.ap.bssid, false, apr.ap.profileUuid);
        }
        Connections {
            target: Network
            function onReplied(id, ok, error) {
                if (id !== apr.pendingId)
                    return;
                apr.connecting = false;
                apr.pendingId = -1;
                if (ok) {
                    apr.expanded = false;
                    pwField.text = "";
                } else {
                    apr.errorShown = true;
                    pwField.text = "";
                }
            }
        }

        Item {
            width: parent.width
            height: 26 * root.s
            Rectangle {
                anchors.fill: parent
                radius: 3 * root.s
                color: apTap.pressed ? Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.16)
                    : (apHover.hovered ? Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.08) : "transparent")
                Behavior on color { ColorAnimation { duration: Motion.fast } }
            }
            MaterialIcon {
                id: apIcon
                anchors.left: parent.left
                anchors.leftMargin: 7 * root.s
                anchors.verticalCenter: parent.verticalCenter
                size: 14 * root.s
                name: root.bars(apr.ap.strength)
                color: root.inkDim
            }
            Text {
                anchors.left: apIcon.right
                anchors.leftMargin: 7 * root.s
                anchors.right: apMeta.left
                anchors.rightMargin: 6 * root.s
                anchors.verticalCenter: parent.verticalCenter
                text: apr.ap.ssid
                color: root.ink
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h5 * root.s
                elide: Text.ElideRight
            }
            Row {
                id: apMeta
                anchors.right: parent.right
                anchors.rightMargin: 8 * root.s
                anchors.verticalCenter: parent.verticalCenter
                spacing: 5 * root.s
                Text {
                    visible: (apr.ap.band || "") !== "" && !!root.multiBandSsids[apr.ap.ssid]
                    anchors.verticalCenter: parent.verticalCenter
                    text: apr.ap.band || ""
                    color: root.inkDim
                    font.family: Theme.fontPrimary
                    font.pixelSize: Theme.h6 * root.s
                }
                MaterialIcon {
                    visible: apr.sec
                    anchors.verticalCenter: parent.verticalCenter
                    size: 11 * root.s
                    name: "lock"
                    color: root.inkDim
                }
                Text {
                    visible: apr.connecting
                    anchors.verticalCenter: parent.verticalCenter
                    text: "…"
                    color: root.inkDim
                    font.family: Theme.fontPrimary
                    font.pixelSize: Theme.h6 * root.s
                }
            }
            HoverHandler { id: apHover; cursorShape: Qt.PointingHandCursor }
            MouseArea {
                id: apTap
                anchors.fill: parent
                onClicked: {
                    if (apr.needsPw) {
                        apr.expanded = !apr.expanded;
                        if (apr.expanded)
                            pwField.forceActiveFocus();
                    } else {
                        apr.doConnect();
                    }
                }
            }
        }

        // inline password (secured, unsaved).
        Column {
            width: parent.width
            spacing: 4 * root.s
            visible: apr.needsPw && apr.expanded

            Rectangle {
                width: parent.width
                height: 26 * root.s
                radius: 3 * root.s
                color: "transparent"
                border.width: Theme.borderWidth
                border.color: root.line
                TextInput {
                    id: pwField
                    anchors.left: parent.left
                    anchors.right: eye.left
                    anchors.leftMargin: 8 * root.s
                    anchors.rightMargin: 4 * root.s
                    anchors.verticalCenter: parent.verticalCenter
                    color: root.ink
                    font.family: Theme.fontPrimary
                    font.pixelSize: Theme.h5 * root.s
                    echoMode: eye.show ? TextInput.Normal : TextInput.Password
                    clip: true
                    onAccepted: apr.doConnect()
                    Text {
                        anchors.fill: parent
                        verticalAlignment: Text.AlignVCenter
                        text: "Password"
                        color: root.inkDim
                        font: pwField.font
                        visible: pwField.text.length === 0 && !pwField.activeFocus
                    }
                }
                MaterialIcon {
                    id: eye
                    property bool show: false
                    anchors.right: parent.right
                    anchors.rightMargin: 7 * root.s
                    anchors.verticalCenter: parent.verticalCenter
                    size: 13 * root.s
                    name: eye.show ? "visibility_off" : "visibility"
                    color: root.inkDim
                    HoverHandler { cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: eye.show = !eye.show }
                }
            }
            Text {
                width: parent.width
                visible: apr.errorShown
                text: "Wrong password or connection failed"
                color: Theme.error
                wrapMode: Text.WordWrap
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h6 * root.s
            }
            PopoutAction {
                width: parent.width
                s: root.s
                enabled: !apr.connecting
                label: apr.connecting ? "Connecting…" : "Connect"
                onClicked: apr.doConnect()
            }
        }
    }

    Column {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: root.pad
        spacing: 8 * root.s

        // head: label, wifi toggle, scan.
        Item {
            width: parent.width
            height: 20 * root.s
            PopoutTitle {
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                text: "NETWORK"
                s: root.s
            }
            Row {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                spacing: 5 * root.s
                PopoutHeadButton {
                    visible: root.wifiOn
                    s: root.s
                    glyph: "restart_alt"
                    active: root.scanning
                    spin: root.scanning
                    onClicked: root.scan()
                }
                PopoutHeadButton {
                    s: root.s
                    glyph: "wifi"
                    active: root.wifiOn
                    onClicked: root.toggleWifi()
                }
            }
        }

        // OFF plate (wifi off and no wired link).
        Column {
            width: parent.width
            spacing: 7 * root.s
            visible: !root.wifiOn && !root.wiredUp
            topPadding: 6 * root.s
            bottomPadding: 4 * root.s
            MaterialIcon {
                anchors.horizontalCenter: parent.horizontalCenter
                size: 27 * root.s
                name: "wifi"
                color: root.inkDim
                opacity: 0.5
            }
            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: "Wi-Fi is off"
                color: root.ink
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h5 * root.s
                font.weight: Font.DemiBold
            }
            PopoutAction {
                anchors.horizontalCenter: parent.horizontalCenter
                s: root.s
                label: "Turn on"
                onClicked: root.toggleWifi()
            }
        }

        // the current link, styled like the rows below: wired, the joined
        // Wi-Fi (tap to disconnect), or a join in progress.
        Item {
            id: current
            readonly property bool connecting: !root.wiredUp && !root.wifiUp
                && Network.wifiConnectivity === "Connecting"
            width: parent.width
            height: 26 * root.s
            visible: root.wiredUp || root.wifiUp || current.connecting
            Rectangle {
                anchors.fill: parent
                radius: 3 * root.s
                color: curTap.pressed ? Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.16)
                    : (curHover.hovered ? Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.08) : "transparent")
                Behavior on color { ColorAnimation { duration: Motion.fast } }
            }
            MaterialIcon {
                id: curIcon
                anchors.left: parent.left
                anchors.leftMargin: 7 * root.s
                anchors.verticalCenter: parent.verticalCenter
                size: 14 * root.s
                name: root.wiredUp ? "lan" : root.bars(Network.wifi.strength)
                color: Theme.primary
            }
            Text {
                anchors.left: curIcon.right
                anchors.leftMargin: 7 * root.s
                anchors.right: curState.left
                anchors.rightMargin: 6 * root.s
                anchors.verticalCenter: parent.verticalCenter
                text: root.wiredUp ? "Wired" : root.wifiUp ? root.activeSsid : "Joining…"
                color: root.ink
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h5 * root.s
                font.weight: Font.DemiBold
                elide: Text.ElideRight
            }
            Text {
                id: curState
                anchors.right: parent.right
                anchors.rightMargin: 8 * root.s
                anchors.verticalCenter: parent.verticalCenter
                text: root.wifiUp && curHover.hovered ? "Disconnect"
                    : current.connecting ? ""
                    : Network.vpnActive ? "VPN · " + Network.vpnName
                    : "Connected"
                color: root.wifiUp && curHover.hovered ? Theme.error : root.inkDim
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h6 * root.s
            }
            HoverHandler { id: curHover; cursorShape: root.wifiUp ? Qt.PointingHandCursor : Qt.ArrowCursor }
            MouseArea {
                id: curTap
                anchors.fill: parent
                enabled: root.wifiUp
                onClicked: Network.disconnectWifi()
            }
        }
        Rectangle {
            width: parent.width
            height: 1
            color: root.line
            visible: current.visible && root.wifiOn
        }

        // available networks, the strongest first.
        Column {
            width: parent.width
            spacing: 2 * root.s
            visible: root.wifiOn
            Repeater {
                model: root.shownNets
                delegate: ApRow {
                    required property var modelData
                    ap: modelData
                }
            }
        }
        Text {
            width: parent.width
            visible: root.wifiOn && root.availableNets.length > root.shortList
            leftPadding: 7 * root.s
            text: root.showAll ? "Fewer"
                : "%1 more".arg(root.availableNets.length - root.shortList)
            color: moreHover.hovered ? root.ink : root.inkDim
            font.family: Theme.fontPrimary
            font.pixelSize: Theme.h6 * root.s
            Behavior on color { ColorAnimation { duration: 120 } }
            HoverHandler { id: moreHover; cursorShape: Qt.PointingHandCursor }
            TapHandler { onTapped: root.showAll = !root.showAll }
        }
        Text {
            width: parent.width
            visible: root.wifiOn && root.availableNets.length === 0
            horizontalAlignment: Text.AlignHCenter
            text: root.scanning ? "Scanning…" : "No networks found"
            color: root.inkDim
            font.family: Theme.fontPrimary
            font.pixelSize: Theme.h6 * root.s
            topPadding: 2 * root.s
        }
        Rectangle {
            width: parent.width
            height: 1
            color: root.line
            visible: root.wifiOn
        }
        Text {
            width: parent.width
            visible: root.wifiOn && !root.hiddenOpen
            leftPadding: 7 * root.s
            text: "Hidden network…"
            color: hiddenLinkHover.hovered ? root.ink : root.inkDim
            font.family: Theme.fontPrimary
            font.pixelSize: Theme.h6 * root.s
            Behavior on color { ColorAnimation { duration: 120 } }
            HoverHandler { id: hiddenLinkHover; cursorShape: Qt.PointingHandCursor }
            TapHandler { onTapped: root.openHidden() }
        }
        Column {
            width: parent.width
            spacing: 4 * root.s
            visible: root.wifiOn && root.hiddenOpen

            Rectangle {
                width: parent.width
                height: 26 * root.s
                radius: 3 * root.s
                color: "transparent"
                border.width: Theme.borderWidth
                border.color: root.line
                TextInput {
                    id: hiddenSsidField
                    KeyNavigation.tab: hiddenPwField
                    KeyNavigation.backtab: hiddenPwField
                    anchors.fill: parent
                    anchors.leftMargin: 8 * root.s
                    anchors.rightMargin: 8 * root.s
                    verticalAlignment: TextInput.AlignVCenter
                    color: root.ink
                    font.family: Theme.fontPrimary
                    font.pixelSize: Theme.h5 * root.s
                    clip: true
                    text: root.hiddenSsid
                    onTextChanged: { root.hiddenSsid = text; root.hiddenError = false; }
                    onAccepted: hiddenPwField.forceActiveFocus()
                    Keys.onPressed: function(event) { if (event.key === Qt.Key_Escape) { root.cancelHidden(); event.accepted = true; } }
                    Text {
                        anchors.fill: parent
                        verticalAlignment: Text.AlignVCenter
                        text: "Network name (SSID)"
                        color: root.inkDim
                        font: hiddenSsidField.font
                        visible: hiddenSsidField.text.length === 0 && !hiddenSsidField.activeFocus
                    }
                }
            }
            Rectangle {
                width: parent.width
                height: 26 * root.s
                radius: 3 * root.s
                color: "transparent"
                border.width: Theme.borderWidth
                border.color: root.line
                TextInput {
                    id: hiddenPwField
                    KeyNavigation.tab: hiddenSsidField
                    KeyNavigation.backtab: hiddenSsidField
                    anchors.left: parent.left
                    anchors.right: hiddenEye.left
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    anchors.leftMargin: 8 * root.s
                    anchors.rightMargin: 4 * root.s
                    verticalAlignment: TextInput.AlignVCenter
                    color: root.ink
                    font.family: Theme.fontPrimary
                    font.pixelSize: Theme.h5 * root.s
                    echoMode: hiddenEye.show ? TextInput.Normal : TextInput.Password
                    clip: true
                    text: root.hiddenPassword
                    onTextChanged: { root.hiddenPassword = text; root.hiddenError = false; }
                    onAccepted: root.submitHidden()
                    Keys.onPressed: function(event) { if (event.key === Qt.Key_Escape) { root.cancelHidden(); event.accepted = true; } }
                    Text {
                        anchors.fill: parent
                        verticalAlignment: Text.AlignVCenter
                        text: "Password (leave empty if open)"
                        color: root.inkDim
                        font: parent.font
                        visible: parent.text.length === 0 && !parent.activeFocus
                    }
                }
                MaterialIcon {
                    id: hiddenEye
                    property bool show: false
                    anchors.right: parent.right
                    anchors.rightMargin: 7 * root.s
                    anchors.verticalCenter: parent.verticalCenter
                    size: 13 * root.s
                    name: hiddenEye.show ? "visibility_off" : "visibility"
                    color: root.inkDim
                    HoverHandler { cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: hiddenEye.show = !hiddenEye.show }
                }
            }
            Text {
                width: parent.width
                visible: root.hiddenError
                text: "Wrong password or connection failed"
                color: Theme.error
                wrapMode: Text.WordWrap
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h6 * root.s
            }
            PopoutAction {
                width: parent.width
                s: root.s
                enabled: root.hiddenSsid !== "" && !root.hiddenConnecting
                label: root.hiddenConnecting ? "Connecting…" : "Connect"
                onClicked: root.submitHidden()
            }
            PopoutAction {
                width: parent.width
                s: root.s
                label: "Cancel"
                onClicked: root.cancelHidden()
            }
        }
    }
}
