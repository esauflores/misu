pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell

// Per-screen bar state and IPC-facing menu requests.
Singleton {
    id: root

    // Filled in place (never reassigned), so a binding that calls forScreen
    // does not depend on its own write.
    readonly property var _states: ({})
    function forScreen(screen) {
        const key = screen ? screen.name : "";
        if (!root._states[key])
            root._states[key] = stateComp.createObject(root);
        return root._states[key];
    }
    Component {
        id: stateComp
        QtObject { property bool barRevealed: true }
    }

    // monitor name -> FrameMenuManager
    property var _managers: ({})
    function registerMenus(name, manager) {
        const next = Object.assign({}, root._managers);
        if (manager) next[name] = manager; else delete next[name];
        root._managers = next;
    }
    function focusedManager() {
        const name = Wm.focusedOutput;
        if (name && root._managers[name]) return root._managers[name];
        for (const k in root._managers) return root._managers[k];
        return null;
    }
    function toggleMenu(id) {
        const m = root.focusedManager();
        if (m) m.openSurface(id, null);
    }
    function toggleBar() {
        for (const k in root._states) root._states[k].barRevealed = !root._states[k].barRevealed;
    }
}
