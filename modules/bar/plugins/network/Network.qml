pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io

// NetworkManager over nmcli. One snapshot command fills every property;
// `nmcli monitor` re-runs it (debounced) on any change, so the bar stays live
// without polling.
Singleton {
    id: root

    property string kind: ""            // "wifi" | "ethernet" | ""
    property real level: 0              // active Wi-Fi signal 0..1
    readonly property var wifi: ({ strength: Math.round(root.level * 100) })
    property string activeSsid: ""
    property var accessPoints: []       // { ssid, bssid, strength, band, security, saved, active }
    property bool wifiPresent: false
    property bool wifiRadio: false
    property string wifiConnectivity: "" // "Connecting" while a Wi-Fi link comes up
    property bool vpnActive: false
    property string vpnName: ""

    // nmcli -t escapes ':' and '\' inside fields.
    function splitTerse(line) {
        const out = [];
        let cur = "";
        for (let i = 0; i < line.length; ++i) {
            const c = line[i];
            if (c === "\\" && i + 1 < line.length) { cur += line[++i]; continue; }
            if (c === ":") { out.push(cur); cur = ""; continue; }
            cur += c;
        }
        out.push(cur);
        return out;
    }
    function bandOf(freq) {
        const f = parseInt(freq, 10) || 0;
        return f >= 5925 ? "6 GHz" : f >= 4900 ? "5 GHz" : f > 0 ? "2.4 GHz" : "";
    }

    function parse(text) {
        let section = "";
        let kind = "", connecting = false, wifiDev = false, radio = false;
        const aps = [], saved = {};
        let vpn = "";
        for (const line of text.split("\n")) {
            if (line.indexOf("##") === 0) { section = line.substring(2); continue; }
            if (line.length === 0) continue;
            const f = root.splitTerse(line);
            if (section === "dev") {
                const [type, state] = f;
                if (type === "wifi") wifiDev = true;
                if (state === "connected" && (type === "ethernet" || type === "wifi") && kind !== "ethernet")
                    kind = type;
                if (type === "wifi" && state.indexOf("connecting") === 0) connecting = true;
            } else if (section === "radio") {
                radio = f[0] === "enabled";
            } else if (section === "wifi") {
                const [inUse, ssid, bssid, signal, freq, security] = f;
                if (!ssid) continue;
                aps.push({
                    ssid: ssid, bssid: bssid, strength: parseInt(signal, 10) || 0,
                    band: root.bandOf(freq), security: security && security !== "--" ? security : "None",
                    active: inUse === "*", saved: false
                });
            } else if (section === "con") {
                const [name, type, active] = f;
                if (type === "802-11-wireless") saved[name] = true;
                if ((type === "vpn" || type === "wireguard") && active === "yes") vpn = name;
            }
        }
        let activeSsid = "", level = 0;
        for (const ap of aps) {
            ap.saved = saved[ap.ssid] === true;
            if (ap.active) { activeSsid = ap.ssid; level = ap.strength / 100; }
        }
        root.kind = kind;
        root.wifiPresent = wifiDev;
        root.wifiRadio = radio;
        root.wifiConnectivity = connecting ? "Connecting" : (kind === "wifi" ? "Full" : "");
        root.accessPoints = aps;
        root.activeSsid = kind === "wifi" ? activeSsid : "";
        root.level = kind === "wifi" ? level : 0;
        root.vpnActive = vpn !== "";
        root.vpnName = vpn;
    }

    Process {
        id: snapshot
        property bool rescan: false
        command: ["sh", "-c",
            "echo '##dev'; nmcli -t -f TYPE,STATE device;"
            + " echo '##radio'; nmcli -t -f WIFI radio;"
            + " echo '##wifi'; nmcli -t -f IN-USE,SSID,BSSID,SIGNAL,FREQ,SECURITY device wifi list --rescan " + (snapshot.rescan ? "yes" : "no") + ";"
            + " echo '##con'; nmcli -t -f NAME,TYPE,ACTIVE connection show"]
        stdout: StdioCollector { onStreamFinished: root.parse(text) }
        onExited: snapshot.rescan = false
    }
    function refresh() {
        snapshot.rescan = true;
        debounce.restart();
    }
    Timer {
        id: debounce
        interval: 250
        onTriggered: if (!snapshot.running) snapshot.running = true; else debounce.restart()
    }

    Process {
        command: ["nmcli", "monitor"]
        running: true
        stdout: SplitParser { onRead: debounce.restart() }
        onExited: monitorRestart.start()
    }
    Timer { id: monitorRestart; interval: 5000; onTriggered: debounce.restart() }
    Component.onCompleted: debounce.restart()

    // owners that want VPN state kept fresh (nmcli monitor already reports
    // VPN changes; this just forces a snapshot when one starts watching)
    function setVpnPolling(owner, on) { if (on) debounce.restart(); }

    function run(args) {
        Quickshell.execDetached(args);
        debounce.restart();
    }
    // Each connect is its own request: it returns an id, and replied(id, ok,
    // error) fires when nmcli finishes, so the row that asked can show the
    // outcome.
    signal replied(int id, bool ok, string error)
    property int _nextId: 1
    function connectWifi(ssid, password, bssid, hidden) {
        const args = ["nmcli", "device", "wifi", "connect", ssid];
        if (password && password.length > 0) args.push("password", password);
        if (bssid && bssid.length > 0) args.push("bssid", bssid);
        if (hidden) args.push("hidden", "yes");
        root.wifiConnectivity = "Connecting";
        const id = root._nextId++;
        requestComp.createObject(root, { requestId: id, command: args, running: true });
        return id;
    }
    Component {
        id: requestComp
        Process {
            id: req
            property int requestId: 0
            stderr: StdioCollector { id: err }
            onExited: code => {
                root.replied(req.requestId, code === 0, code === 0 ? "" : err.text.trim());
                debounce.restart();
                req.destroy();
            }
        }
    }
    function disconnectWifi() {
        root.run(["sh", "-c", "nmcli -t -f DEVICE,TYPE device | awk -F: '$2==\"wifi\"{print $1}' | xargs -r -n1 nmcli device disconnect"]);
    }
    function setWifiEnabled(on) { root.run(["nmcli", "radio", "wifi", on ? "on" : "off"]); }

}
