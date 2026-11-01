pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Widgets
import qs.services
import "../../popouts"
import "../.."

// Now-playing card, after the launcher's NowPlaying: the cover art on
// the left; the 力 NOW PLAYING eyebrow, title and artist beside it; the wavy seekbar (a
// sine over the played part while playing, flat when paused) with elapsed and
// total time either side of the transport. It follows the active MPRIS player
// (MprisPlayer), and the eyebrow's open button raises that player's app. The position poll and
// the wave only run while the card is open and playing.
Item {
    id: root

    property real s: 1
    property bool open: false

    // seconds -> "m:ss" (or "h:mm:ss")
    function stamp(sec) {
        const t = Math.max(0, Math.floor(sec || 0));
        const h = Math.floor(t / 3600), m = Math.floor(t % 3600 / 60), ss = t % 60;
        const pad = n => n < 10 ? "0" + n : String(n);
        return h > 0 ? h + ":" + pad(m) + ":" + pad(ss) : m + ":" + pad(ss);
    }

    MprisPlayer { id: mpris }

    readonly property var player: mpris.player
    readonly property bool hasPlayer: root.player !== null
    readonly property bool playing: mpris.playing
    readonly property string title: root.hasPlayer && root.player.trackTitle ? root.player.trackTitle : "Nothing playing"
    readonly property string artist: {
        if (!root.hasPlayer) return "";
        const artists = Array.from(root.player.trackArtists || []).filter(Boolean);
        if (artists.length === 0) return root.player.trackArtist || "";
        if (artists.length === 1) return artists[0];
        return artists.slice(0, -1).join(", ") + " & " + artists[artists.length - 1];
    }
    readonly property string artUrl: root.hasPlayer && root.player.trackArtUrl ? root.player.trackArtUrl : ""
    readonly property real positionSec: root.hasPlayer ? root.player.position : 0
    readonly property real lengthSec: root.hasPlayer && root.player.length > 0 ? root.player.length : 0
    readonly property real frac: root.lengthSec > 0 ? Math.max(0, Math.min(1, root.positionSec / root.lengthSec)) : 0
    readonly property bool canSeek: root.hasPlayer && root.player.canSeek && root.lengthSec > 0
    property bool scrubbing: false
    property real scrubFrac: 0

    readonly property real pad: 12 * root.s
    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property color inkDim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)
    readonly property color line: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.18)
    readonly property color accent: Theme.vermLit

    implicitWidth: 340 * root.s
    implicitHeight: 132 * root.s

    // MPRIS never pushes position, so re-read it on a tick while playing.
    Timer {
        interval: 500
        running: root.open && root.playing
        repeat: true
        onTriggered: root.player.positionChanged()
    }

    PopoutCard { anchors.fill: parent }

    ClippingRectangle {
        id: cover
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.margins: root.pad
        width: height
        radius: Theme.radiusWidget
        color: root.line

        Image {
            anchors.fill: parent
            source: root.artUrl
            sourceSize: Qt.size(Math.ceil(width * 2), Math.ceil(height * 2))
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            cache: true
            visible: status === Image.Ready
        }
        // music-note stand-in while there is no art.
        MaterialIcon {
            anchors.centerIn: parent
            visible: root.artUrl === ""
            name: "music_note"
            color: root.inkDim
            size: 32 * root.s
        }
    }

    Column {
        id: meta
        anchors.left: cover.right
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.leftMargin: 14 * root.s
        anchors.rightMargin: root.pad
        anchors.topMargin: root.pad + 2 * root.s
        spacing: 4 * root.s

        Item {
            width: parent.width
            height: eyebrow.implicitHeight

            Row {
                id: eyebrow
                spacing: 6 * root.s
                BrandMark {
                    anchors.verticalCenter: parent.verticalCenter
                    size: 11 * root.s
                    color: root.accent
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "NOW PLAYING"
                    color: root.accent
                    font.family: Theme.fontPrimary
                    font.pixelSize: 10 * root.s
                    font.letterSpacing: 1.5
                }
            }
            // raises the player's app.
            MaterialIcon {
                visible: !!root.player && root.player.canRaise
                anchors.right: parent.right
                anchors.verticalCenter: eyebrow.verticalCenter
                name: "open_in_new"
                color: openHover.hovered ? root.ink : root.inkDim
                size: 14 * root.s
                HoverHandler { id: openHover; cursorShape: Qt.PointingHandCursor }
                TapHandler { onTapped: if (root.player && root.player.canRaise) root.player.raise() }
            }
        }
        Text {
            width: parent.width
            text: root.title
            color: root.ink
            font.family: Theme.fontPrimary
            font.pixelSize: 15 * root.s
            font.weight: Font.DemiBold
            elide: Text.ElideRight
        }
        Text {
            width: parent.width
            visible: text.length > 0
            text: root.artist
            color: root.inkDim
            font.family: Theme.fontPrimary
            font.pixelSize: 12 * root.s
            elide: Text.ElideRight
        }
    }

    // one transport glyph; dims when the player can't take the step.
    component TransportButton: Item {
        id: tb
        property string icon: ""
        property bool available: true
        signal tapped()

        width: 26 * root.s
        height: 26 * root.s
        opacity: tb.available ? 1 : 0.35

        Rectangle {
            anchors.fill: parent
            radius: width / 2
            color: tbHover.hovered && tb.available ? root.line : "transparent"
        }
        MaterialIcon {
            anchors.centerIn: parent
            name: tb.icon
            size: 14 * root.s
            color: root.ink
        }
        HoverHandler { id: tbHover; enabled: tb.available; cursorShape: Qt.PointingHandCursor }
        TapHandler { enabled: tb.available; onTapped: tb.tapped() }
    }

    Row {
        id: transport
        anchors.horizontalCenter: seek.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: root.pad - 2 * root.s
        spacing: 4 * root.s

        TransportButton {
            icon: "skip_previous"
            available: root.hasPlayer && root.player.canGoPrevious
            onTapped: root.player.previous()
        }
        TransportButton {
            icon: root.playing ? "pause" : "play_arrow"
            available: root.hasPlayer && root.player.canTogglePlaying
            onTapped: root.player.togglePlaying()
        }
        TransportButton {
            icon: "skip_next"
            available: root.hasPlayer && root.player.canGoNext
            onTapped: root.player.next()
        }
    }

    Text {
        anchors.left: seek.left
        anchors.verticalCenter: transport.verticalCenter
        text: root.stamp(root.scrubbing ? root.scrubFrac * root.lengthSec : root.positionSec)
        color: root.inkDim
        font.family: Theme.mono
        font.pixelSize: 10 * root.s
        font.features: ({ "tnum": 1 })
    }
    Text {
        anchors.right: seek.right
        anchors.verticalCenter: transport.verticalCenter
        text: root.stamp(root.lengthSec)
        color: root.inkDim
        font.family: Theme.mono
        font.pixelSize: 10 * root.s
        font.features: ({ "tnum": 1 })
    }

    // Wavy seekbar: a flat base line with a sine over the played part while
    // playing. drawFrac glides between the 500 ms polls so the fill advances
    // smoothly; press or drag to preview a seek, committed on release.
    Canvas {
        id: seek
        anchors.left: cover.right
        anchors.right: parent.right
        anchors.bottom: transport.top
        anchors.leftMargin: 14 * root.s
        anchors.rightMargin: root.pad
        anchors.bottomMargin: 4 * root.s
        height: 12 * root.s

        property real phase: 0
        property real drawFrac: root.scrubbing ? root.scrubFrac : root.frac
        Behavior on drawFrac { enabled: !root.scrubbing; NumberAnimation { duration: 480; easing.type: Easing.Linear } }
        onDrawFracChanged: requestPaint()

        MouseArea {
            anchors.fill: parent
            anchors.topMargin: -8 * root.s
            anchors.bottomMargin: -8 * root.s
            enabled: root.canSeek
            cursorShape: root.canSeek ? Qt.PointingHandCursor : Qt.ArrowCursor
            preventStealing: true
            function fracAt(x) { return Math.max(0, Math.min(1, x / seek.width)); }
            onPressed: m => { root.scrubbing = true; root.scrubFrac = fracAt(m.x); }
            onPositionChanged: m => { if (root.scrubbing) root.scrubFrac = fracAt(m.x); }
            onReleased: m => {
                if (!root.scrubbing)
                    return;
                if (root.player)
                    root.player.position = fracAt(m.x) * root.lengthSec;
                root.scrubbing = false;
            }
            onCanceled: root.scrubbing = false
        }

        onPaint: {
            const ctx = getContext("2d");
            ctx.reset();
            const w = width;
            const cy = height / 2;
            const fillW = w * seek.drawFrac;

            ctx.strokeStyle = root.line;
            ctx.lineWidth = 2 * root.s;
            ctx.beginPath();
            ctx.moveTo(fillW, cy);
            ctx.lineTo(w, cy);
            ctx.stroke();

            const amp = root.playing ? 3 * root.s : 0;
            const waves = Math.max(1, Math.round(fillW / (24 * root.s)));
            const steps = waves * 16;
            ctx.strokeStyle = root.accent;
            ctx.lineWidth = 2.5 * root.s;
            ctx.beginPath();
            for (let i = 0; i <= steps; i++) {
                const x = (fillW * i) / steps;
                const y = cy + amp * Math.sin(waves * 2 * Math.PI * (i / steps) + seek.phase);
                if (i === 0)
                    ctx.moveTo(x, y);
                else
                    ctx.lineTo(x, y);
            }
            ctx.stroke();
        }
    }

    // ~30 fps is plenty for a 3px wiggle; FrameAnimation repainted every vsync.
    Timer {
        interval: 33
        repeat: true
        running: root.open && root.playing
        onTriggered: {
            seek.phase = Date.now() / 400;
            seek.requestPaint();
        }
    }

    // a pause or a track change repaints the bar even with the wave stopped.
    onPlayingChanged: seek.requestPaint()
    onFracChanged: seek.requestPaint()
}
