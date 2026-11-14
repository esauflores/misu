pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Services.Pam
import qs.services

// Misu's lock screen, drawn like the greeter (modules/greeter/shell.qml): the
// current wallpaper dimmed, the username and a password field as pills, an
// error line below. Locks every screen through ext-session-lock and checks the
// password through PAM. `qs -c misu ipc call lock lock` locks (misu-lock does
// that); only the right password unlocks.
Scope {
    id: root

    // the greeter's palette, kept in step with it
    readonly property color ink: "#e3e1e9"
    readonly property color inkDim: Qt.rgba(227 / 255, 225 / 255, 233 / 255, 0.4)
    readonly property color fill: Qt.rgba(10 / 255, 11 / 255, 16 / 255, 0.62)
    readonly property color line: Qt.rgba(1, 1, 1, 0.07)
    readonly property color accent: "#c1c5dd"
    readonly property color danger: "#f6b4ab"

    readonly property string user: Quickshell.env("USER") || ""
    readonly property bool locked: sessionLock.locked

    property string message: ""
    property bool busy: false
    property string pending: ""
    // Bumped on a failed try, so every screen's field clears and refocuses.
    property int resets: 0

    function lock() {
        if (sessionLock.locked) return;
        root.message = "";
        root.busy = false;
        root.resets++;
        sessionLock.locked = true;
    }
    function tryUnlock(password) {
        if (root.busy || password === "") return;
        root.busy = true;
        root.message = "";
        root.pending = password;
        if (!pam.start()) root.fail("Couldn't start authentication");
    }
    function fail(text) {
        root.pending = "";
        root.message = text;
        root.busy = false;
        root.resets++;
    }

    // the current wallpaper, as misu-wall records it
    property string wallpaper: "/usr/share/backgrounds/rice/black-hole.png"
    FileView {
        id: wallFile
        path: (Quickshell.env("XDG_STATE_HOME") || (Quickshell.env("HOME") + "/.local/state")) + "/rice/wallpaper"
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: {
            const p = wallFile.text().trim();
            if (p !== "") root.wallpaper = p;
        }
    }

    PamContext {
        id: pam
        user: root.user
        onPamMessage: {
            if (pam.responseRequired) {
                pam.respond(root.pending);
                root.pending = "";
            } else if (pam.messageIsError) {
                root.message = pam.message.trim();
            }
        }
        onCompleted: result => {
            if (result === PamResult.Success) {
                root.pending = "";
                root.busy = false;
                root.message = "";
                sessionLock.locked = false;
            } else {
                root.fail(result === PamResult.MaxTries ? "Too many tries, wait a moment" : "Wrong password");
            }
        }
        onError: error => root.fail("Authentication error")
    }

    component Pill: Rectangle {
        id: pill
        property alias text: input.text
        property alias echoMode: input.echoMode
        property alias readOnly: input.readOnly
        property string placeholder
        signal accepted
        function focusInput() { input.forceActiveFocus(); }

        width: 280
        height: 44
        radius: 22
        color: root.fill
        border { width: 1; color: input.activeFocus ? Qt.rgba(193 / 255, 197 / 255, 221 / 255, 0.5) : root.line }
        Behavior on border.color { ColorAnimation { duration: 150 } }

        TextInput {
            id: input
            anchors { fill: parent; leftMargin: 20; rightMargin: 20 }
            verticalAlignment: TextInput.AlignVCenter
            horizontalAlignment: TextInput.AlignHCenter
            color: input.readOnly ? root.inkDim : root.ink
            selectionColor: root.accent
            selectedTextColor: "#121318"
            passwordCharacter: "•"
            font { family: "Space Grotesk"; pixelSize: Theme.h3; letterSpacing: input.echoMode === TextInput.Password ? 2 : 0 }
            enabled: !root.busy
            activeFocusOnPress: !input.readOnly
            onAccepted: pill.accepted()
        }
        Text {
            anchors.centerIn: parent
            visible: input.text === "" && !input.activeFocus
            text: pill.placeholder
            color: root.inkDim
            font { family: "Space Grotesk"; pixelSize: Theme.h3 }
        }
    }

    WlSessionLock {
        id: sessionLock

        WlSessionLockSurface {
            id: surface
            color: "black"

            Image {
                anchors.fill: parent
                source: "file://" + root.wallpaper
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
            }
            // Dim the wallpaper so the fields read on its bright ring.
            Rectangle { anchors.fill: parent; color: Qt.rgba(0, 0, 0, 0.35) }

            Column {
                anchors.centerIn: parent
                spacing: 10

                Pill {
                    text: root.user
                    readOnly: true
                }

                Pill {
                    id: passField
                    placeholder: "Password"
                    echoMode: TextInput.Password
                    onAccepted: root.tryUnlock(passField.text)
                    Component.onCompleted: passField.focusInput()
                    Connections {
                        target: root
                        function onResetsChanged() {
                            passField.text = "";
                            passField.focusInput();
                        }
                    }
                }

                Text {
                    width: 280
                    topPadding: 6
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                    text: root.message
                    color: root.danger
                    opacity: root.message === "" ? 0 : 1
                    Behavior on opacity { NumberAnimation { duration: 150 } }
                    font { family: "Space Grotesk"; pixelSize: Theme.h4 }
                }
            }
        }
    }

    IpcHandler {
        target: "lock"
        function lock(): void { root.lock(); }
        function isLocked(): bool { return root.locked; }
    }
}
