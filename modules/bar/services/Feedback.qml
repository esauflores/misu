pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import "../plugins/audio" as AudioPlugin

// Shared state for the short-lived volume, microphone, and brightness OSD.
Singleton {
    id: root

    property string kind: "volume"
    property real level: 0
    property bool muted: false
    property bool visible: false
    property bool armed: false

    readonly property var sink: AudioPlugin.Audio.sink
    readonly property var source: AudioPlugin.Audio.source
    readonly property var outputAudio: root.sink ? root.sink.audio : null
    readonly property var microphoneAudio: root.source ? root.source.audio : null

    function show(nextKind, nextLevel, nextMuted, force) {
        if (!root.armed && force !== true) return;
        root.kind = nextKind;
        root.level = Math.max(0, Math.min(1, nextLevel));
        root.muted = nextMuted;
        root.visible = true;
        hideTimer.restart();
    }

    function readBrightness() {
        if (!brightness.running) brightness.running = true;
    }

    Timer { interval: 1000; running: true; onTriggered: root.armed = true }

    Connections {
        target: root.outputAudio
        function onVolumesChanged() { root.show("volume", root.outputAudio.volume, root.outputAudio.muted); }
        function onMutedChanged() { root.show("volume", root.outputAudio.volume, root.outputAudio.muted); }
    }
    Connections {
        target: root.microphoneAudio
        function onVolumesChanged() { root.show("microphone", root.microphoneAudio.volume, root.microphoneAudio.muted); }
        function onMutedChanged() { root.show("microphone", root.microphoneAudio.volume, root.microphoneAudio.muted); }
    }

    Process {
        id: brightness
        command: ["brightnessctl", "-m"]
        stdout: StdioCollector {
            onStreamFinished: {
                const match = text.match(/,(\d+)%/);
                if (match) root.show("brightness", Number(match[1]) / 100, false, true);
            }
        }
    }

    Timer {
        id: hideTimer
        interval: 1400
        onTriggered: root.visible = false
    }
}
