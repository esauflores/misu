pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io

Singleton {
    id: root
    readonly property bool isWm: (Quickshell.env("WM_SOCK") || "") !== ""

    function focusWorkspace(name) {
        const n = Number(name);
        if (Number.isInteger(n) && n >= 1 && n <= 9)
            root.wmMsg(["workspace", String(n)]);
    }
    function cycleWorkspace(dir) {
        root.focusWorkspace((root.wmMe ? root.wmMe.number || 1 : 1) + (dir > 0 ? 1 : -1));
    }
    // wm: what `wm msg subscribe` says, by id. Each event carries the whole
    // seat or screen; "gone" only its id.
    property var wmSeats: ({})
    property var wmScreens: ({})
    property var wmWindows: ({})

    function wmMsg(args) {
        Quickshell.execDetached(["wm", "msg", "seat", "0"].concat(args));
    }

    function wmEvent(line) {
        let e;
        try { e = JSON.parse(line); } catch (err) { return; }
        const key = e.event === "seat" ? "wmSeats" : e.event === "screen" ? "wmScreens"
            : e.event === "window" ? "wmWindows" : "";
        if (key === "") return;
        const next = Object.assign({}, root[key]);
        if (e.change === "gone") delete next[e.id];
        else next[e.id] = e;
        root[key] = next;
    }

    // The seat the host's devices drive: the user's.
    readonly property var wmMe: root.wmSeats[0] || null
    readonly property string focusedOutput: {
        const screen = root.wmMe ? root.wmScreens[root.wmMe.screen] : null;
        return screen ? screen.name : "";
    }
    // The connected screens' workspaces, numbered from 1 on each. wm keeps
    // nine per screen; `occupied` says which hold windows.
    readonly property var workspaces: {
        const busy = {};
        for (const w of Object.values(root.wmWindows)) busy[w.workspace] = true;
        const screens = Object.values(root.wmScreens).filter(s => s.connected);
        screens.sort((a, b) => a.position - b.position);
        const me = root.wmMe;
        const out = [];
        for (const screen of screens)
            screen.workspaces.forEach((id, i) => out.push({
                name: String(i + 1), number: i + 1, occupied: busy[id] === true,
                active: screen.active === id, focused: me !== null && me.workspace === id
            }));
        return out;
    }

    Process {
        id: wmSubscribe
        command: ["wm", "msg", "subscribe"]
        running: root.isWm
        onStarted: {
            root.wmSeats = {};
            root.wmScreens = {};
            root.wmWindows = {};
        }
        // wm restarted, or the socket hiccuped: listen again.
        onExited: if (root.isWm) wmRetry.restart()
        stdout: SplitParser {
            onRead: line => root.wmEvent(line)
        }
    }
    Timer {
        id: wmRetry
        interval: 1000
        onTriggered: wmSubscribe.running = true
    }
}
