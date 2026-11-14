pragma ComponentBehavior: Bound

import QtQuick
import qs.services
import "../../popouts"
import "../.."

// Audio popout: the mixer card grown off the speaker/mic rail widget (shared
// PopoutCard, so it opens and melts like the music card). Output and input each
// lead with the device (a switcher that expands; a Bluetooth sink shows its
// codec there and a Hi-Fi <-> Headset flip by the section's name), then its
// live VU fader -- tap the glyph to mute, drag or wheel to set. Per-app rows
// give every playback stream its own fader on one line, and the recording
// section does the same for apps capturing the mic. All live off Pipewire via
// Audio.
Item {
    id: root

    property real s: 1
    property bool open: false

    readonly property real pad: 11 * root.s
    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property color inkDim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)
    readonly property color line: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.14)

    readonly property var sink: Audio.sink
    readonly property var source: Audio.source
    readonly property bool haveSink: !!(root.sink && root.sink.audio)
    readonly property bool haveSource: !!(root.source && root.source.audio)

    // device switchers collapse again whenever the card closes.
    property bool outDevicesOpen: false
    property bool inDevicesOpen: false
    // the device lists leave out unplugged ports (Audio.plugged), looked up
    // again on each open: a screen or headphones may have come since.
    onOpenChanged: {
        if (root.open)
            Audio.refreshPorts();
        else {
            root.outDevicesOpen = false;
            root.inDevicesOpen = false;
        }
    }

    // faders and app rows sit under their device's name
    readonly property real indent: 8 * root.s

    component SectionLabel: Text {
        leftPadding: root.indent
        color: root.inkDim
        font.family: Theme.fontPrimary
        font.pixelSize: Theme.h6 * root.s
    }

    implicitWidth: 272 * root.s
    implicitHeight: content.implicitHeight + root.pad * 2

    PopoutCard { anchors.fill: parent }

    Column {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: root.pad
        spacing: 8 * root.s

        // head.
        PopoutTitle {
            text: "AUDIO"
            s: root.s
        }

        // ── output ───────────────────────────────────────────────────────────
        Column {
            width: parent.width
            spacing: 4 * root.s

            Item {
                width: parent.width
                height: outLabel.implicitHeight
                SectionLabel { id: outLabel; text: "Output" }
                // a Bluetooth sink: Hi-Fi (A2DP) <-> Headset (with the mic)
                Text {
                    visible: Audio.sinkIsBluez && Audio.profileLabel().length > 0
                    anchors.right: parent.right
                    anchors.rightMargin: 8 * root.s
                    anchors.verticalCenter: parent.verticalCenter
                    text: Audio.profileLabel() + "  ⇄"
                    color: profHover.hovered ? root.ink : root.inkDim
                    font.family: Theme.fontPrimary
                    font.pixelSize: Theme.h6 * root.s
                    HoverHandler { id: profHover; cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: Audio.toggleProfile() }
                }
            }
            DevicePicker {
                width: parent.width
                s: root.s
                current: root.sink
                devices: Audio.outputs.filter(n => Audio.alive(n) && (Audio.plugged(n) || n === root.sink))
                listOpen: root.outDevicesOpen
                fallbackIcon: "speaker"
                emptyLabel: "No output device"
                detail: Audio.btCodec
                onToggled: root.outDevicesOpen = !root.outDevicesOpen
                onPicked: node => Audio.setOutput(node)
            }
            HFader {
                width: parent.width - root.indent
                x: root.indent
                s: root.s
                icon: "volume_up"
                lit: root.open
                value: root.haveSink ? root.sink.audio.volume : 0
                muted: root.haveSink ? root.sink.audio.muted : false
                valueLabel: !root.haveSink ? "" : (root.sink.audio.muted ? "off" : Math.round(root.sink.audio.volume * 100) + "%")
                onMoved: v => { if (root.haveSink) root.sink.audio.volume = v; }
                onIconTapped: { if (root.haveSink) root.sink.audio.muted = !root.sink.audio.muted; }
            }
        }

        // ── input ────────────────────────────────────────────────────────────
        Column {
            width: parent.width
            spacing: 4 * root.s

            SectionLabel { text: "Input" }
            DevicePicker {
                width: parent.width
                s: root.s
                current: root.source
                devices: Audio.inputs.filter(n => Audio.alive(n) && (Audio.plugged(n) || n === root.source))
                listOpen: root.inDevicesOpen
                fallbackIcon: "mic"
                emptyLabel: "No input device"
                onToggled: root.inDevicesOpen = !root.inDevicesOpen
                onPicked: node => Audio.setInput(node)
            }
            HFader {
                width: parent.width - root.indent
                x: root.indent
                s: root.s
                icon: "mic"
                lit: root.open
                value: root.haveSource ? root.source.audio.volume : 0
                muted: root.haveSource ? root.source.audio.muted : false
                valueLabel: !root.haveSource ? "" : (root.source.audio.muted ? "off" : Math.round(root.source.audio.volume * 100) + "%")
                onMoved: v => { if (root.haveSource) root.source.audio.volume = v; }
                onIconTapped: { if (root.haveSource) root.source.audio.muted = !root.source.audio.muted; }
            }
        }

        Rectangle {
            width: parent.width
            height: 1
            color: root.line
        }

        // ── per-app playback ──────────────────────────────────────────────────
        Column {
            id: appsCol
            width: parent.width
            spacing: 6 * root.s

            SectionLabel { text: "Apps" }
            Repeater {
                model: root.open ? Audio.streams.filter(Audio.alive) : []
                delegate: AudioAppRow {
                    required property var modelData
                    width: appsCol.width - root.indent
                    x: root.indent
                    s: root.s
                    open: root.open
                    stream: modelData
                }
            }
            Text {
                visible: Audio.streams.length === 0
                width: parent.width
                leftPadding: root.indent
                text: "Nothing playing"
                color: root.inkDim
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h5 * root.s
            }
        }

        // ── per-app capture (recording) ───────────────────────────────────────
        Column {
            id: capCol
            width: parent.width
            spacing: 6 * root.s
            visible: Audio.captureStreams.length > 0

            SectionLabel { text: "Recording" }
            Repeater {
                model: root.open ? Audio.captureStreams.filter(Audio.alive) : []
                delegate: AudioAppRow {
                    required property var modelData
                    width: capCol.width - root.indent
                    x: root.indent
                    s: root.s
                    open: root.open
                    capture: true
                    stream: modelData
                }
            }
        }
    }
}
