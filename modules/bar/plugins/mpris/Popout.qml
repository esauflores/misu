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
// total time either side of the transport (no bar when the player doesn't
// say how long the track is). It follows the active MPRIS player
// (MprisPlayer), named at the eyebrow's right. The position poll and the wave
// only run while the card is open and playing.
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


    readonly property var player: MprisPlayer.player
    readonly property bool hasPlayer: root.player !== null
    // a skip was sent and the player hasn't shown the next track yet.
    readonly property bool loading: MprisPlayer.skipping
    readonly property bool playing: MprisPlayer.playing && !root.loading
    readonly property string title: root.loading ? "Loading…"
        : root.hasPlayer && root.player.trackTitle ? root.player.trackTitle : "Nothing playing"
    readonly property string artist: {
        if (!root.hasPlayer || root.loading) return "";
        // trackArtists is a plain string in Quickshell 0.3; Array.from would
        // split it into letters.
        const list = root.player.trackArtists;
        const artists = (typeof list === "string" ? [list] : Array.from(list || [])).filter(Boolean);
        if (artists.length === 0) return root.player.trackArtist || "";
        if (artists.length === 1) return artists[0];
        return artists.slice(0, -1).join(", ") + " & " + artists[artists.length - 1];
    }
    readonly property string artUrl: root.hasPlayer && root.player.trackArtUrl ? root.player.trackArtUrl : ""
    readonly property real positionSec: root.hasPlayer ? root.player.position : 0
    // MprisPlayer keeps a browser's length past the update it came in. With
    // no length known (a browser that hasn't reported one for this track) the
    // position is only Quickshell's count since it saw the player, so the card
    // drops the times and seeking, and the wave runs the full width.
    readonly property real lengthSec: MprisPlayer.length
    readonly property bool timed: root.lengthSec > 0 && !root.loading
    readonly property real frac: root.loading ? 0
        : root.timed ? Math.max(0, Math.min(1, root.positionSec / root.lengthSec)) : 1
    readonly property bool canSeek: root.hasPlayer && root.player.canSeek && root.timed
    property bool scrubbing: false
    property real scrubFrac: 0

    readonly property real pad: 12 * root.s
    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property color inkDim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)
    readonly property color line: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.18)
    readonly property color accent: Theme.vermLit

    implicitWidth: 340 * root.s
    implicitHeight: 148 * root.s

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
            opacity: root.loading ? 0.35 : 1
            Behavior on opacity { NumberAnimation { duration: 150 } }
        }
        // spins over the cover while a skip is loading.
        MaterialIcon {
            anchors.centerIn: parent
            visible: root.loading
            name: "progress_activity"
            color: root.ink
            size: 28 * root.s
            RotationAnimation on rotation {
                running: root.loading && root.open
                from: 0; to: 360
                duration: 900
                loops: Animation.Infinite
            }
        }
        // music-note stand-in while there is no art.
        MaterialIcon {
            anchors.centerIn: parent
            visible: root.artUrl === "" && !root.loading
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

            PopoutTitle {
                id: eyebrow
                text: "NOW PLAYING"
                s: root.s
            }
            // which app is playing
            Text {
                anchors.left: eyebrow.right
                anchors.leftMargin: 8 * root.s
                anchors.right: parent.right
                anchors.verticalCenter: eyebrow.verticalCenter
                horizontalAlignment: Text.AlignRight
                text: root.hasPlayer ? root.player.identity || "" : ""
                color: root.inkDim
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h6 * root.s
                elide: Text.ElideRight
            }
        }
        Text {
            width: parent.width
            text: root.title
            color: root.ink
            font.family: Theme.fontPrimary
            font.pixelSize: Theme.h3 * root.s
            font.weight: Font.DemiBold
            elide: Text.ElideRight
        }
        Text {
            width: parent.width
            visible: text.length > 0
            text: root.artist
            color: root.inkDim
            font.family: Theme.fontPrimary
            font.pixelSize: Theme.h5 * root.s
            elide: Text.ElideRight
        }
    }

    // one transport glyph; dims when the player can't take the step.
    // `main` (play/pause) is a filled disc, the steps either side plain.
    component TransportButton: Item {
        id: tb
        property string icon: ""
        property bool available: true
        property bool main: false
        signal tapped()

        width: (tb.main ? 32 : 28) * root.s
        height: tb.width
        opacity: tb.available ? 1 : 0.35

        Rectangle {
            anchors.fill: parent
            radius: width / 2
            color: tb.main ? (tbHover.hovered && tb.available ? Qt.lighter(root.ink, 0.85) : root.ink)
                : (tbHover.hovered && tb.available ? root.line : "transparent")
            Behavior on color { ColorAnimation { duration: Motion.fast } }
        }
        MaterialIcon {
            anchors.centerIn: parent
            name: tb.icon
            size: (tb.main ? 18 : 17) * root.s
            color: tb.main ? Theme.effectiveSurface : root.ink
        }
        HoverHandler { id: tbHover; enabled: tb.available; cursorShape: Qt.PointingHandCursor }
        TapHandler { enabled: tb.available; onTapped: tb.tapped() }
    }

    Row {
        id: transport
        anchors.horizontalCenter: seek.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: root.pad - 2 * root.s
        spacing: 8 * root.s

        TransportButton {
            anchors.verticalCenter: parent.verticalCenter
            icon: "skip_previous"
            // canControl: see MprisPlayer.skip.
            available: root.hasPlayer && (root.player.canGoPrevious || root.player.canControl)
            onTapped: MprisPlayer.skip(false)
        }
        TransportButton {
            main: true
            icon: root.playing ? "pause" : "play_arrow"
            available: root.hasPlayer && root.player.canTogglePlaying
            onTapped: root.player.togglePlaying()
        }
        TransportButton {
            anchors.verticalCenter: parent.verticalCenter
            icon: "skip_next"
            available: root.hasPlayer && (root.player.canGoNext || root.player.canControl)
            onTapped: MprisPlayer.skip(true)
        }
    }

    Text {
        anchors.left: seek.left
        anchors.verticalCenter: transport.verticalCenter
        visible: root.timed
        text: root.stamp(root.scrubbing ? root.scrubFrac * root.lengthSec : root.positionSec)
        color: root.inkDim
        font.family: Theme.fontPrimary
        font.pixelSize: Theme.h6 * root.s
        font.features: ({ "tnum": 1 })
    }
    Text {
        anchors.right: seek.right
        anchors.verticalCenter: transport.verticalCenter
        visible: root.timed
        text: root.stamp(root.lengthSec)
        color: root.inkDim
        font.family: Theme.fontPrimary
        font.pixelSize: Theme.h6 * root.s
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
        anchors.bottomMargin: 6 * root.s
        height: 12 * root.s
        // a player that doesn't give the track's length gets no bar: a full
        // one reads as finished
        visible: root.timed

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
