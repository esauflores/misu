pragma ComponentBehavior: Bound

import QtQuick
import qs.services
import ".."

// The search field of the list cards (launcher, clipboard, wallpaper): a
// hairline box that brightens while typing, a magnifier, the placeholder and
// a clear button. It keeps the keyboard for the card: Up/Down (Ctrl+P/N or
// K/J) move by `step`, Tab/Shift+Tab by one, PgUp/PgDn by `page` when set,
// Left/Right (Ctrl+H/L) by one while the field is empty when `sideways` is
// set. Any other key goes to `key(event)` first; accept it there to take it.
Rectangle {
    id: field

    property real s: 1
    property string placeholder: ""
    property alias text: input.text
    property int step: 1
    property int page: 0
    property bool sideways: false

    signal move(int by)
    signal accepted()
    signal key(var event)

    function reset() { input.text = ""; Qt.callLater(() => input.forceActiveFocus()); }

    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property color inkDim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)

    width: parent ? parent.width : 0
    height: 36 * field.s
    color: Qt.rgba(field.ink.r, field.ink.g, field.ink.b, 0.04)
    border.width: Theme.borderWidth
    border.color: Qt.rgba(field.ink.r, field.ink.g, field.ink.b, input.activeFocus ? 0.4 : 0.14)
    Behavior on border.color { ColorAnimation { duration: Motion.effects } }

    MaterialIcon {
        id: glass
        anchors.left: parent.left
        anchors.leftMargin: 11 * field.s
        anchors.verticalCenter: parent.verticalCenter
        name: "search"
        size: 16 * field.s
        color: input.text.length ? field.ink : field.inkDim
    }
    TextInput {
        id: input
        anchors.left: glass.right
        anchors.right: clear.left
        anchors.leftMargin: 9 * field.s
        anchors.rightMargin: 6 * field.s
        anchors.verticalCenter: parent.verticalCenter
        color: field.ink
        selectionColor: Qt.rgba(field.ink.r, field.ink.g, field.ink.b, 0.25)
        selectedTextColor: field.ink
        font.family: Theme.fontPrimary
        font.pixelSize: Theme.h4 * field.s
        clip: true
        onAccepted: field.accepted()
        Keys.onTabPressed: field.move(1)
        Keys.onBacktabPressed: field.move(-1)
        Keys.onPressed: event => {
            field.key(event);
            if (event.accepted) return;
            const ctrl = event.modifiers & Qt.ControlModifier;
            const empty = input.text === "";
            let by = 0;
            if (event.key === Qt.Key_Up || ctrl && (event.key === Qt.Key_P || event.key === Qt.Key_K)) by = -field.step;
            else if (event.key === Qt.Key_Down || ctrl && (event.key === Qt.Key_N || event.key === Qt.Key_J)) by = field.step;
            else if (field.page && event.key === Qt.Key_PageUp) by = -field.page;
            else if (field.page && event.key === Qt.Key_PageDown) by = field.page;
            else if (field.sideways && (ctrl && event.key === Qt.Key_H || empty && event.key === Qt.Key_Left)) by = -1;
            else if (field.sideways && (ctrl && event.key === Qt.Key_L || empty && event.key === Qt.Key_Right)) by = 1;
            if (by !== 0) { field.move(by); event.accepted = true; }
        }
        Text {
            anchors.fill: parent
            verticalAlignment: Text.AlignVCenter
            text: field.placeholder
            color: field.inkDim
            font: input.font
            visible: input.text.length === 0
        }
    }
    MaterialIcon {
        id: clear
        anchors.right: parent.right
        anchors.rightMargin: 9 * field.s
        anchors.verticalCenter: parent.verticalCenter
        visible: input.text.length > 0
        width: visible ? implicitWidth : 0
        name: "close"
        size: 14 * field.s
        color: field.inkDim
        TapHandler { onTapped: input.text = "" }
    }
}
