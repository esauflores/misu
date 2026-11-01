pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.services
import "../rail"
import ".."

// One monitor's bar: an exclusive-zone surface that reserves the rail's
// width on the right edge, plus a full-screen transparent overlay holding the
// painted frame, the rail and this monitor's rail cards. Input reaches the
// overlay only over the rail, its 1px reveal strip and an open card; the rest
// clicks through. The overlay hides while a window is fullscreen here.
Scope {
    id: root

    // The screen, injected by the per-screen Variants in shell.qml.
    property var modelData: null

    readonly property var state: root.modelData ? ShellState.forScreen(root.modelData) : null
    readonly property bool revealed: (root.state ? root.state.barRevealed : true) && Config.rail.reveal !== false
    readonly property bool hasWidgets: ["top", "center", "bottom"].some(z => (Config.rail[z] || []).length > 0)
    // The rail's width plus the frame lip while revealed, a 1px lip while
    // hidden (so it still reveals on hover), nothing for an empty rail.
    readonly property real reserve: !root.hasWidgets ? 0
        : root.revealed ? Config.rail.size + Config.frameThickness : 1

    FrameEdge { screen: root.modelData; reserve: root.reserve }

    PanelWindow {
        id: overlay

        // Card chrome follows the panel's physical height (1080p = 1) and the
        // accessibility font scale, so cards match the rail on small screens.
        readonly property real s: (root.modelData ? root.modelData.height * root.modelData.devicePixelRatio / 1080 : 1)
            * Config.fontScale

        readonly property bool monFullscreen: Wm.outputHasFullscreen(root.modelData ? root.modelData.name : "")
        onMonFullscreenChanged: if (monFullscreen) cards.closeAll()

        screen: root.modelData
        color: "transparent"
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.layer: WlrLayer.Top
        WlrLayershell.keyboardFocus: cards.keyboardMode === "ondemand" ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.None
        WlrLayershell.namespace: "misu"
        anchors { top: true; left: true; right: true; bottom: true }

        // While a card shows, the whole surface takes input so a click outside
        // can close it.
        mask: overlay.monFullscreen ? hiddenRegion
            : cards.anyVisible ? fullRegion
            : railRegion

        Region { id: hiddenRegion }
        Region { id: fullRegion; width: overlay.width; height: overlay.height }
        Region {
            id: railRegion
            Region { x: overlay.width - Math.max(1, root.reserve); y: 0; width: root.hasWidgets ? Math.max(1, root.reserve) : 0; height: overlay.height }
        }

        // A press outside the rail and the open card closes it.
        MouseArea {
            anchors.fill: parent
            enabled: cards.anyOpen
            acceptedButtons: Qt.AllButtons
            onPressed: mouse => {
                const m = cards.mask;
                const inside = (x, y, w, h) => w > 0 && mouse.x >= x && mouse.x < x + w && mouse.y >= y && mouse.y < y + h;
                if (inside(m.tx, m.ty, m.tw, m.th) || inside(m.bx, m.by, m.bw, m.bh)
                        || mouse.x >= overlay.width - root.reserve)
                    return;
                cards.closeAll();
            }
        }

        FocusScope {
            anchors.fill: parent
            focus: cards.anyOpen
            visible: !overlay.monFullscreen
            Keys.onEscapePressed: cards.closeAll()

            // The painted band: the rail's surface with the screen-side hole's
            // corners arced at frameCorner.
            FrameChrome {
                anchors.fill: parent
                reserveTop: 0
                reserveBottom: 0
                reserveLeft: 0
                reserveRight: root.reserve
                holeRadius: Config.frameCorner
                surface: Theme.surface
                opacity: Theme.windowOpacity
            }

            Rail {
                z: 1
                revealed: root.revealed
                onMenuRequested: (id, ownerRect) => cards.openSurface(id, ownerRect)
            }

            // Above the rail: a card grows out of it and must cover it.
            CardManager {
                id: cards
                z: 2
                monitorName: root.modelData ? root.modelData.name : ""
                scale: overlay.s
                group: QtObject { property color color: Theme.surface }
                railClearance: Math.max(root.reserve, Config.frameThickness)
                active: !overlay.monFullscreen
                Component.onCompleted: ShellState.registerMenus(cards.monitorName, cards)
                Component.onDestruction: ShellState.registerMenus(cards.monitorName, null)
            }
        }
    }
}
