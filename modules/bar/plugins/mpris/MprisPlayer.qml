pragma Singleton

import QtQuick
import Quickshell
import Quickshell.Services.Mpris

// The MPRIS player the rail button and card follow: the first one playing,
// else the first registered. One shared instance, kept alive by the rail.
//
// `length` is the track's length in seconds, 0 when unknown. Firefox-based
// browsers (Zen) send it only alongside a position update (a track change,
// seek, play or pause) and drop it from the next metadata a moment later, so
// it is remembered for the track it came with.
Singleton {
    id: root

    readonly property var player: {
        const players = Mpris.players.values;
        for (let i = 0; i < players.length; ++i)
            if (players[i] && players[i].isPlaying) return players[i];
        return players.length > 0 ? players[0] : null;
    }
    readonly property bool playing: player !== null && player.isPlaying

    readonly property string trackKey: root.player ? (root.player.trackTitle || "") + "\n" + (root.player.trackArtist || "") : ""
    property string lengthKey: ""
    property real lengthSeen: 0
    readonly property real length: root.player && root.player.lengthSupported && root.player.length > 0
        ? root.player.length
        : (root.lengthKey !== "" && root.lengthKey === root.trackKey ? root.lengthSeen : 0)

    // The title can land just after the length in the same update, so take
    // the track key a tick later.
    function remember() {
        if (!root.player || !root.player.lengthSupported || !(root.player.length > 0)) return;
        const len = root.player.length;
        Qt.callLater(() => { root.lengthSeen = len; root.lengthKey = root.trackKey; });
    }
    // Next/Previous. Firefox-based browsers (Zen) turn CanGoNext/CanGoPrevious
    // on once the page registers its media-session handlers but never signal
    // the change, so Quickshell keeps the false it first read and refuses the
    // call; the method is sent over D-Bus directly then.
    function skip(forward) {
        const p = root.player;
        if (!p) return;
        root.skipFrom = root.trackKey;
        root.skipping = true;
        skipTimeout.restart();
        if (forward ? p.canGoNext : p.canGoPrevious)
            forward ? p.next() : p.previous();
        else if (p.canControl && p.dbusName)
            Quickshell.execDetached(["busctl", "--user", "call", p.dbusName, "/org/mpris/MediaPlayer2",
                "org.mpris.MediaPlayer2.Player", forward ? "Next" : "Previous"]);
    }

    // A browser skips only once the next page has loaded, which can take a
    // while, so `skipping` holds from the click until a new titled track
    // shows up (a browser passes through an empty title on the way), the
    // player goes away, or 15 s pass.
    property bool skipping: false
    property string skipFrom: ""
    onTrackKeyChanged: {
        if (root.skipping && root.player && root.player.trackTitle && root.trackKey !== root.skipFrom)
            root.skipping = false;
    }
    onPlayerChanged: root.skipping = false
    Timer {
        id: skipTimeout
        interval: 15000
        onTriggered: root.skipping = false
    }

    Connections {
        target: root.player
        function onLengthChanged() { root.remember(); }
        function onLengthSupportedChanged() { root.remember(); }
    }
}
