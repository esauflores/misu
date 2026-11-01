pragma ComponentBehavior: Bound

import QtQuick
import qs.services
import "../.."

// The device switcher shared by the audio output and input mixers: a header
// showing the current default device (icon + name + a chevron) that toggles a
// revealed list of the available devices, each selectable with a check on the
// current default. The host owns `listOpen` and switches the default on `picked`.
Column {
    id: root

    property real s: 1
    property var current: null
    property var devices: []
    property bool listOpen: false
    property string emptyLabel: ""
    property string fallbackIcon: "speaker"

    signal toggled()
    signal picked(var node)

    spacing: 2 * root.s

    DeviceButton {
        id: header
        width: root.width
        minH: headRow.implicitHeight + header.pad * 2
        keepEnabledLook: true
        onClicked: root.toggled()

        Row {
            id: headRow
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: 8 * root.s

            MaterialIcon {
                anchors.verticalCenter: parent.verticalCenter
                size: 15 * root.s
                name: root.current ? Audio.nodeIcon(root.current) : root.fallbackIcon
                color: header.contentColor
            }
            Text {
                width: parent.width - (15 + 16 + 8 * 2) * root.s
                anchors.verticalCenter: parent.verticalCenter
                text: root.current ? Audio.nodeLabel(root.current) : root.emptyLabel
                elide: Text.ElideRight
                color: header.contentColor
                font.family: Theme.fontPrimary
                font.pixelSize: 11 * root.s
                font.weight: Font.DemiBold
            }
            MaterialIcon {
                anchors.verticalCenter: parent.verticalCenter
                size: 16 * root.s
                name: "expand_more"
                color: header.contentColor
                rotation: root.listOpen ? 180 : 0
                Behavior on rotation { NumberAnimation { duration: Motion.chevronRotate; easing.type: Motion.easeType; easing.bezierCurve: Motion.easeCurve } }
            }
        }
    }

    Item {
        width: root.width
        clip: true
        height: root.listOpen ? list.implicitHeight : 0
        Behavior on height { NumberAnimation { duration: Motion.rowReveal; easing.type: Motion.rowRevealCurve } }

        Column {
            id: list
            width: parent.width
            spacing: 2 * root.s

            Repeater {
                model: root.listOpen ? root.devices : []
                delegate: DeviceButton {
                    id: item
                    required property var modelData
                    readonly property bool isDefault: root.current && item.modelData && item.modelData.name === root.current.name
                    width: parent.width
                    minH: lbl.implicitHeight + item.pad * 2
                    selected: item.isDefault
                    onClicked: root.picked(item.modelData)
                    DeviceRow {
                        id: lbl
                        anchors.fill: parent
                        iconName: item.isDefault ? "check_circle" : ""
                        label: Audio.nodeLabel(item.modelData)
                        iconColor: item.contentColor
                    }
                }
            }
        }
    }
}
