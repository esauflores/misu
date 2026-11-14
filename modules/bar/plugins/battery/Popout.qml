pragma ComponentBehavior: Bound

import QtQuick
import Quickshell.Bluetooth
import "../../popouts"
import "../.."
import qs.services
import "../bluetooth"

// Battery popout, flat like the audio and network cards: the level with what
// it's doing (charging, time to full or left) and a bar under them, the power
// mode as a segmented setting, then the few details, always shown: the rate,
// health, capacity and any connected Bluetooth device's own battery. All live
// off the Battery + PowerProfiles services. Never grabs the keyboard;
// dismisses on an outside click.
Item {
    id: root

    property real s: 1
    property bool open: false

    readonly property real pad: 11 * root.s
    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property color inkDim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)
    readonly property color line: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.14)

    readonly property real indent: 7 * root.s

    function profileLabel(name) {
        return name === "power-saver" ? "Eco"
            : name === "balanced" ? "Balanced"
            : name === "performance" ? "Performance"
            : name;
    }
    // connected Bluetooth devices that report a battery, for the detail block.
    readonly property var btBatteries: {
        if (typeof Bluetooth === "undefined" || !Bluetooth || !Bluetooth.devices)
            return [];
        const out = [];
        const vals = Bluetooth.devices.values;
        for (let i = 0; i < vals.length; i++) {
            const d = vals[i];
            if (d && d.connected && BtLink.batteryLevel(d) >= 0)
                out.push({ name: BtLink.label(d), pct: BtLink.batteryLevel(d) });
        }
        return out;
    }

    implicitWidth: 270 * root.s
    implicitHeight: content.implicitHeight + root.pad * 2

    PopoutCard { anchors.fill: parent }

    Column {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: root.pad
        spacing: 6 * root.s

        PopoutTitle {
            text: "BATTERY"
            s: root.s
            bottomPadding: 4 * root.s
        }

        // absent battery (desktop) - the widget self-hides, but stay honest.
        Text {
            visible: !Battery.present
            width: parent.width
            leftPadding: root.indent
            text: "No battery"
            color: root.inkDim
            font.family: Theme.fontPrimary
            font.pixelSize: Theme.h5 * root.s
        }

        // the level, what it's doing, and a bar.
        Item {
            visible: Battery.present
            width: parent.width
            height: level.implicitHeight
            Text {
                id: level
                anchors.left: parent.left
                anchors.leftMargin: root.indent
                text: Battery.pct + "%"
                color: Battery.low ? Theme.error : root.ink
                font.family: Theme.fontPrimary
                font.pixelSize: Theme.h1 * root.s
                font.weight: Font.Medium
                font.features: ({ "tnum": 1 })
            }
            Row {
                anchors.left: level.right
                anchors.leftMargin: 10 * root.s
                anchors.right: parent.right
                anchors.verticalCenter: level.verticalCenter
                spacing: 3 * root.s
                MaterialIcon {
                    visible: Battery.charging
                    anchors.verticalCenter: state.verticalCenter
                    size: 13 * root.s
                    name: "bolt"
                    fill: 1
                    color: root.ink
                }
                Text {
                    id: state
                    width: parent.width - (Battery.charging ? 16 * root.s : 0)
                    text: !Battery.hasTime ? Battery.stateLabel
                        : Battery.charging ? "%1 · %2 to full".arg(Battery.stateLabel).arg(Battery.timeStr)
                        : "%1 · %2 left".arg(Battery.stateLabel).arg(Battery.timeStr)
                    color: root.inkDim
                    font.family: Theme.fontPrimary
                    font.pixelSize: Theme.h5 * root.s
                    elide: Text.ElideRight
                }
            }
        }
        Item {
            visible: Battery.present
            width: parent.width - root.indent * 2
            x: root.indent
            height: 14 * root.s
            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                height: 5 * root.s
                radius: height / 2
                color: root.line
                Rectangle {
                    width: parent.width * Math.max(0, Math.min(1, Battery.frac))
                    height: parent.height
                    radius: parent.radius
                    color: Battery.low ? Theme.error : Theme.vermLit
                    Behavior on width { NumberAnimation { duration: Motion.fast } }
                }
            }
        }

        PopoutSetting {
            visible: PowerProfiles.available
            width: parent.width
            s: root.s
            name: "Mode"
            options: PowerProfiles.profiles.map(p => ({ id: p, label: root.profileLabel(p) }))
            value: PowerProfiles.profile
            onPicked: id => PowerProfiles.setProfile(id)
        }

        Rectangle {
            visible: Battery.present
            width: parent.width
            height: 1
            color: root.line
        }

        // details, few enough to always show.
        Column {
            visible: Battery.present
            width: parent.width - root.indent * 2
            x: root.indent

            PopoutDetailRow {
                width: parent.width
                s: root.s
                size: Theme.h5
                label: Battery.charging ? "Charging at" : "Using"
                value: Math.abs(Battery.rateW).toFixed(1) + " W"
            }
            PopoutDetailRow {
                width: parent.width
                visible: Battery.healthSupported
                s: root.s
                size: Theme.h5
                label: "Health"
                value: Battery.health + "%"
            }
            PopoutDetailRow {
                width: parent.width
                s: root.s
                size: Theme.h5
                label: "Capacity"
                value: Battery.capacityWh.toFixed(1) + " Wh"
            }
            Repeater {
                model: root.btBatteries
                delegate: PopoutDetailRow {
                    required property var modelData
                    width: parent.width
                    s: root.s
                    size: Theme.h5
                    label: modelData.name
                    value: modelData.pct + "%"
                }
            }
        }
    }
}
