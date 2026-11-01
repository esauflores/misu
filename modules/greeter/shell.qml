import QtQuick
import Quickshell
import Quickshell.Services.Greetd

// Misu's login screen: the wallpaper, a username and a password, nothing else.
// Runs as the greeter user under Cage; install.sh fills in @USER@ (the default
// username) and @SESSION@ (the command the session starts with).
ShellRoot {
    id: root

    readonly property color ink: "#e3e1e9"
    readonly property color inkDim: Qt.rgba(227 / 255, 225 / 255, 233 / 255, 0.4)
    readonly property color fill: Qt.rgba(10 / 255, 11 / 255, 16 / 255, 0.62)
    readonly property color line: Qt.rgba(1, 1, 1, 0.07)
    readonly property color accent: "#c1c5dd"
    readonly property color danger: "#f6b4ab"

    property string message: ""
    property bool busy: false

    function login() {
        if (root.busy || userField.text === "" || passField.text === "") return;
        root.busy = true;
        root.message = "";
        Greetd.createSession(userField.text);
    }

    // greetd has already closed the session when it reports a failure.
    function fail(text) {
        root.message = text;
        root.busy = false;
        passField.text = "";
        passField.focusInput();
    }

    Connections {
        target: Greetd
        function onAuthMessage(message, error, responseRequired, echoResponse) {
            if (responseRequired) Greetd.respond(passField.text);
            else root.message = message.trim();
        }
        function onAuthFailure(message) { root.fail(root.message || "Wrong password"); }
        function onError(error) { root.fail(error); }
        function onReadyToLaunch() { Greetd.launch(["@SESSION@"]); }
    }

    component Field: Rectangle {
        id: field
        property alias text: input.text
        property alias echoMode: input.echoMode
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
            color: root.ink
            selectionColor: root.accent
            selectedTextColor: "#121318"
            passwordCharacter: "•"
            font { family: "Space Grotesk"; pixelSize: 16; letterSpacing: input.echoMode === TextInput.Password ? 2 : 0 }
            enabled: !root.busy
            onAccepted: field.accepted()
        }
        Text {
            anchors.centerIn: parent
            visible: input.text === "" && !input.activeFocus
            text: field.placeholder
            color: root.inkDim
            font { family: "Space Grotesk"; pixelSize: 16 }
        }
    }

    FloatingWindow {
        color: "black"

        Image {
            anchors.fill: parent
            source: "file:///usr/share/backgrounds/rice/black-hole.webp"
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
        }
        // Dim the wallpaper so the fields read on its bright ring.
        Rectangle { anchors.fill: parent; color: Qt.rgba(0, 0, 0, 0.35) }

        Column {
            anchors.centerIn: parent
            spacing: 10

            Field {
                id: userField
                placeholder: "Username"
                text: "@USER@"
                onAccepted: passField.focusInput()
            }

            Field {
                id: passField
                placeholder: "Password"
                echoMode: TextInput.Password
                onAccepted: root.login()
                Component.onCompleted: focusInput()
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
                font { family: "Space Grotesk"; pixelSize: 13 }
            }
        }
    }
}
