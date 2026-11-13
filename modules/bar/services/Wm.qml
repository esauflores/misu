pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.I3
import Quickshell.Io

// scroll (a sway fork) over the i3 IPC socket. Workspaces and outputs come from
// Quickshell.I3; per-output fullscreen is read from the tree, refreshed on
// window and workspace events.
// Under wm (~/Projects/wm, $WM_SOCK set) they come from `wm msg subscribe`
// instead: its seats and screens, as the seat the host drives sees them.
// Outside both every list is empty.
Singleton {
    id: root

    readonly property bool isWm: (Quickshell.env("WM_SOCK") || "") !== ""
    readonly property bool connected: !root.isWm
        && (Quickshell.env("SWAYSOCK") || Quickshell.env("I3SOCK") || "") !== ""


    readonly property var workspaces: {
        if (root.isWm) return root.wmWorkspaces;
        if (!root.connected) return [];
        const vals = I3.workspaces ? I3.workspaces.values : [];
        const out = [];
        for (let i = 0; i < vals.length; ++i) {
            const w = vals[i];
            if (!w) continue;
            out.push({
                id: w.id, name: w.name, number: w.number,
                active: w.active, focused: w.focused, urgent: w.urgent,
                output: w.monitor ? w.monitor.name : "", special: false
            });
        }
        out.sort((a, b) => a.number - b.number);
        return out;
    }

    readonly property string focusedOutput: root.isWm ? root.wmFocusedOutput
        : root.connected && I3.focusedMonitor ? I3.focusedMonitor.name : ""



    function dispatch(cmd) {
        if (root.connected) I3.dispatch(cmd);
    }
    // Workspace by number when the name is numeric, so a dot for a workspace
    // that does not exist yet creates it.
    function focusWorkspace(name) {
        const n = parseInt(name, 10);
        if (root.isWm) {
            if (n >= 1 && n <= 9) root.wmMsg(["workspace", String(n)]);
            return;
        }
        root.dispatch(String(n) === String(name) ? "workspace number " + n : "workspace \"" + name + "\"");
    }
    function cycleWorkspace(dir) {
        if (root.isWm) {
            const n = (root.wmMe ? root.wmMe.number || 1 : 1) + (dir > 0 ? 1 : -1);
            if (n >= 1 && n <= 9) root.wmMsg(["workspace", String(n)]);
            return;
        }
        root.dispatch(dir > 0 ? "workspace next_on_output" : "workspace prev_on_output");
    }

    // output name -> true while its visible workspace holds a fullscreen window
    property var fullscreen: ({})
    function outputHasFullscreen(name) { return root.fullscreen[name] === true; }

    function scanTree(node, output, visible, out) {
        if (!node) return;
        if (node.type === "output") output = node.name;
        if (node.type === "workspace") visible = root.visibleNames[node.name] === true;
        // only windows count: sway reports fullscreen_mode 1 on workspace nodes
        if (visible && (node.type === "con" || node.type === "floating_con")
                && node.fullscreen_mode > 0 && output)
            out[output] = true;
        for (const k of ["nodes", "floating_nodes"])
            for (const c of (node[k] || [])) root.scanTree(c, output, visible, out);
    }
    readonly property var visibleNames: {
        const s = {};
        for (const w of root.workspaces) if (w.active) s[w.name] = true;
        return s;
    }

    Process {
        id: treeProc
        command: ["scrollmsg", "-r", "-t", "get_tree"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const out = {};
                    root.scanTree(JSON.parse(text), "", false, out);
                    root.fullscreen = out;
                } catch (e) {}
            }
        }
    }
    Timer {
        id: treeDebounce
        interval: 80
        onTriggered: if (root.connected && !treeProc.running) treeProc.running = true
    }
    Connections {
        target: root.connected ? I3 : null
        function onRawEvent(event) {
            const t = event ? event.type : "";
            if (t === "window" || t === "workspace")
                treeDebounce.restart();
        }
    }
    Component.onCompleted: if (root.connected) treeDebounce.restart()

    // wm: what `wm msg subscribe` says, by id. Each event carries the whole
    // seat or screen; "gone" only its id.
    property var wmSeats: ({})
    property var wmScreens: ({})

    function wmMsg(args) {
        Quickshell.execDetached(["wm", "msg"].concat(args));
    }

    function wmEvent(line) {
        let e;
        try { e = JSON.parse(line); } catch (err) { return; }
        const key = e.event === "seat" ? "wmSeats" : e.event === "screen" ? "wmScreens" : "";
        if (key === "") return;
        const next = Object.assign({}, root[key]);
        if (e.change === "gone") delete next[e.id];
        else next[e.id] = e;
        root[key] = next;
    }

    // The seat the host's devices drive: the user's.
    readonly property var wmMe: {
        const seats = Object.values(root.wmSeats);
        return seats.find(s => s.active) || root.wmSeats[0] || null;
    }
    readonly property string wmFocusedOutput: {
        const screen = root.wmMe ? root.wmScreens[root.wmMe.screen] : null;
        return screen ? screen.name : "";
    }
    // The connected screens' workspaces, numbered from 1 on each.
    readonly property var wmWorkspaces: {
        const screens = Object.values(root.wmScreens).filter(s => s.connected);
        screens.sort((a, b) => a.position - b.position);
        const me = root.wmMe;
        const out = [];
        for (const screen of screens)
            screen.workspaces.forEach((id, i) => out.push({
                id: id, name: String(i + 1), number: i + 1,
                active: screen.active === id, focused: me !== null && me.workspace === id,
                urgent: false, output: screen.name, special: false
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
