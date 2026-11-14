pragma ComponentBehavior: Bound

import QtQuick
import "../../popouts"
import "../.."
import qs.services

// Subtitles card, flat like the network and Bluetooth cards: a labelled row
// each for what misu-dictate listens to (the computer's audio, the
// microphone, or both) and the language spoken (the subtitles are only right
// in the one picked), a hairline, then one row that starts it (Super+Shift+Z
// does too). While it runs, the settings give way to its state and the row
// stops it; the text is saved to ~/Documents/Dictations. Never grabs the
// keyboard.
Item {
    id: root

    property real s: 1
    property bool open: false

    readonly property real pad: 11 * root.s
    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property color inkDim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)
    readonly property color line: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.14)

    readonly property var sources: [
        { id: "computer", label: "Computer" },
        { id: "mic", label: "Mic" },
        { id: "both", label: "Both" }
    ]
    readonly property var languages: [
        { id: "en", label: "EN" },
        { id: "es", label: "ES" }
    ]
    function labelOf(list, id) {
        const o = list.find(x => x.id === id);
        return o ? o.label : id;
    }

    implicitWidth: 260 * root.s
    implicitHeight: content.implicitHeight + root.pad * 2

    PopoutCard { anchors.fill: parent }

    Column {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: root.pad
        spacing: 4 * root.s

        PopoutTitle {
            text: "SUBTITLES"
            s: root.s
            bottomPadding: 4 * root.s
        }

        PopoutSetting {
            width: parent.width
            s: root.s
            visible: !Dictation.active
            name: "Listen to"
            options: root.sources
            value: Dictation.source
            onPicked: id => Dictation.setSource(id)
        }
        PopoutSetting {
            width: parent.width
            s: root.s
            visible: !Dictation.active
            name: "Language"
            options: root.languages
            value: Dictation.language
            onPicked: id => Dictation.setLanguage(id)
        }

        // running: what it's doing, in place of the settings
        Item {
            visible: Dictation.active
            width: parent.width
            height: 28 * root.s
            Rectangle {
                id: dot
                anchors.left: parent.left
                anchors.leftMargin: 9 * root.s
                anchors.verticalCenter: parent.verticalCenter
                width: 6 * root.s
                height: width
                radius: width / 2
                color: Theme.error
                SequentialAnimation on opacity {
                    running: Dictation.active
                    loops: Animation.Infinite
                    NumberAnimation { to: 0.3; duration: 700 }
                    NumberAnimation { to: 1; duration: 700 }
                }
            }
            Text {
                anchors.left: dot.right
                anchors.leftMargin: 8 * root.s
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                text: "%1 · %2 · %3"
                    .arg(Dictation.phase === "loading" ? "Loading…" : "Listening")
                    .arg(root.labelOf(root.sources, Dictation.source))
                    .arg(root.labelOf(root.languages, Dictation.language))
                color: root.ink
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h5 * root.s
                elide: Text.ElideRight
            }
        }

        Rectangle {
            width: parent.width
            height: 1
            color: root.line
        }

        // start / stop, with the shortcut that does the same
        Item {
            width: parent.width
            height: 30 * root.s
            Rectangle {
                anchors.fill: parent
                radius: 3 * root.s
                color: goTap.pressed ? Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.16)
                    : (goHover.hovered ? Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.08) : "transparent")
                Behavior on color { ColorAnimation { duration: Motion.fast } }
            }
            MaterialIcon {
                id: goIcon
                anchors.left: parent.left
                anchors.leftMargin: 7 * root.s
                anchors.verticalCenter: parent.verticalCenter
                size: 16 * root.s
                name: Dictation.active ? "stop" : "play_arrow"
                color: root.ink
            }
            Text {
                anchors.left: goIcon.right
                anchors.leftMargin: 7 * root.s
                anchors.verticalCenter: parent.verticalCenter
                text: Dictation.active ? "Stop and save" : "Start subtitles"
                color: root.ink
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h5 * root.s
            }
            Text {
                anchors.right: parent.right
                anchors.rightMargin: 8 * root.s
                anchors.verticalCenter: parent.verticalCenter
                text: "⇧ Super Z"
                color: root.inkDim
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h6 * root.s
            }
            HoverHandler { id: goHover; cursorShape: Qt.PointingHandCursor }
            MouseArea { id: goTap; anchors.fill: parent; onClicked: Dictation.toggle() }
        }
    }
}
