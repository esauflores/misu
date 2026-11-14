pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import "../../popouts"
import "../.."
import qs.services

// Wallpaper card (Super+Shift+Q), the launcher's twin over `misu-wall`: the 力
// WALLPAPER title, a search field and a grid of the images in
// ~/Pictures/Wallpapers ($RICE_WALLPAPERS), the current one ringed. Enter
// or a click sets it (misu-wall set: swaybg plus the matugen palette, so the
// bar recolours with it) and closes. Arrows move on the grid (Left/Right
// while the search is empty), Esc closes. Takes the keyboard while open
// (keyboard: "exclusive" in index.js).
Item {
    id: root

    property real s: 1
    property bool open: false
    signal requestClose()

    readonly property real pad: 12 * root.s
    readonly property int cols: 3
    readonly property int rows: 3
    readonly property real gap: 8 * root.s
    readonly property real tileW: (content.width - root.gap * (root.cols - 1)) / root.cols
    readonly property real tileH: Math.round(root.tileW * 10 / 16) + 22 * root.s
    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property color inkDim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)
    readonly property color line: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.14)

    readonly property string dir: Quickshell.env("RICE_WALLPAPERS") || (Quickshell.env("HOME") + "/Pictures/Wallpapers")

    property string query: ""
    property int current: 0
    property bool loaded: false
    property var images: []        // absolute paths, A to Z
    property string active: ""     // the wallpaper set now

    Process {
        id: lister
        command: ["bash", "-c", `
            find "$1" -maxdepth 1 -type f \\( -iname '*.png' -o -iname '*.jpg' -o -iname '*.jpeg' -o -iname '*.webp' \\) | sort -f
            printf '@%s\\n' "$(misu-wall current 2>/dev/null)"`, "misu-wallpaper", root.dir]
        stdout: StdioCollector {
            onStreamFinished: {
                const out = [];
                let active = "";
                for (const ln of this.text.split("\n")) {
                    if (ln.startsWith("@")) active = ln.slice(1);
                    else if (ln !== "") out.push(ln);
                }
                root.images = out;
                root.active = active;
                root.loaded = true;
                const i = root.matches.indexOf(active);
                root.current = Math.max(0, i);
                grid.positionViewAtIndex(root.current, GridView.Contain);
            }
        }
    }

    function name(path) { return path.slice(path.lastIndexOf("/") + 1); }
    function stem(path) { const n = root.name(path); const d = n.lastIndexOf("."); return d > 0 ? n.slice(0, d) : n; }

    readonly property var matches: {
        const q = root.query.trim().toLowerCase();
        return q === "" ? root.images : root.images.filter(p => root.name(p).toLowerCase().includes(q));
    }
    onQueryChanged: root.current = 0

    function apply(path) {
        if (!path) return;
        root.active = path;
        Quickshell.execDetached(["misu-wall", "set", path]);
        root.requestClose();
    }
    function move(by) {
        const n = root.matches.length;
        if (n === 0) return;
        root.current = Math.max(0, Math.min(n - 1, root.current + by));
        grid.positionViewAtIndex(root.current, GridView.Contain);
    }

    onOpenChanged: if (root.open) {
        search.reset();
        lister.running = true;
    }

    implicitWidth: 440 * root.s
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
            title: "WALLPAPER"
            note: root.matches.length + (root.matches.length === 1 ? " image" : " images")
        }
        PopoutSearch {
            id: search
            s: root.s
            placeholder: "Search wallpapers…"
            step: root.cols
            sideways: true
            onTextChanged: root.query = search.text
            onAccepted: root.apply(root.matches[root.current])
            onMove: by => root.move(by)
        }

        // the grid: as many rows as it has, up to $rows
        GridView {
            id: grid
            width: parent.width + root.gap
            height: Math.max(1, Math.min(root.rows, Math.ceil(root.matches.length / root.cols))) * (root.tileH + root.gap)
            clip: true
            cellWidth: root.tileW + root.gap
            cellHeight: root.tileH + root.gap
            model: root.matches
            currentIndex: root.current
            boundsBehavior: Flickable.StopAtBounds
            highlightFollowsCurrentItem: false
            Behavior on height { NumberAnimation { duration: Motion.dur(120); easing.type: Easing.OutCubic } }

            delegate: Item {
                id: tile
                required property string modelData
                required property int index
                readonly property bool selected: tile.index === root.current
                readonly property bool isActive: tile.modelData === root.active

                width: root.tileW
                height: root.tileH

                Rectangle {
                    id: frame
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    height: root.tileH - 22 * root.s
                    color: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.04)
                    Image {
                        anchors.fill: parent
                        source: "file://" + tile.modelData
                        sourceSize.width: 320
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                    }
                    // selection: an ink outline; hover: a lighter one
                    Rectangle {
                        anchors.fill: parent
                        color: "transparent"
                        border.width: tile.selected ? 2 * root.s : Theme.borderWidth
                        border.color: tile.selected ? root.ink
                            : hover.hovered ? Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.4) : root.line
                    }
                    // the wallpaper in use: a seal-red check in the corner
                    Rectangle {
                        visible: tile.isActive
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 5 * root.s
                        width: 18 * root.s
                        height: width
                        color: Theme.vermLit
                        MaterialIcon {
                            anchors.centerIn: parent
                            name: "check"
                            size: 13 * root.s
                            color: Theme.ink(Theme.vermLit)
                        }
                    }
                }
                Text {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    height: 18 * root.s
                    verticalAlignment: Text.AlignVCenter
                    text: root.stem(tile.modelData)
                    color: tile.selected ? root.ink : root.inkDim
                    font.family: Theme.fontPrimary
                    font.pixelSize: Theme.h6 * root.s
                    font.weight: tile.selected ? Font.Medium : Font.Normal
                    elide: Text.ElideMiddle
                }
                HoverHandler { id: hover }
                TapHandler { onTapped: root.apply(tile.modelData) }
            }

            Text {
                anchors.centerIn: parent
                width: parent.width - root.gap * 4
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                visible: root.matches.length === 0
                text: !root.loaded ? "Loading…"
                    : root.query.trim() === "" ? "No images in " + root.dir.replace(Quickshell.env("HOME"), "~")
                    : "No wallpapers match “" + root.query.trim() + "”"
                color: root.inkDim
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h5 * root.s
            }
        }

        Rectangle { width: parent.width; height: 1; color: root.line }
        PopoutKeyHints {
            s: root.s
            keys: [["↵", "set"], ["icon:open_with", "move"], ["esc", "close"]]
        }
    }
}
