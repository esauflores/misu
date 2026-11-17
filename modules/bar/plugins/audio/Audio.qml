pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.Pipewire
import "../bluetooth"

// PipeWire devices and app streams. sink/source are the default nodes (bound
// so their .audio volume and mute are live); outputs/inputs are the hardware
// nodes, streams/captureStreams the application playback and recording nodes.
Singleton {
    id: root

    readonly property var sink: Pipewire.defaultAudioSink
    readonly property var source: Pipewire.defaultAudioSource

    readonly property var nodes: Pipewire.nodes ? Pipewire.nodes.values : []
    readonly property var outputs: root.nodes.filter(n => n && n.audio && n.isSink && !n.isStream)
    readonly property var inputs: root.nodes.filter(n => n && n.audio && !n.isSink && !n.isStream)
    readonly property var streams: root.nodes.filter(n => n && n.audio && n.isStream && n.isSink)
    // Quickshell's peak monitor creates its own capture stream; listing that
    // stream here makes every row create another peak monitor recursively.
    readonly property var captureStreams: root.nodes.filter(n => n && n.audio && n.isStream && !n.isSink
        && root.streamName(n) !== "Quickshell Peak Detect")

    PwObjectTracker {
        objects: [root.sink, root.source].concat(root.outputs, root.inputs, root.streams, root.captureStreams)
    }

    function alive(n) { return !!n && n.ready !== false; }

    function prop(n, key) { return n && n.properties ? (n.properties[key] || "") : ""; }

    function isBluez(n) {
        return root.prop(n, "device.api") === "bluez5" || root.prop(n, "api.bluez5.address") !== ""
            || (n && n.name ? n.name.indexOf("bluez") >= 0 : false);
    }
    readonly property bool sinkIsBluez: root.isBluez(root.sink)

    function btMac(n) {
        const a = root.prop(n, "api.bluez5.address");
        if (a) return a;
        const m = n && n.name ? n.name.match(/([0-9A-Fa-f]{2}[_:]){5}[0-9A-Fa-f]{2}/) : null;
        return m ? m[0].replace(/_/g, ":") : "";
    }
    function btDeviceFor(n) {
        const mac = root.btMac(n).toUpperCase();
        return mac ? BtLink.deviceFor(mac) : null;
    }
    readonly property string btCodec: root.sinkIsBluez ? root.prop(root.sink, "api.bluez5.codec").toUpperCase() : ""

    // A sound card's outputs all start with the card's long name ("500 Series
    // Chipset Family … (HD Audio) Speaker"), so they read as one; its port's
    // own name ("Speaker", "HDMI / DisplayPort 1") tells them apart.
    function nodeLabel(n) {
        if (!n) return "";
        const port = root.prop(n, "device.profile.description").replace(/ Output$/, "");
        return port || n.description || n.nickname || n.name || "";
    }
    // A sound card's ports with nothing plugged in (an HDMI port without a
    // screen, an empty headphone jack) still have nodes; only the card's
    // routes say so ("available": "no"), and Quickshell doesn't show those,
    // so pw-dump does. Keys are "<device id>:<card.profile.device>".
    property var unplugged: ({})
    function plugged(n) {
        const p = n && n.properties ? n.properties : {};
        return !root.unplugged[p["device.id"] + ":" + p["card.profile.device"]];
    }
    function refreshPorts() { portsProc.running = true; }
    Process {
        id: portsProc
        command: ["sh", "-c", "pw-dump | jq -c '[.[] | .id as $d | .info.params.EnumRoute? // [] | .[] | select(.available == \"no\") | .devices[] | \"\\($d):\\(.)\"]'"]
        stdout: StdioCollector {
            onStreamFinished: {
                const map = {};
                try {
                    for (const k of JSON.parse(this.text))
                        map[k] = true;
                } catch (e) {
                    return;
                }
                root.unplugged = map;
            }
        }
    }
    onOutputsChanged: root.refreshPorts()
    onInputsChanged: root.refreshPorts()

    function nodeIcon(n) {
        if (!n) return "speaker";
        if (root.isBluez(n)) return n.isSink ? "headphones" : "mic";
        const ff = (root.prop(n, "device.form-factor") + " " + root.prop(n, "device.icon-name")).toLowerCase();
        if (ff.indexOf("head") >= 0) return n.isSink ? "headphones" : "mic";
        return n.isSink ? "speaker" : "mic";
    }

    function streamName(n) {
        return root.prop(n, "application.name") || root.nodeLabel(n);
    }
    // The stream's own icon name, else its app's desktop entry (for streams
    // that name no icon and run as a binary named unlike the icon), else
    // the binary's name. Quickshell scans the desktop entries only once
    // `applications` is read, and heuristicLookup alone doesn't trigger it, so
    // read it here: that loads them and re-runs the binding when they land.
    function streamIcon(n) {
        const bin = root.prop(n, "application.process.binary");
        let icon = root.prop(n, "application.icon-name");
        if (!icon) {
            const entry = DesktopEntries.applications.values.length > 0
                ? DesktopEntries.heuristicLookup(root.prop(n, "application.name") || bin.replace(/-bin$/, "")) : null;
            icon = entry && entry.icon ? entry.icon : (bin || root.streamName(n).toLowerCase());
        }
        return Quickshell.iconPath(icon, "application-x-executable");
    }

    function setOutput(n) { if (n) Pipewire.preferredDefaultAudioSink = n; }
    function setInput(n) { if (n) Pipewire.preferredDefaultAudioSource = n; }

    // Bluetooth sink profile: A2DP (high quality) vs HSP/HFP (headset mic).
    readonly property string _profile: root.prop(root.sink, "api.bluez5.profile")
    function profileLabel() {
        if (!root.sinkIsBluez) return "";
        const p = root._profile;
        return p.indexOf("a2dp") >= 0 ? "Hi-Fi" : p.length > 0 ? "Headset" : "";
    }
    function toggleProfile() {
        if (!root.sinkIsBluez) return;
        const card = root.prop(root.sink, "device.name") || ("bluez_card." + root.btMac(root.sink).replace(/:/g, "_"));
        const next = root._profile.indexOf("a2dp") >= 0 ? "headset-head-unit" : "a2dp-sink";
        Quickshell.execDetached(["pactl", "set-card-profile", card, next]);
    }

    function stepVolume(delta) {
        if (!root.sink || !root.sink.audio) return;
        root.sink.audio.volume = Math.max(0, Math.min(1, root.sink.audio.volume + delta));
    }
}
