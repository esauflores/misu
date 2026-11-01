pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Services.SystemTray
import qs.services

// System tray, from Quickshell's StatusNotifier host. The root self-hides while
// there are no items; a toggle button reveals/collapses the item strip. Left
// click activates the item (or opens its menu when it only has one); right
// click opens the item's own menu next to the rail.
//
// Design: 4px rhythm gaps between cells, hover wash (on-surface 8%),
// press dip (scale 0.88 + opacity 0.72), smooth color behavior. The
// reveal strip sits behind a sumi-edged container (frameBorder outline).
Item {
    id: root

    required property string edge
    required property real scale

    readonly property var items: SystemTray.items ? SystemTray.items.values.filter(item => !!item) : []
    readonly property bool horizontal: edge === "top" || edge === "bottom"
    readonly property real cross: 48 * scale
    property bool revealed: false

    readonly property bool selfShown: items.length > 0
    visible: selfShown
    implicitWidth: selfShown ? (horizontal ? line.implicitWidth : cross) : 0
    implicitHeight: selfShown ? (horizontal ? cross : line.implicitHeight) : 0

    function itemSource(it) {
        return it.icon && String(it.icon).length > 0 ? it.icon : Quickshell.iconPath("application-x-executable");
    }

    // The item's own menu, anchored beside the cell that asked for it.
    QsMenuAnchor {
        id: trayMenu
        anchor.window: root.QsWindow.window
        anchor.edges: root.edge === "right" ? Edges.Left : root.edge === "left" ? Edges.Right
            : root.edge === "top" ? Edges.Bottom : Edges.Top
        anchor.gravity: anchor.edges
        function openFor(it, cell) {
            if (!it.hasMenu) return;
            const p = cell.mapToItem(root.QsWindow.window.contentItem, 0, 0);
            trayMenu.menu = it.menu;
            trayMenu.anchor.rect = Qt.rect(p.x, p.y, cell.width, cell.height);
            trayMenu.open();
        }
    }

    Loader {
        id: line
        anchors.centerIn: parent
        sourceComponent: root.horizontal ? rowComp : colComp
    }

    Component {
        id: rowComp
        Row {
            spacing: 0
            RailButton {
                edge: root.edge
                scale: root.scale
                icon: "widgets"
                onClicked: root.revealed = !root.revealed
            }
            // Sumi-edged reveal strip: clipped container with a subtle outline
            // so the icon strip feels anchored against the bar edge.
            Item {
                clip: true
                height: root.cross
                width: root.revealed ? (strip.implicitWidth + 2) : 0
                Behavior on width {
                    NumberAnimation {
                        duration: Motion.barReveal
                        easing.type: Motion.barRevealCurve
                    }
                }
                Rectangle {
                    anchors.fill: strip
                    anchors.margins: -2 * root.scale
                    radius: 0  // misu: square, not rounded
                    color: "transparent"
                    border.width: root.revealed ? 1 : 0
                    border.color: Qt.rgba(Theme.outline.r, Theme.outline.g, Theme.outline.b, 0.28)
                }
                Row {
                    id: strip
                    anchors.centerIn: parent
                    spacing: 4 * root.scale
                    Repeater { model: root.items; delegate: itemComp }
                }
            }
        }
    }
    Component {
        id: colComp
        Column {
            spacing: 0
            RailButton {
                edge: root.edge
                scale: root.scale
                icon: "widgets"
                onClicked: root.revealed = !root.revealed
            }
            // Sumi-edged reveal strip (vertical).
            Item {
                clip: true
                width: root.cross
                height: root.revealed ? (strip.implicitHeight + 2) : 0
                Behavior on height {
                    NumberAnimation {
                        duration: Motion.barReveal
                        easing.type: Motion.barRevealCurve
                    }
                }
                Rectangle {
                    anchors.fill: strip
                    anchors.margins: -2 * root.scale
                    radius: 0  // misu: square, not rounded
                    color: "transparent"
                    border.width: root.revealed ? 1 : 0
                    border.color: Qt.rgba(Theme.outline.r, Theme.outline.g, Theme.outline.b, 0.28)
                }
                Column {
                    id: strip
                    anchors.centerIn: parent
                    spacing: 4 * root.scale
                    Repeater { model: root.items; delegate: itemComp }
                }
            }
        }
    }

    Component {
        id: itemComp
        Item {
            id: cell
            required property var modelData

            // Along-bar slot: minimum 36px; icon (iconSm=16) + 20px pad = 36px exact.
            // Using iconSm keeps tray icons compact relative to dock/status icons.
            readonly property real iconPx: Theme.iconSm * root.scale
            readonly property real along: 36 * root.scale
            width: root.horizontal ? along : root.cross
            height: root.horizontal ? root.cross : along

            // Smooth hover wash
            Rectangle {
                anchors.fill: parent
                anchors.margins: 2 * root.scale
                radius: 0  // misu: square, not rounded
                color: area.containsMouse
                    ? Qt.rgba(Theme.surfaceText.r, Theme.surfaceText.g, Theme.surfaceText.b,
                              area.pressed ? 0.14 : 0.08)
                    : "transparent"
                Behavior on color {
                    ColorAnimation { duration: Motion.fast; easing.type: Motion.easeStandard }
                }
            }

            Image {
                anchors.centerIn: parent
                width: cell.iconPx
                height: cell.iconPx
                sourceSize.width: width
                sourceSize.height: height
                smooth: true
                asynchronous: true
                source: root.itemSource(cell.modelData)
                // Press dip: slight scale-down on click for tactile feel.
                scale: area.pressed ? 0.82 : 1.0
                Behavior on scale {
                    NumberAnimation { duration: 80; easing.type: Easing.OutCubic }
                }
                opacity: area.pressed ? 0.72 : 1.0
                Behavior on opacity {
                    NumberAnimation { duration: 80; easing.type: Easing.OutCubic }
                }
            }

            MouseArea {
                id: area
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: event => {
                    const it = cell.modelData;
                    if (event.button === Qt.LeftButton && !it.onlyMenu)
                        it.activate();
                    else
                        trayMenu.openFor(it, cell);
                }
            }
        }
    }
}
