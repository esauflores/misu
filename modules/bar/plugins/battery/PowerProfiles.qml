pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io

// power-profiles-daemon through powerprofilesctl, polled only while an owner
// (an open menu or card) asks.
Singleton {
    id: root

    property bool available: false
    property string profile: ""
    property var profiles: []
    property var owners: []

    function setActive(owner, on) {
        const list = root.owners.filter(o => o !== owner);
        if (on) list.push(owner);
        root.owners = list;
        if (on) root.poll();
    }
    function poll() { if (!query.running) query.running = true; }
    function setProfile(name) {
        root.profile = name;
        Quickshell.execDetached(["powerprofilesctl", "set", name]);
        pollLater.restart();
    }

    Process {
        id: query
        command: ["powerprofilesctl", "list"]
        stdout: StdioCollector {
            onStreamFinished: {
                const list = [];
                let cur = "";
                for (const line of text.split("\n")) {
                    const m = line.match(/^(\*?)\s*([a-z-]+):\s*$/);
                    if (!m) continue;
                    list.push(m[2]);
                    if (m[1] === "*") cur = m[2];
                }
                // order: saver, balanced, performance
                const order = ["power-saver", "balanced", "performance"];
                list.sort((a, b) => order.indexOf(a) - order.indexOf(b));
                root.profiles = list;
                root.profile = cur;
                root.available = list.length > 0;
            }
        }
        onExited: code => { if (code !== 0) root.available = false; }
    }
    Timer { id: pollLater; interval: 400; onTriggered: root.poll() }
    Timer { interval: 5000; repeat: true; running: root.owners.length > 0; onTriggered: root.poll() }
    Component.onCompleted: root.poll()
}
