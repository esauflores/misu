pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import "../../popouts"
import "../../popouts/match.js" as Match
import "../.."
import qs.services

// Clipboard history card (Super+Shift+V), the launcher's twin over cliphist: the
// 力 CLIPBOARD title, a search field, the history newest first (text as up
// to two lines, images as a thumbnail with their format and size) and a
// key-hint footer. Enter or a click copies the entry back to the clipboard
// and closes; Shift+Delete or the row's bin removes it from the history.
// Thumbnails are decoded once into ~/.cache/misu/clipboard and pruned with
// the history. Takes the keyboard while open (keyboard: "exclusive").
Item {
    id: root

    property real s: 1
    property bool open: false
    signal requestClose()

    readonly property real pad: 12 * root.s
    readonly property real rowH: 52 * root.s
    readonly property int rows: 6
    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property color inkDim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)
    readonly property color line: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.14)

    readonly property string thumbDir: (Quickshell.env("XDG_CACHE_HOME") || (Quickshell.env("HOME") + "/.cache")) + "/misu/clipboard"

    property string query: ""
    property int current: 0
    property bool loaded: false

    // ── history: [{ id, text, image: { ext, size, dims } | null }] ────────────
    property var entries: []

    // Lists the history; decodes each new image into thumbDir and drops the
    // thumbnails of entries that are gone. Prints "id<TAB>preview" per entry.
    Process {
        id: lister
        command: ["bash", "-c", `
            dir=$1; mkdir -p "$dir"
            declare -A keep
            while IFS=$'\\t' read -r id rest; do
              if [[ $rest =~ ^\\[\\[\\ binary\\ data\\ .*\\ (png|jpeg|jpg|webp|gif|bmp)\\ [0-9]+x[0-9]+\\ \\]\\]$ ]]; then
                f="$dir/$id.\${BASH_REMATCH[1]}"
                keep[$f]=1
                [[ -s $f ]] || printf '%s\\t\\n' "$id" | cliphist decode >"$f" 2>/dev/null
              fi
              printf '%s\\t%s\\n' "$id" "$rest"
            done < <(cliphist -preview-width 300 list)
            for f in "$dir"/*; do [[ -e $f && -z \${keep[$f]} ]] && rm -f -- "$f"; done
            exit 0`, "misu-clipboard", root.thumbDir]
        stdout: StdioCollector {
            onStreamFinished: {
                const out = [];
                for (const ln of this.text.split("\n")) {
                    const tab = ln.indexOf("\t");
                    if (tab <= 0) continue;
                    const id = ln.slice(0, tab);
                    const text = ln.slice(tab + 1);
                    const m = text.match(/^\[\[ binary data (\S+ \S+) (\S+) (\d+x\d+) \]\]$/);
                    out.push({
                        id: id,
                        text: text,
                        image: m ? { size: m[1], ext: m[2], dims: m[3].replace("x", "×") } : null
                    });
                }
                root.entries = out;
                root.loaded = true;
            }
        }
    }

    function refresh() { lister.running = true; }

    // ── matching: the query anywhere in the text (or "image"/the format) ─────
    readonly property var matches: {
        const q = root.query.trim().toLowerCase();
        if (q === "") return root.entries;
        return root.entries.filter(e => e.image
            ? ("image " + e.image.ext).includes(q)
            : e.text.toLowerCase().includes(q));
    }
    onMatchesChanged: root.current = Math.min(root.current, Math.max(0, root.matches.length - 1))
    onQueryChanged: root.current = 0

    function thumb(e) { return "file://" + root.thumbDir + "/" + e.id + "." + e.image.ext; }

    function copy(e) {
        if (!e) return;
        Quickshell.execDetached(["sh", "-c", "printf '%s\\t\\n' \"$1\" | cliphist decode | wl-copy", "misu-clipboard", e.id]);
        root.requestClose();
    }
    function remove(e) {
        if (!e) return;
        Quickshell.execDetached(["sh", "-c", "printf '%s\\t\\n' \"$1\" | cliphist delete", "misu-clipboard", e.id]);
        root.entries = root.entries.filter(x => x.id !== e.id);
    }
    function move(by) {
        const n = root.matches.length;
        if (n === 0) return;
        root.current = (root.current + by + n) % n;
        list.positionViewAtIndex(root.current, ListView.Contain);
    }

    // Fresh history and search each time it opens, with the field focused.
    onOpenChanged: if (root.open) {
        search.reset();
        root.current = 0;
        list.positionViewAtBeginning();
        root.refresh();
    }

    implicitWidth: 400 * root.s
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
            title: "CLIPBOARD"
            note: root.matches.length + (root.matches.length === 1 ? " entry" : " entries")
        }
        PopoutSearch {
            id: search
            s: root.s
            placeholder: "Search clipboard…"
            page: root.rows
            onTextChanged: root.query = search.text
            onAccepted: root.copy(root.matches[root.current])
            onMove: by => root.move(by)
            onKey: event => {
                if (event.key === Qt.Key_Delete && (event.modifiers & Qt.ShiftModifier)) {
                    root.remove(root.matches[root.current]);
                    event.accepted = true;
                }
            }
        }

        // history: as tall as it is, up to $rows (6 fit a 768p panel with the footer)
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
                readonly property bool isImage: row.modelData.image !== null

                width: list.width
                height: root.rowH
                s: root.s
                selected: row.index === root.current
                onTapped: root.copy(row.modelData)

                // image: a thumbnail on a hairline frame
                Rectangle {
                    id: frame
                    visible: row.isImage
                    anchors.left: parent.left
                    anchors.leftMargin: 12 * root.s
                    anchors.verticalCenter: parent.verticalCenter
                    width: row.isImage ? 64 * root.s : 0
                    height: 40 * root.s
                    color: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.04)
                    border.width: Theme.borderWidth
                    border.color: root.line
                    Image {
                        anchors.fill: parent
                        anchors.margins: 1
                        source: row.isImage ? root.thumb(row.modelData) : ""
                        sourceSize.width: 128 * root.s
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                        cache: false
                    }
                }

                Text {
                    anchors.left: frame.right
                    anchors.right: bin.left
                    anchors.leftMargin: row.isImage ? 12 * root.s : 0
                    anchors.rightMargin: 8 * root.s
                    anchors.verticalCenter: parent.verticalCenter
                    text: row.isImage
                        ? (row.modelData.image.ext.toUpperCase() + " image · " + row.modelData.image.dims + " · " + row.modelData.image.size)
                        : Match.marked(row.modelData.text, root.query)
                    textFormat: row.isImage ? Text.PlainText : Text.StyledText
                    color: row.isImage ? root.inkDim : row.selected ? root.ink : Theme.surfaceText
                    font.family: Theme.fontPrimary
                    font.pixelSize: (row.isImage ? Theme.h5 : Theme.h4) * root.s
                    wrapMode: Text.WrapAnywhere
                    maximumLineCount: 2
                    elide: Text.ElideRight
                }

                // remove from history (hover or selected)
                MaterialIcon {
                    id: bin
                    anchors.right: parent.right
                    anchors.rightMargin: 10 * root.s
                    anchors.verticalCenter: parent.verticalCenter
                    name: "delete"
                    size: 15 * root.s
                    color: binHover.hovered ? Theme.vermLit : root.inkDim
                    opacity: row.selected || row.hovered ? 1 : 0
                    HoverHandler { id: binHover }
                    TapHandler { onTapped: root.remove(row.modelData) }
                }
            }

            Text {
                anchors.centerIn: parent
                visible: root.matches.length === 0
                text: !root.loaded ? "Loading…"
                    : root.query.trim() === "" ? "Clipboard history is empty"
                    : "Nothing matches “" + root.query.trim() + "”"
                color: root.inkDim
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h5 * root.s
            }
        }

        Rectangle { width: parent.width; height: 1; color: root.line }
        PopoutKeyHints {
            s: root.s
            keys: [["↵", "copy"], ["⇧del", "remove"], ["↑↓", "move"], ["esc", "close"]]
        }
    }
}
