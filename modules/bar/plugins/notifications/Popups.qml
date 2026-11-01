pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Services.Notifications
import qs.services
import "../.."

// The on-screen notification stack: newest on top, in the top-right corner
// of the focused output, clear of the right rail. Click runs the default
// action (or dismisses); the close button dismisses; each card leaves the
// stack after its timeout but stays in the history card.
PanelWindow {
    id: root

    required property var modelData
    screen: root.modelData
    readonly property bool shown: Notifs.popups.length > 0
        && (Wm.focusedOutput === "" || Wm.focusedOutput === root.modelData.name)
    visible: shown

    readonly property real railReserve: Config.rail.size + Config.frameThickness

    anchors { top: true; right: true }
    margins { top: 12; right: 12 + root.railReserve }
    implicitWidth: 360
    implicitHeight: Math.max(1, stack.implicitHeight)
    color: "transparent"
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "misu-notifications"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None

    function defaultAction(n) {
        for (const a of (n.actions || []))
            if (a && a.identifier === "default") return a;
        return null;
    }

    Column {
        id: stack
        width: parent.width
        spacing: 8

        Repeater {
            model: Notifs.popups

            delegate: Rectangle {
                id: card
                required property var modelData
                readonly property var n: modelData
                readonly property bool critical: n.urgency === NotificationUrgency.Critical
                width: stack.width
                implicitHeight: body.implicitHeight + 24
                radius: Theme.radiusWindow
                color: Theme.surface
                opacity: Theme.windowOpacity
                border.width: Theme.borderWidth
                border.color: card.critical ? Theme.error : (hover.hovered ? Theme.primary : Theme.outlineVariant)

                Timer {
                    interval: Notifs.popupTimeout(card.n)
                    running: interval > 0 && !hover.hovered
                    onTriggered: Notifs.dropPopup(card.n)
                }
                HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor }
                TapHandler {
                    onTapped: {
                        const a = root.defaultAction(card.n);
                        if (a) a.invoke();
                        Notifs.dismiss(card.n);
                    }
                }

                Row {
                    id: body
                    x: 12
                    y: 12
                    width: parent.width - 24
                    spacing: 10

                    Image {
                        id: art
                        readonly property string src: card.n.image || (card.n.appIcon ? Quickshell.iconPath(card.n.appIcon, true) : "")
                        visible: src.length > 0
                        width: visible ? 36 : 0
                        height: 36
                        source: src
                        sourceSize.width: 72
                        sourceSize.height: 72
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                    }
                    Column {
                        width: body.width - (art.visible ? art.width + body.spacing : 0) - 20
                        spacing: 3
                        Text {
                            width: parent.width
                            text: card.n.appName || ""
                            visible: text.length > 0
                            color: Theme.surfaceVariantText
                            font.family: Theme.fontPrimary
                            font.pixelSize: 10
                            elide: Text.ElideRight
                        }
                        Text {
                            width: parent.width
                            text: card.n.summary || ""
                            color: Theme.surfaceText
                            font.family: Theme.fontPrimary
                            font.pixelSize: 14
                            font.weight: Font.DemiBold
                            wrapMode: Text.Wrap
                            maximumLineCount: 2
                            elide: Text.ElideRight
                        }
                        Text {
                            width: parent.width
                            text: card.n.body || ""
                            visible: text.length > 0
                            color: Theme.surfaceVariantText
                            font.family: Theme.fontPrimary
                            font.pixelSize: 12
                            textFormat: Text.StyledText
                            wrapMode: Text.Wrap
                            maximumLineCount: 4
                            elide: Text.ElideRight
                        }
                    }
                }

                MaterialIcon {
                    anchors.top: parent.top
                    anchors.right: parent.right
                    anchors.margins: 8
                    name: "close"
                    size: 16
                    color: closeHover.hovered ? Theme.surfaceText : Theme.surfaceVariantText
                    HoverHandler { id: closeHover; cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: Notifs.dismiss(card.n) }
                }
            }
        }
    }
}
