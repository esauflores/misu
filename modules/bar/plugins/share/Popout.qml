pragma ComponentBehavior: Bound

import QtQuick
import "../../popouts"
import "../.."
import qs.services

// Screen share card: the portal's sources (ShareRequest), screens and windows,
// each with a thumbnail, or its icon if there is none. No rail button: it opens when an app asks to share. Up/Down (Tab, J/K) to
// move, Enter or a click to share; closing it any other way declines the share.
// Takes the keyboard while open (keyboard: "exclusive" in index.js).
Item {
    id: root

    property real s: 1
    property bool open: false
    signal requestClose()

    readonly property real pad: 12 * root.s
    readonly property real rowH: 66 * root.s
    readonly property int maxRows: 6     // with the title and hints, within the card's height cap
    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property color inkDim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)
    readonly property color line: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.14)
    readonly property color selBg: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.08)
    readonly property color hoverBg: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.04)

    property int current: 0

    function share(i) {
        const option = ShareRequest.options[i];
        if (!option) return;
        ShareRequest.pick(option);
        root.requestClose();
    }
    function move(by) {
        const n = ShareRequest.options.length;
        if (n) root.current = (root.current + by + n) % n;
    }

    onOpenChanged: if (root.open) {
        root.current = 0;
        Qt.callLater(() => keys.forceActiveFocus());
    }
    Connections {
        target: ShareRequest
        function onOptionsChanged() { root.current = 0; }
    }

    implicitWidth: 360 * root.s
    implicitHeight: content.implicitHeight + root.pad * 2

    PopoutCard { anchors.fill: parent }

    Item {
        id: keys
        focus: true
        Keys.onUpPressed: root.move(-1)
        Keys.onDownPressed: root.move(1)
        Keys.onTabPressed: root.move(1)
        Keys.onBacktabPressed: root.move(-1)
        Keys.onReturnPressed: root.share(root.current)
        Keys.onEnterPressed: root.share(root.current)
        Keys.onPressed: event => {
            if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier)) return;
            if (event.key === Qt.Key_J) { root.move(1); event.accepted = true; }
            else if (event.key === Qt.Key_K) { root.move(-1); event.accepted = true; }
        }
    }

    Column {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: root.pad
        spacing: 10 * root.s

        PopoutTitle {
            text: "SHARE SCREEN"
            s: root.s
        }

        ListView {
            width: parent.width
            height: Math.min(count, root.maxRows) * root.rowH
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            model: ShareRequest.options
            currentIndex: root.current
            highlightFollowsCurrentItem: false
            onCurrentIndexChanged: positionViewAtIndex(currentIndex, ListView.Contain)

            delegate: Item {
                id: row
                required property string modelData
                required property int index
                readonly property bool selected: row.index === root.current
                readonly property bool isScreen: row.modelData.startsWith("Monitor: ")

                width: ListView.view.width
                height: root.rowH

                Rectangle {
                    anchors.fill: parent
                    color: row.selected ? root.selBg : hover.hovered ? root.hoverBg : "transparent"
                }
                Rectangle {
                    visible: row.selected
                    anchors.left: parent.left
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    width: 2 * root.s
                    color: root.ink
                }
                Rectangle {
                    id: frame
                    anchors.left: parent.left
                    anchors.leftMargin: 10 * root.s
                    anchors.verticalCenter: parent.verticalCenter
                    width: 96 * root.s
                    height: 54 * root.s
                    color: "transparent"
                    border.width: 1
                    border.color: row.selected ? root.ink : root.line

                    MaterialIcon {
                        anchors.centerIn: parent
                        visible: thumb.status !== Image.Ready
                        name: row.isScreen ? "monitor" : "web_asset"
                        size: 18 * root.s
                        color: row.selected ? root.ink : root.inkDim
                    }
                    Image {
                        id: thumb
                        anchors.fill: parent
                        anchors.margins: 1
                        source: "file://" + ShareRequest.thumbDir + "/thumb-" + row.index + ".jpg"
                        fillMode: Image.PreserveAspectFit
                        asynchronous: true
                        cache: false
                    }
                }
                Text {
                    anchors.left: frame.right
                    anchors.right: parent.right
                    anchors.leftMargin: 12 * root.s
                    anchors.rightMargin: 10 * root.s
                    anchors.verticalCenter: parent.verticalCenter
                    // less the "Window: " and the portal's "(id)" at the end
                    text: row.modelData.replace(/^(Monitor|Window): /, "").replace(/ \([^()]*\)$/, "")
                    color: row.selected ? root.ink : Theme.surfaceText
                    font.family: Theme.fontPrimary
                    font.pixelSize: Theme.h4 * root.s
                    font.weight: row.selected ? Font.Medium : Font.Normal
                    wrapMode: Text.Wrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                }
                HoverHandler { id: hover }
                TapHandler { onTapped: root.share(row.index) }
            }
        }

        Rectangle { width: parent.width; height: 1; color: root.line }
        PopoutKeyHints {
            s: root.s
            keys: [["↵", "share"], ["↑↓", "move"], ["esc", "cancel"]]
        }
    }
}
