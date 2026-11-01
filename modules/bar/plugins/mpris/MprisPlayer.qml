import QtQuick
import Quickshell.Services.Mpris

// The MPRIS player the rail button and card follow: the first one playing,
// else the first registered.
QtObject {
    readonly property var player: {
        const players = Mpris.players.values;
        for (let i = 0; i < players.length; ++i)
            if (players[i] && players[i].isPlaying) return players[i];
        return players.length > 0 ? players[0] : null;
    }
    readonly property bool playing: player !== null && player.isPlaying
}
