pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io

// Dictation state for the subtitles: misu-dictate and its server write
// $XDG_RUNTIME_DIR/misu-dictate/state.json ({state, text, lines}: lines is the
// live caption, one line; text is the final transcript). The
// file comes and goes, so a watch isn't enough: `misu-dictate start` calls
// `ipc call dictate wake` and the file is read ten times a second until
// dictation is over.
Singleton {
    id: root

    property string phase: "idle"     // loading, listening, finishing, done, idle
    property string text: ""
    property var lines: []
    property bool lingering: false    // "done" stays on screen briefly
    property bool polling: false
    property string source: "mic"     // mic, computer or both (misu-dictate source)
    property string language: "en"    // en or es (misu-dictate lang)
    // the dictate module is optional: without misu-dictate on PATH the rail
    // button stays hidden (checked when the bar starts)
    property bool installed: false

    // loading: the server's first start, its model still coming in (the
    // audio is already being kept)
    readonly property bool active: root.phase === "loading" || root.phase === "listening"
        || root.phase === "finishing"

    readonly property bool shown: root.active
        || (root.phase === "done" && root.lingering)

    function apply(raw) {
        try {
            const st = JSON.parse(raw);
            const was = root.phase;
            root.phase = st.state || "idle";
            root.text = st.text || "";
            root.lines = st.lines || [];
            if (root.phase === "done" && was !== "done") {
                root.lingering = true;
                linger.restart();
            }
            if (root.phase === "idle")
                root.polling = false;
        } catch (e) {
            // half-written; the next read gets the whole file
        }
    }

    function toggle() {
        Quickshell.execDetached(["misu-dictate", "toggle"]);
    }

    function setLanguage(l) {
        root.language = l;
        Quickshell.execDetached(["misu-dictate", "lang", l]);
    }

    function setSource(s) {
        root.source = s;
        Quickshell.execDetached(["misu-dictate", "source", s]);
    }

    Process {
        running: true
        command: ["sh", "-c", "command -v misu-dictate"]
        onExited: code => root.installed = code === 0
    }

    Process {
        running: root.installed
        command: ["misu-dictate", "source"]
        stdout: StdioCollector {
            onStreamFinished: {
                const s = this.text.trim();
                if (s !== "")
                    root.source = s;
            }
        }
    }

    Process {
        running: root.installed
        command: ["misu-dictate", "lang"]
        stdout: StdioCollector {
            onStreamFinished: {
                const l = this.text.trim();
                if (l !== "")
                    root.language = l;
            }
        }
    }

    FileView {
        id: file
        path: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/misu-dictate/state.json"
        printErrors: false
        onLoaded: root.apply(text())
    }

    Timer {
        interval: 100
        // also while a dictation looks under way: a bar reload mid-dictation
        // drops the wake, and the outcome must still be read
        running: root.polling || root.active
        repeat: true
        triggeredOnStart: true
        onTriggered: file.reload()
    }

    Timer {
        id: linger
        interval: 1000
        onTriggered: {
            root.lingering = false;
            root.polling = false;
        }
    }

    IpcHandler {
        target: "dictate"
        function wake(): void { root.polling = true; }
    }
}
