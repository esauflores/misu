pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import "usage.js" as Calc

// Plan usage for every AI provider `ai-usagebar` is set up for (Claude, Codex,
// Copilot, OpenRouter, DeepSeek, ...), read from `ai-usagebar usage --json`
// every five minutes and on demand. One poller serves the rail and the card.
// A failed read keeps the last report and marks it stale.
Singleton {
    id: root

    property var providers: []
    property bool installed: true
    property bool stale: false
    property string error: ""
    property real updated: 0        // unix seconds of the last good read
    // wall clock in unix seconds, ticking so pace and countdowns stay live.
    property real now: Date.now() / 1000

    readonly property bool refreshing: fetch.running
    readonly property string worst: Calc.worst(root.providers)

    function refresh() {
        if (!fetch.running)
            fetch.running = true;
    }

    Process {
        id: fetch
        command: ["sh", "-c", "command -v ai-usagebar >/dev/null || exit 127; exec ai-usagebar usage --json"]
        stdout: StdioCollector {
            onStreamFinished: {
                const list = Calc.parse(this.text);
                if (list === null)
                    return;
                root.providers = list;
                root.updated = Date.now() / 1000;
                root.stale = false;
                root.error = "";
            }
        }
        stderr: StdioCollector { id: err }
        onExited: code => {
            root.installed = code !== 127;
            if (code !== 0 && root.installed) {
                const lines = err.text.trim().split("\n");
                root.error = lines[lines.length - 1] || "ai-usagebar exited with " + code;
                root.stale = root.providers.length > 0;
            }
        }
    }

    Timer {
        interval: 5 * 60 * 1000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: root.refresh()
    }

    Timer {
        interval: 30000
        running: true
        repeat: true
        onTriggered: root.now = Date.now() / 1000
    }
}
