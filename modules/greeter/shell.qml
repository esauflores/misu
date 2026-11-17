import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Services.Greetd

// Misu's login screen: the wallpaper softly blurred, the time and date, and a
// card like the bar's (the 力 head, hairline border, key hints) with the user's
// initial, the username and a password field. A wrong password shakes the
// field; a right one fades to black before the session starts, so wm's first
// frame follows without the console in between. Runs as the greeter user
// under Cage; install.sh fills in @USER@ (the default username) and @SESSION@
// (the command the session starts with).
ShellRoot {
    id: root

    // Misu's default palette (the bar's, from the black-hole wallpaper): the
    // greeter can't read the user's matugen state.
    readonly property color ink: "#e3e1e9"
    // Secondary text, as light as it can be and still sit back: legible over
    // the blurred wallpaper's bright ring.
    function inkAt(alpha) { return Qt.rgba(227 / 255, 225 / 255, 233 / 255, alpha); }
    readonly property color inkDate: inkAt(0.8)
    readonly property color inkNote: inkAt(0.7)
    readonly property color inkHint: inkAt(0.65)
    readonly property color primary: "#b5c4ff"
    readonly property color primaryLit: Qt.lighter(primary, 1.22)
    readonly property color primaryInk: "#1c2d61"
    readonly property color card: Qt.rgba(18 / 255, 19 / 255, 24 / 255, 0.84)
    readonly property color well: Qt.rgba(0, 0, 0, 0.28)
    readonly property color line: Qt.rgba(1, 1, 1, 0.08)
    readonly property color danger: "#ffb4ab"
    readonly property string sans: "Space Grotesk"
    readonly property string iconFont: "Material Symbols Rounded"

    property string message: ""     // shown under the field
    property bool messageIsError: true
    property string pamError: ""    // PAM's own error, said instead of "Wrong password"
    property bool busy: false
    property bool leaving: false
    // PAM's first question gets the password; a later one (a one-time code, a
    // new password) is shown in the field as its placeholder and answered from it.
    property bool answered: false
    property string prompt: ""

    function login() {
        if (root.busy || passField.text === "") return;
        if (root.prompt !== "") {
            root.busy = true;
            root.prompt = "";
            Greetd.respond(passField.text);
            return;
        }
        if (userField.text === "") return;
        root.busy = true;
        root.message = "";
        root.pamError = "";
        root.answered = false;
        Greetd.createSession(userField.text);
    }

    // greetd has already closed the session when it reports a failure; an
    // error after the fade began takes the screen back from black.
    function fail(text) {
        root.leaving = false;
        root.answered = false;
        root.prompt = "";
        root.message = text;
        root.messageIsError = true;
        root.busy = false;
        passField.echoMode = TextInput.Password;
        passField.text = "";
        passField.forceActiveFocus();
        shake.restart();
    }

    Connections {
        target: Greetd
        function onAuthMessage(message, error, responseRequired, echoResponse) {
            const text = message.trim();
            if (responseRequired && !root.answered) {
                root.answered = true;
                Greetd.respond(passField.text);
            } else if (responseRequired) {
                root.prompt = text || "Answer";
                root.busy = false;
                passField.echoMode = echoResponse ? TextInput.Normal : TextInput.Password;
                passField.text = "";
                passField.forceActiveFocus();
            } else if (text !== "") {
                root.message = text;
                root.messageIsError = error;
                if (error) root.pamError = text;
            }
        }
        function onAuthFailure(message) { root.fail(root.pamError || "Wrong password"); }
        function onError(error) { root.fail(error); }
        function onReadyToLaunch() { root.leaving = true; }
    }

    SystemClock { id: clock; precision: SystemClock.Minutes }

    // A Material Symbols ligature in fixed bounds, so a missing glyph can't
    // push the layout.
    component Icon: Text {
        width: 20
        height: 20
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        color: root.ink
        font { family: root.iconFont; pixelSize: 20 }
    }

    FloatingWindow {
        color: "black"

        Image {
            anchors.fill: parent
            source: "file:///usr/share/backgrounds/rice/black-hole.png"
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            layer.enabled: true
            layer.effect: MultiEffect {
                blurEnabled: true
                blur: 0.6
                blurMax: 48
                brightness: -0.08
                autoPaddingEnabled: false
            }
        }
        // Darker towards the bottom, where the card sits.
        Rectangle {
            anchors.fill: parent
            gradient: Gradient {
                GradientStop { position: 0; color: Qt.rgba(0, 0, 0, 0.2) }
                GradientStop { position: 1; color: Qt.rgba(0, 0, 0, 0.55) }
            }
        }

        Column {
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottom: loginCard.top
            anchors.bottomMargin: 40
            spacing: 2

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: Qt.formatDateTime(clock.date, "HH:mm")
                color: root.ink
                font { family: root.sans; pixelSize: 96; weight: Font.Light; letterSpacing: -2 }
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: Qt.formatDateTime(clock.date, "dddd, d MMMM")
                color: root.inkDate
                font { family: root.sans; pixelSize: 17; letterSpacing: 0.5 }
            }
        }

        Rectangle {
            id: loginCard
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.verticalCenter: parent.verticalCenter
            anchors.verticalCenterOffset: 70
            width: 340
            height: body.implicitHeight + 48
            radius: 16
            color: root.card
            border { width: 1; color: root.line }

            Column {
                id: body
                anchors { left: parent.left; right: parent.right; top: parent.top; margins: 24 }
                spacing: 16

                // The head: the 力 seal and MISU, the bar cards' title.
                Item {
                    width: parent.width
                    height: 18
                    Row {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 6
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "力"
                            color: root.primaryLit
                            font { family: "Noto Sans CJK JP"; pixelSize: 13; weight: Font.Medium }
                        }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "MISU"
                            color: root.primaryLit
                            font { family: root.sans; pixelSize: 12; weight: Font.Medium; letterSpacing: 1.6 }
                        }
                    }
                    Text {
                        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                        text: "sign in"
                        color: root.inkNote
                        font { family: root.sans; pixelSize: 11; letterSpacing: 0.6 }
                    }
                }

                // Who: the initial in a ring, the username under it (editable).
                Column {
                    width: parent.width
                    spacing: 10

                    Rectangle {
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: 64
                        height: 64
                        radius: 32
                        color: Qt.rgba(root.primary.r, root.primary.g, root.primary.b, 0.13)
                        border { width: 1; color: Qt.rgba(root.primary.r, root.primary.g, root.primary.b, 0.4) }
                        Text {
                            anchors.centerIn: parent
                            text: userField.text.charAt(0).toUpperCase() || "?"
                            color: root.primaryLit
                            font { family: root.sans; pixelSize: 26; weight: Font.Medium }
                        }
                    }
                    Item {
                        width: parent.width
                        height: 26
                        TextInput {
                            id: userField
                            anchors.fill: parent
                            horizontalAlignment: TextInput.AlignHCenter
                            verticalAlignment: TextInput.AlignVCenter
                            text: "@USER@"
                            color: root.ink
                            selectionColor: root.primary
                            selectedTextColor: root.primaryInk
                            font { family: root.sans; pixelSize: 18; weight: Font.Medium }
                            enabled: !root.busy
                            KeyNavigation.tab: passField
                            KeyNavigation.backtab: passField
                            onAccepted: passField.forceActiveFocus()
                        }
                        Text {
                            anchors.centerIn: parent
                            visible: userField.text === ""
                            text: "Username"
                            color: root.inkHint
                            font { family: root.sans; pixelSize: 18 }
                        }
                        // Underlined while it's being edited.
                        Rectangle {
                            anchors { bottom: parent.bottom; horizontalCenter: parent.horizontalCenter }
                            width: Math.max(userField.contentWidth, 60) + 16
                            height: 1
                            color: root.primary
                            opacity: userField.activeFocus ? 0.6 : 0
                            Behavior on opacity { NumberAnimation { duration: 150 } }
                        }
                    }
                }

                // The password: a lock, the dots, and a button that signs in
                // (a spinner while greetd checks).
                Rectangle {
                    id: passBox
                    width: parent.width
                    height: 46
                    radius: 10
                    color: root.well
                    border { width: 1; color: passField.activeFocus ? Qt.rgba(root.primary.r, root.primary.g, root.primary.b, 0.8) : root.line }
                    Behavior on border.color { ColorAnimation { duration: 150 } }

                    transform: Translate { id: nudge }
                    SequentialAnimation {
                        id: shake
                        loops: 1
                        NumberAnimation { target: nudge; property: "x"; to: -10; duration: 50 }
                        NumberAnimation { target: nudge; property: "x"; to: 8; duration: 70 }
                        NumberAnimation { target: nudge; property: "x"; to: -5; duration: 60 }
                        NumberAnimation { target: nudge; property: "x"; to: 0; duration: 60 }
                    }

                    Icon {
                        id: lockIcon
                        anchors { left: parent.left; leftMargin: 14; verticalCenter: parent.verticalCenter }
                        text: "lock"
                        color: root.inkHint
                    }
                    TextInput {
                        id: passField
                        anchors { left: lockIcon.right; right: go.left; leftMargin: 10; rightMargin: 10; top: parent.top; bottom: parent.bottom }
                        verticalAlignment: TextInput.AlignVCenter
                        echoMode: TextInput.Password
                        passwordCharacter: "•"
                        color: root.ink
                        selectionColor: root.primary
                        selectedTextColor: root.primaryInk
                        font { family: root.sans; pixelSize: 16; letterSpacing: 2 }
                        enabled: !root.busy
                        focus: true
                        KeyNavigation.tab: userField
                        KeyNavigation.backtab: userField
                        onAccepted: root.login()
                        onTextChanged: if (text !== "") root.message = ""
                    }
                    Text {
                        anchors { left: passField.left; verticalCenter: parent.verticalCenter }
                        anchors.right: passField.right
                        visible: passField.text === ""
                        text: root.prompt || "Password"
                        elide: Text.ElideRight
                        color: root.inkHint
                        font { family: root.sans; pixelSize: 16 }
                    }
                    Rectangle {
                        id: go
                        anchors { right: parent.right; rightMargin: 7; verticalCenter: parent.verticalCenter }
                        width: 32
                        height: 32
                        radius: 16
                        readonly property bool ready: passField.text !== "" && (userField.text !== "" || root.prompt !== "")
                        color: go.ready || root.busy ? root.primary : Qt.rgba(1, 1, 1, 0.06)
                        Behavior on color { ColorAnimation { duration: 150 } }
                        Icon {
                            anchors.centerIn: parent
                            text: root.busy ? "progress_activity" : "arrow_forward"
                            color: go.ready || root.busy ? root.primaryInk : root.inkHint
                            font.pixelSize: 18
                            RotationAnimation on rotation {
                                running: root.busy
                                from: 0; to: 360; duration: 900; loops: Animation.Infinite
                                onRunningChanged: if (!running) parent.rotation = 0
                            }
                        }
                        MouseArea {
                            anchors.fill: parent
                            enabled: go.ready && !root.busy
                            cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                            onClicked: root.login()
                        }
                    }
                }

                Text {
                    width: parent.width
                    // Room for two lines, kept, so the card doesn't move.
                    height: 36
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                    wrapMode: Text.WordWrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                    text: root.message
                    color: root.messageIsError ? root.danger : root.inkNote
                    opacity: root.message === "" ? 0 : 1
                    Behavior on opacity { NumberAnimation { duration: 150 } }
                    font { family: root.sans; pixelSize: 13 }
                }

                Rectangle { width: parent.width; height: 1; color: root.line }

                // Key hints, as on the bar's cards.
                Row {
                    anchors.horizontalCenter: parent.horizontalCenter
                    spacing: 18
                    Repeater {
                        model: [["↵", "sign in"], ["tab", "switch"]]
                        Row {
                            id: hint
                            required property var modelData
                            spacing: 6
                            Rectangle {
                                anchors.verticalCenter: parent.verticalCenter
                                width: Math.max(keyText.implicitWidth + 10, 20)
                                height: 18
                                radius: 4
                                color: "transparent"
                                border { width: 1; color: root.line }
                                Text {
                                    id: keyText
                                    anchors.centerIn: parent
                                    text: hint.modelData[0]
                                    color: root.inkNote
                                    font { family: root.sans; pixelSize: 12 }
                                }
                            }
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                text: hint.modelData[1]
                                color: root.inkNote
                                font { family: root.sans; pixelSize: 12 }
                            }
                        }
                    }
                }
            }
        }

        // Signed in: fade to black, then hand over to the session.
        Rectangle {
            anchors.fill: parent
            color: "black"
            opacity: root.leaving ? 1 : 0
            Behavior on opacity { NumberAnimation { duration: 280; easing.type: Easing.OutCubic } }
        }
        Timer {
            interval: 320
            running: root.leaving
            onTriggered: if (root.leaving) Greetd.launch(["@SESSION@"])
        }
    }
}
