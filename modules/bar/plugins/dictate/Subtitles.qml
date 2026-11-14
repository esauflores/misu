pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.services

// Dictation subtitles (misu-dictate, Super+Shift+Z), styled like a film's: one
// centred line in a fixed-size dark box (the server's 60 characters) at the bottom of the focused screen, filled word by
// word; when it's full the server starts a fresh
// page, and a long silence clears it. Before anything is heard, while saving
// and when saved it shows a dim status instead (state from Dictation).
PanelWindow {
    id: root

    required property var modelData
    screen: modelData

    readonly property bool onScreen: Wm.focusedOutput === "" || Wm.focusedOutput === modelData.name
    visible: onScreen && (Dictation.shown || box.opacity > 0)

    // what's on screen: the live caption, a hint, or the outcome
    readonly property string caption: {
        if (Dictation.phase === "idle")
            return "";
        const words = Dictation.text.trim() === "" ? 0 : Dictation.text.trim().split(/\s+/).length;
        if (Dictation.phase === "done")
            // misu-dictate copies up to 1000 characters (its copy_max)
            return words ? (Dictation.text.length <= 1000 ? "Saved and copied · " : "Saved · ")
                + words + (words === 1 ? " word" : " words") : "Nothing heard";
        if (Dictation.lines.length > 0)
            return Dictation.lines.slice(-1).join("\n");
        return Dictation.phase === "finishing" ? "Saving…"
            : Dictation.phase === "loading" ? "Loading…" : "Listening…";
    }
    readonly property bool hint: Dictation.phase !== "done" && Dictation.lines.length === 0

    anchors { bottom: true; left: true; right: true }
    margins.bottom: 56
    implicitHeight: box.height
    color: "transparent"
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "misu-subtitles"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    mask: Region {}

    Rectangle {
        id: box
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        width: full.width + 32
        height: full.height * label.lineHeight + 14
        radius: 8
        color: Qt.rgba(0, 0, 0, 0.7)
        opacity: Dictation.shown ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: Motion.standard; easing.type: Easing.OutCubic } }

        // a full line: 60 characters of ordinary text
        TextMetrics {
            id: full
            font: label.font
            text: "and we are glad to welcome his gospel. Nor shall we forgo it"
        }

        Text {
            id: label
            anchors.centerIn: parent
            text: root.caption
            horizontalAlignment: Text.AlignHCenter
            lineHeight: 1.15
            color: root.hint ? Qt.rgba(1, 1, 1, 0.6) : "#ffffff"
            font.family: Theme.fontPrimary
            font.pixelSize: Theme.h3   // the rail clock's weight, a bit bigger
            font.weight: Font.DemiBold
        }
    }
}
