pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.I3
import Quickshell.Io

// scroll (a sway fork) over the i3 IPC socket. Workspaces and outputs come from
// Quickshell.I3; per-output fullscreen is read from the tree, refreshed on
// window and workspace events. Outside scroll every list is empty.
Singleton {
    id: root

    readonly property bool connected: (Quickshell.env("SWAYSOCK") || Quickshell.env("I3SOCK") || "") !== ""


    readonly property var workspaces: {
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

    readonly property string focusedOutput: root.connected && I3.focusedMonitor ? I3.focusedMonitor.name : ""



    function dispatch(cmd) {
        if (root.connected) I3.dispatch(cmd);
    }
    // Workspace by number when the name is numeric, so a dot for a workspace
    // that does not exist yet creates it.
    function focusWorkspace(name) {
        const n = parseInt(name, 10);
        root.dispatch(String(n) === String(name) ? "workspace number " + n : "workspace \"" + name + "\"");
    }
    function cycleWorkspace(dir) {
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
    Component.onCompleted: treeDebounce.restart()
}
