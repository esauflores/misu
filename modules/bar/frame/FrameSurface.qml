pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import Quickshell.Widgets
import qs.services
import "../popouts"
import "../plugins/index.js" as Plugins
// Plugin folders are loaded by URL (Qt.createComponent), which Quickshell's
// config scanner cannot see; importing them here gets every plugin file
// indexed so sibling types (BtHero, MprisPlayer, Usage) resolve.
import "../plugins/audio" as PluginAudio
import "../plugins/battery" as PluginBattery
import "../plugins/bluetooth" as PluginBluetooth
import "../plugins/calendar" as PluginCalendar
import "../plugins/aiusage" as PluginAiUsage
import "../plugins/mpris" as PluginMpris
import "../plugins/network" as PluginNetwork
import "../plugins/notifications" as PluginNotifications

// One rail card: plugins/<kind>/Popout.qml riding the shared Popout, sliding
// out of the right rail. The card is built on first open and kept, so
// reopening only runs the reveal; its `open` drops on close, which stops its
// scans and timers.
Popout {
    id: root

    property var record: null
    property string anchor: "right"
    property bool menuOpen: false
    // The trigger centre along the rail (the button, or the rail's middle).
    property real triggerAlong: -1

    signal requestClose()

    readonly property real minWidth: record && record.minWidth ? record.minWidth : 200
    readonly property string kind: record ? record.kind : ""
    // stays true through the close slide so the body tears down only once flush.
    readonly property bool effectiveOpen: menuOpen || prog > 0.004

    edge: anchor
    align: "center"
    hoverOpen: false
    pinned: root.menuOpen
    alongCenter: root.triggerAlong

    openW: Math.max(root.minWidth * root.s, body.item ? body.item.implicitWidth : 0)
    // Capped to a share of the screen; anything taller scrolls inside the card.
    readonly property real contentH: body.item ? body.item.implicitHeight : 0
    readonly property bool overflows: root.contentH > root.maxH
    readonly property real maxH: Math.round(root.height * Config.cardMaxHeight)
    openH: Math.min(root.contentH, root.maxH)
    // Cards float off the frame lip.
    edgeGap: 10 * root.s

    // Held in a var property so the JS-owned component outlives the GC.
    readonly property var pluginBody: Plugins.hasPopout(root.kind)
        ? Qt.createComponent(Qt.resolvedUrl("../plugins/" + root.kind + "/Popout.qml")) : null
    // a component built from a URL fails silently, so say why.
    onPluginBodyChanged: if (root.pluginBody && root.pluginBody.status === Component.Error)
        console.error("misu plugin " + root.kind + ": " + root.pluginBody.errorString())

    property bool everOpened: false
    onMenuOpenChanged: if (root.menuOpen) { root.everOpened = true; scroller.contentY = 0; }

    // One quick 150 ms slide, zeroed by reduce-motion.
    transitions: [
        Transition {
            to: "open"
            NumberAnimation { property: "prog"; duration: Motion.dur(150); easing.type: Motion.menuSlideCurve }
        },
        Transition {
            from: "open"
            NumberAnimation { property: "prog"; duration: Motion.dur(150); easing.type: Motion.menuSlideCurve }
        }
    ]

    // The card's own skin scrolls with its content, so the rounded clip keeps
    // its corners and the outline below redraws the edges a scroll cuts off.
    ClippingRectangle {
        width: root.openW
        height: root.openH
        radius: Theme.radiusWindow
        color: "transparent"

        Flickable {
            id: scroller
            anchors.fill: parent
            contentWidth: width
            contentHeight: root.contentH
            interactive: root.overflows
            flickableDirection: Flickable.VerticalFlick
            // Wheel and touchpad only, so a drag on a fader never scrolls the card.
            acceptedButtons: Qt.NoButton
            boundsBehavior: Flickable.StopAtBounds

            Loader {
                id: body
                active: root.pluginBody !== null && (root.effectiveOpen || root.everOpened)
                width: root.openW
                height: root.contentH
                sourceComponent: root.pluginBody
                onLoaded: {
                    item.s = Qt.binding(() => root.s);
                    item.open = Qt.binding(() => root.effectiveOpen);
                    if ("requestClose" in item) item.requestClose.connect(root.requestClose);
                }
            }

            ScrollBar.vertical: ScrollBar {
                id: bar
                policy: root.overflows ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff
                rightPadding: 3 * root.s
                topPadding: 8 * root.s
                bottomPadding: 8 * root.s
                contentItem: Rectangle {
                    implicitWidth: 4 * root.s
                    radius: width / 2
                    color: Theme.outline
                    opacity: bar.active ? 0.9 : 0.45
                    Behavior on opacity { NumberAnimation { duration: Motion.effects } }
                }
            }
        }

        Rectangle {
            anchors.fill: parent
            visible: root.overflows
            radius: Theme.radiusWindow
            color: "transparent"
            border.width: Theme.borderWidth
            border.color: Theme.outline
        }
    }
}
