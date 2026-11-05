pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import qs.services

// A screen share waiting for its source. wm's misu-share-pick calls
// `ipc call share ask <dir>`, with the sources in <dir>/options, one per line,
// their thumbnails in <dir>/thumb-<line>.jpg, and waits on the <dir>/answer FIFO for the chosen line. An empty answer
// declines the share; @fuzzel hands the question back, as the card can't show
// over a fullscreen window.
Singleton {
    id: root

    property string dir: ""
    property var options: []
    // The last request's folder: thumbnails stay put while the card slides out.
    property string thumbDir: ""
    // The CardManager showing the card: its card closing any way but a pick
    // declines the share.
    property var cards: null

    function ask(path: string): void {
        root.decline();     // one at a time: a newer request replaces an open one
        root.dir = path;
    }
    function pick(line: string): void { root.reply(line); }
    function decline(): void { root.reply(""); }

    function reply(line: string): void {
        if (!root.dir) return;
        const answer = root.dir + "/answer";
        root.dir = "";
        Quickshell.execDetached(["sh", "-c", 'printf "%s\\n" "$1" >"$2"', "sh", line, answer]);
    }

    FileView {
        path: root.dir ? root.dir + "/options" : ""
        printErrors: false
        onLoaded: {
            root.thumbDir = root.dir;
            root.options = text().split("\n").filter(l => l);
            if (!root.options.length) {
                root.decline();
            } else if (ShellState.openMenu("share")) {
                root.cards = ShellState.focusedManager();
            } else {
                root.reply("@fuzzel");
            }
        }
        onLoadFailed: root.decline()
    }

    Connections {
        target: root.cards || null     // gone once the bar reloads
        function onOpenIdChanged() {
            if (root.cards.openId !== "share") root.decline();
        }
    }
}
