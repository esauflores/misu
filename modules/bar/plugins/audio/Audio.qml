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

    function nodeLabel(n) {
        if (!n) return "";
        return n.description || n.nickname || n.name || "";
    }
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
    function streamIcon(n) {
        const icon = root.prop(n, "application.icon-name") || root.prop(n, "application.process.binary")
            || root.streamName(n).toLowerCase();
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
