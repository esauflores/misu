pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Widgets
import "../../popouts"
import "../../popouts/match.js" as Match
import "../.."
import qs.services

// App launcher card (Super+Space): the 力 LAUNCH title, a search field, the
// matching desktop apps (icon, name with the match in bold, what it is)
// and a key-hint footer. The list fits its results, up to $rows. Apps you
// launch often and lately come first; launches are counted in the shell's
// state dir (launcher.json). Type to filter, Up/Down (Tab, Ctrl+N/P/J/K) to
// move, Enter or a click to launch, Esc closes. Takes the keyboard while open
// (keyboard: "exclusive" in index.js).
Item {
    id: root

    property real s: 1
    property bool open: false
    signal requestClose()

    readonly property real pad: 12 * root.s
    readonly property real rowH: 46 * root.s
    readonly property int rows: 8
    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property color inkDim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)
    readonly property color line: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.14)

    property string query: ""
    property int current: 0

    // ── usage: desktop entry id -> { n: launches, t: last launch (ms) } ────────
    property var usage: ({})
    FileView {
        id: usageFile
        path: Quickshell.statePath("launcher.json")
        printErrors: false
        onLoaded: {
            try { root.usage = JSON.parse(usageFile.text()) || ({}); }
            catch (e) { root.usage = ({}); }
        }
    }
    // Launch count, faded by age: a week-old launch counts half.
    function frecency(e) {
        const u = root.usage[e.id];
        if (!u) return 0;
        const days = (Date.now() - (u.t || 0)) / 86400000;
        return u.n / (1 + days / 7);
    }
    function remember(e) {
        const u = Object.assign({}, root.usage);
        const prev = u[e.id] || { n: 0, t: 0 };
        u[e.id] = { n: prev.n + 1, t: Date.now() };
        root.usage = u;
        usageFile.setText(JSON.stringify(u));
    }

    // ── matching ───────────────────────────────────────────────────────────────
    // Best match first: name prefix, a word in the name starting with the
    // query, the query anywhere in the name, then in the generic name, comment,
    // keywords or categories; ties go to the most used, then A to Z. An empty
    // query lists everything, most used first.
    readonly property var matches: {
        const q = root.query.trim().toLowerCase();
        const all = DesktopEntries.applications.values.filter(e => e && !e.noDisplay);
        const scored = [];
        for (const e of all) {
            const name = (e.name || "").toLowerCase();
            let score = 0;
            if (q !== "") {
                const extra = [e.genericName, e.comment, (e.keywords || []).join(" "), (e.categories || []).join(" ")]
                    .join(" ").toLowerCase();
                if (name.startsWith(q)) score = 0;
                else if (name.split(/[\s\-_.]+/).some(w => w.startsWith(q))) score = 1;
                else if (name.includes(q)) score = 2;
                else if (extra.includes(q)) score = 3;
                else continue;
            }
            scored.push({ e: e, score: score, use: root.frecency(e) });
        }
        scored.sort((a, b) => a.score - b.score || b.use - a.use || a.e.name.localeCompare(b.e.name));
        return scored.map(x => x.e);
    }
    onMatchesChanged: root.current = 0

    function launch(entry) {
        if (!entry) return;
        root.remember(entry);
        if (entry.runInTerminal)
            Quickshell.execDetached(["foot", "-e"].concat(entry.command));
        else
            entry.execute();
        root.requestClose();
    }
    function move(by) {
        const n = root.matches.length;
        if (n === 0) return;
        root.current = (root.current + by + n) % n;
        list.positionViewAtIndex(root.current, ListView.Contain);
    }

    // Fresh search each time it opens, with the field focused.
    onOpenChanged: if (root.open) {
        search.reset();
        root.current = 0;
        list.positionViewAtBeginning();
    }

    implicitWidth: 380 * root.s
    implicitHeight: content.implicitHeight + root.pad * 2

    PopoutCard { anchors.fill: parent }

    Column {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: root.pad
        spacing: 10 * root.s

        PopoutHead {
            s: root.s
            title: "LAUNCH"
            note: root.matches.length + (root.matches.length === 1 ? " app" : " apps")
        }
        PopoutSearch {
            id: search
            s: root.s
            placeholder: "Search apps…"
            page: root.rows
            onTextChanged: root.query = search.text
            onAccepted: root.launch(root.matches[root.current])
            onMove: by => root.move(by)
        }

        // results: as tall as they are, up to $rows
        ListView {
            id: list
            width: parent.width
            height: Math.max(1, Math.min(root.rows, root.matches.length)) * root.rowH
            clip: true
            model: root.matches
            currentIndex: root.current
            boundsBehavior: Flickable.StopAtBounds
            highlightFollowsCurrentItem: false
            Behavior on height { NumberAnimation { duration: Motion.dur(120); easing.type: Easing.OutCubic } }

            delegate: PopoutRow {
                id: row
                required property var modelData
                required property int index

                width: list.width
                height: root.rowH
                s: root.s
                selected: row.index === root.current
                onTapped: root.launch(row.modelData)

                IconImage {
                    id: icon
                    anchors.left: parent.left
                    anchors.leftMargin: 12 * root.s
                    anchors.verticalCenter: parent.verticalCenter
                    implicitSize: 28 * root.s
                    source: Quickshell.iconPath(row.modelData.icon, "application-x-executable")
                    asynchronous: true
                }
                Column {
                    anchors.left: icon.right
                    anchors.right: enter.left
                    anchors.leftMargin: 12 * root.s
                    anchors.rightMargin: 8 * root.s
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 2 * root.s
                    Text {
                        width: parent.width
                        text: Match.marked(row.modelData.name, root.query)
                        textFormat: Text.StyledText
                        color: row.selected ? root.ink : Theme.surfaceText
                        font.family: Theme.fontPrimary
                        font.pixelSize: Theme.h4 * root.s
                        font.weight: row.selected ? Font.Medium : Font.Normal
                        elide: Text.ElideRight
                    }
                    Text {
                        width: parent.width
                        visible: text !== ""
                        text: row.modelData.genericName || row.modelData.comment || ""
                        color: root.inkDim
                        font.family: Theme.fontPrimary
                        font.pixelSize: Theme.h6 * root.s
                        elide: Text.ElideRight
                    }
                }
                // Enter launches the selected row
                MaterialIcon {
                    id: enter
                    anchors.right: parent.right
                    anchors.rightMargin: 10 * root.s
                    anchors.verticalCenter: parent.verticalCenter
                    name: "keyboard_return"
                    size: 14 * root.s
                    color: root.ink
                    opacity: row.selected ? 1 : 0
                }
            }

            Text {
                anchors.centerIn: parent
                visible: root.matches.length === 0
                text: "No apps match “" + root.query.trim() + "”"
                color: root.inkDim
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h5 * root.s
            }
        }

        // footer: the keys, on a hairline
        Rectangle { width: parent.width; height: 1; color: root.line }
        PopoutKeyHints {
            s: root.s
            keys: [["↵", "open"], ["↑↓", "move"], ["esc", "close"]]
        }
    }
}
