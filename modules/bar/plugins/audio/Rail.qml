pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import qs.services
import "../../rail"

// Audio rail buttons: the speaker (audio-output) opens the audio card, the mic
// (audio-input) toggles mute; both take a hover wheel for volume. Icons follow
// the reference buckets: muted wins, then >66 high, >33 medium, >0 low.
Item {
    id: root

    property string edge: "right"
    property real scale: 1
    property string widgetId: "audio-output"
    signal menuRequested(string id, rect ownerRect)

    readonly property bool output: root.widgetId === "audio-output"
    readonly property var node: root.output ? Audio.sink : Audio.source
    readonly property int pct: root.node && root.node.audio ? Math.round(root.node.audio.volume * 100) : 0
    readonly property bool muted: root.node && root.node.audio ? root.node.audio.muted : false

    implicitWidth: btn.implicitWidth
    implicitHeight: btn.implicitHeight

    function glyph() {
        if (!root.output)
            return root.muted || root.pct === 0 ? "mic_off" : "mic";
        if (root.muted || root.pct === 0)
            return "volume_off";
        return root.pct > 50 ? "volume_up" : root.pct > 15 ? "volume_down" : "volume_mute";
    }

    // Step +-0.05 clamped to [0,1], on the default sink or source.
    function scrollBy(steps) {
        if (root.node && root.node.audio) {
            const v = root.node.audio.volume + (steps > 0 ? 0.05 : -0.05);
            root.node.audio.volume = Math.max(0, Math.min(1, v));
        }
    }

    RailButton {
        id: btn
        anchors.centerIn: parent
        edge: root.edge
        scale: root.scale
        icon: root.glyph()
        scrollable: true
        onScrolled: steps => root.scrollBy(steps)
        onClicked: {
            if (root.output)
                root.menuRequested("audio", Qt.rect(btn.x, btn.y, btn.width, btn.height));
            else if (root.node && root.node.audio)
                root.node.audio.muted = !root.node.audio.muted;
        }
    }
}
