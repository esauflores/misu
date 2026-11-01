pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.services
import "../.."

// Ito-style feedback: a slim vertical thread beside Misu's right rail.
PanelWindow {
    id: root

    required property var modelData
    screen: modelData
    visible: Feedback.visible && (Wm.focusedOutput === "" || Wm.focusedOutput === modelData.name)

    anchors { top: true; bottom: true; right: true }
    margins.right: Config.rail.size + Config.frameThickness + 4
    implicitWidth: 48
    implicitHeight: 0
    color: "transparent"
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "misu-osd"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    mask: Region {}

    Rectangle {
        anchors.centerIn: parent
        width: 44
        height: 220
        radius: 0
        color: Theme.surface
        opacity: Theme.windowOpacity
        border.width: Theme.borderWidth
        border.color: Theme.outlineVariant

        Column {
            anchors.centerIn: parent
            width: parent.width - 16
            spacing: 9

            Item {
                width: parent.width
                height: 20
                MaterialIcon {
                    anchors.centerIn: parent
                    name: Feedback.kind === "brightness"
                        ? (Feedback.level < 0.35 ? "brightness_low" : Feedback.level < 0.7 ? "brightness_medium" : "brightness_high")
                        : Feedback.kind === "microphone"
                            ? (Feedback.muted ? "mic_off" : "mic")
                            : (Feedback.muted || Feedback.level <= 0 ? "volume_off" : Feedback.level > 0.65 ? "volume_up" : "volume_down")
                    size: 20
                    color: Theme.surfaceText
                }
            }

            Item {
                x: (parent.width - width) / 2
                width: 4
                height: 132

                Rectangle {
                    anchors.fill: parent
                    radius: 0
                    color: Theme.outlineVariant
                }
                Rectangle {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    height: parent.height * (Feedback.muted ? 0 : Feedback.level)
                    radius: 0
                    color: Theme.primary
                }
            }

            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: Feedback.muted && Feedback.kind !== "brightness"
                    ? "OFF" : Math.round(Feedback.level * 100) + "%"
                color: Theme.surfaceText
                font.family: Theme.mono
                font.pixelSize: 9
            }
        }
    }
}
