pragma ComponentBehavior: Bound

import QtQuick
import Quickshell.Widgets
import qs.services
import "../.."

// One per-app mixer row, shared by the audio menu and the audio popout: the app
// icon doubles as a mute toggle, then the app's name and its own live VU
// fader on the same line. A capture row (an app recording the mic) carries a small vermilion mic
// tag so a streamer can see who is on the input at a glance. Reads live from the
// stream node via Audio.
Row {
    id: root

    property real s: 1
    property var stream: null
    property bool open: false
    property bool capture: false

    readonly property var appAudio: root.stream ? root.stream.audio : null
    readonly property bool appMuted: root.appAudio ? root.appAudio.muted : false

    spacing: 8 * root.s

    // the app icon doubles as its mute toggle.
    Item {
        width: 20 * root.s
        height: 20 * root.s
        anchors.verticalCenter: parent.verticalCenter

        IconImage {
            anchors.fill: parent
            source: Audio.streamIcon(root.stream)
            opacity: root.appMuted ? 0.4 : 1
        }
        MaterialIcon {
            visible: root.capture
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            size: 10 * root.s
            name: "mic"
            color: Theme.primary
        }
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: if (root.appAudio) root.appAudio.muted = !root.appAudio.muted
        }
    }

    Text {
        width: 52 * root.s
        anchors.verticalCenter: parent.verticalCenter
        text: Audio.streamName(root.stream)
        elide: Text.ElideRight
        color: Theme.inkOn(Theme.effectiveSurface, root.appMuted ? Theme.surfaceVariantText : Theme.surfaceText, 3.0)
        font.family: Theme.fontPrimary
        font.pixelSize: Theme.h5 * root.s
    }
    HFader {
        width: root.width - (20 + 52 + 8 * 2) * root.s
        anchors.verticalCenter: parent.verticalCenter
        s: root.s
        showIcon: false
        lit: root.open
        value: root.appAudio ? root.appAudio.volume : 0
        muted: root.appMuted
        valueLabel: root.appAudio ? (root.appMuted ? "off" : Math.round(root.appAudio.volume * 100) + "%") : ""
        onMoved: v => { if (root.appAudio) root.appAudio.volume = v; }
    }
}
