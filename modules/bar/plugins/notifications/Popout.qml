pragma ComponentBehavior: Bound

import QtQuick
import "../../popouts"
import "../.."
import qs.services

// Notifications card: the 力 NOTIFICATIONS head with the count, a Do Not
// Disturb toggle and clear-all, over the history newest first (Notifs.history).
// Notifications stack per app: the newest shows, with the count; the head
// expands the stack. Clicking an entry runs the sender's default action and
// dismisses it; the head's x clears the whole stack. Opening the card marks everything seen, which clears
// the rail dot. Never grabs the keyboard; dismisses on an outside click.
Item {
    id: root

    property real s: 1
    property bool open: false

    readonly property real pad: 11 * root.s
    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property color inkDim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 3.0)
    readonly property color line: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.14)

    readonly property var items: Notifs.history
    // history folded into one stack per app, ordered by each app's newest.
    readonly property var groups: {
        const out = [];
        const byApp = {};
        for (let i = 0; i < root.items.length; i++) {
            const n = root.items[i];
            const app = n.appName || "";
            if (byApp[app] === undefined) {
                byApp[app] = out.length;
                out.push({ app: app, items: [] });
            }
            out[byApp[app]].items.push(n);
        }
        return out;
    }
    // apps whose stack is open; the rest show only their newest.
    property var expanded: ({})
    function toggle(app) {
        const e = Object.assign({}, root.expanded);
        e[app] = !e[app];
        root.expanded = e;
    }
    function dismissAll(list) {
        for (let i = list.length - 1; i >= 0; i--)
            Notifs.dismiss(list[i]);
    }
    readonly property real listMax: 380 * root.s

    onOpenChanged: {
        if (root.open)
            Notifs.markSeen();
        else
            root.expanded = ({});
    }
    // a notification arriving while the card is up counts as seen.
    Connections {
        target: Notifs
        enabled: root.open
        function onNewestArrivalMsChanged() { Notifs.markSeen(); }
    }

    function defaultAction(n) {
        const all = n.actions || [];
        for (let i = 0; i < all.length; i++)
            if (all[i] && all[i].identifier === "default")
                return all[i];
        return null;
    }

    implicitWidth: 300 * root.s
    implicitHeight: content.implicitHeight + root.pad * 2

    PopoutCard { anchors.fill: parent }

    Column {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: root.pad
        spacing: 9 * root.s

        // head: label + count, DND and clear-all.
        Item {
            width: parent.width
            height: 20 * root.s
            Row {
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                spacing: 6 * root.s
                PopoutTitle {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "NOTIFICATIONS"
                    color: root.inkDim
                    s: root.s
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: root.items.length > 0
                    text: root.items.length
                    color: root.inkDim
                    font.family: Theme.fontPrimary
                    font.pixelSize: 9 * root.s
                }
            }
            Row {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                spacing: 4 * root.s
                PopoutHeadButton {
                    s: root.s
                    glyph: "do_not_disturb_on"
                    active: Notifs.dnd
                    onClicked: Notifs.dnd = !Notifs.dnd
                }
                PopoutHeadButton {
                    s: root.s
                    glyph: "delete"
                    visible: root.items.length > 0
                    onClicked: Notifs.clearAll()
                }
            }
        }

        // empty state.
        Item {
            width: parent.width
            height: 64 * root.s
            visible: root.items.length === 0
            Column {
                anchors.centerIn: parent
                spacing: 4 * root.s
                MaterialIcon {
                    anchors.horizontalCenter: parent.horizontalCenter
                    name: Notifs.dnd ? "notifications_off" : "notifications_none"
                    color: root.inkDim
                    size: 20 * root.s
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: Notifs.dnd ? "Do not disturb is on" : "No notifications"
                    color: root.inkDim
                    font.family: Theme.fontPrimary
                    font.pixelSize: 11 * root.s
                }
            }
        }

        ListView {
            id: list
            width: parent.width
            height: Math.min(contentHeight, root.listMax)
            visible: root.items.length > 0
            clip: true
            spacing: 6 * root.s
            boundsBehavior: Flickable.StopAtBounds
            model: root.groups
            delegate: group
        }
    }

    // one app's stack: a head (app, count, newest time, x) over its newest
    // entry, or every entry while expanded.
    Component {
        id: group
        Rectangle {
            id: g
            required property var modelData
            readonly property string app: modelData.app
            readonly property var entries: modelData.items
            readonly property int count: entries.length
            readonly property bool isOpen: root.expanded[g.app] === true
            readonly property bool critical: {
                for (let i = 0; i < g.entries.length; i++)
                    if (g.entries[i].urgency === 2)
                        return true;
                return false;
            }

            width: ListView.view.width
            height: gcol.implicitHeight + 14 * root.s
            radius: Theme.radiusWidget
            color: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.05)

            HoverHandler { id: gHover }

            Column {
                id: gcol
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 7 * root.s
                anchors.leftMargin: 10 * root.s
                anchors.rightMargin: 10 * root.s
                spacing: 4 * root.s

                // head: tap to expand when there is more than one.
                Item {
                    width: parent.width
                    height: 16 * root.s
                    HoverHandler { cursorShape: g.count > 1 ? Qt.PointingHandCursor : Qt.ArrowCursor }
                    TapHandler { enabled: g.count > 1; onTapped: root.toggle(g.app) }
                    Row {
                        anchors.left: parent.left
                        anchors.right: gRight.left
                        anchors.rightMargin: 6 * root.s
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 5 * root.s
                        Text {
                            id: appLabel
                            anchors.verticalCenter: parent.verticalCenter
                            width: Math.min(implicitWidth, parent.width - countPill.width - 5 * root.s)
                            text: (g.app || "Unknown").toUpperCase()
                            color: g.critical ? Theme.error : root.inkDim
                            font.family: Theme.fontPrimary
                            font.pixelSize: 9 * root.s
                            font.letterSpacing: 1.2
                            elide: Text.ElideRight
                        }
                        Rectangle {
                            id: countPill
                            anchors.verticalCenter: parent.verticalCenter
                            visible: g.count > 1
                            width: countText.implicitWidth + 8 * root.s
                            height: 13 * root.s
                            radius: height / 2
                            color: root.line
                            Text {
                                id: countText
                                anchors.centerIn: parent
                                text: g.count
                                color: root.inkDim
                                font.family: Theme.fontPrimary
                                font.pixelSize: 8.5 * root.s
                            }
                        }
                        MaterialIcon {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: g.count > 1
                            name: g.isOpen ? "expand_less" : "expand_more"
                            color: root.inkDim
                            size: 13 * root.s
                        }
                    }
                    Row {
                        id: gRight
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 4 * root.s
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: Notifs.timeLabel(g.entries[0])
                            color: root.inkDim
                            font.family: Theme.fontPrimary
                            font.pixelSize: 9 * root.s
                        }
                        // clear this app's stack.
                        MaterialIcon {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: gHover.hovered
                            name: "close"
                            color: closeHover.hovered ? root.ink : root.inkDim
                            size: 13 * root.s
                            HoverHandler { id: closeHover; cursorShape: Qt.PointingHandCursor }
                            TapHandler { onTapped: root.dismissAll(g.entries) }
                        }
                    }
                }

                Repeater {
                    model: g.isOpen ? g.entries : g.entries.slice(0, 1)
                    delegate: entry
                }
            }
        }
    }

    // one notification inside a stack: summary and up to two lines of body.
    Component {
        id: entry
        Item {
            id: e
            required property var modelData
            required property int index
            readonly property var n: modelData
            readonly property var action: root.defaultAction(e.n)

            width: parent ? parent.width : 0
            height: ecol.implicitHeight + (e.index > 0 ? 6 * root.s : 0)

            Rectangle {
                visible: e.index > 0
                anchors.top: parent.top
                width: parent.width
                height: 1
                color: root.line
            }
            HoverHandler { id: eHover; cursorShape: e.action ? Qt.PointingHandCursor : Qt.ArrowCursor }
            TapHandler {
                onTapped: {
                    if (e.action)
                        e.action.invoke();
                    Notifs.dismiss(e.n);
                }
            }

            Column {
                id: ecol
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                spacing: 2 * root.s
                Item {
                    width: parent.width
                    height: summary.implicitHeight
                    visible: summary.text.length > 0
                    Text {
                        id: summary
                        anchors.left: parent.left
                        anchors.right: eTime.left
                        anchors.rightMargin: 6 * root.s
                        text: e.n.summary || ""
                        color: eHover.hovered && e.action ? Theme.vermLit : root.ink
                        font.family: Theme.fontPrimary
                        font.pixelSize: 12 * root.s
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                    }
                    // per-entry time only inside an open stack (the head has the newest).
                    Text {
                        id: eTime
                        anchors.right: parent.right
                        anchors.verticalCenter: summary.verticalCenter
                        visible: e.index > 0
                        width: visible ? implicitWidth : 0
                        text: Notifs.timeLabel(e.n)
                        color: root.inkDim
                        font.family: Theme.fontPrimary
                        font.pixelSize: 8.5 * root.s
                    }
                }
                Text {
                    width: parent.width
                    text: e.n.body || ""
                    visible: text.length > 0
                    color: root.inkDim
                    font.family: Theme.fontPrimary
                    font.pixelSize: 11 * root.s
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                }
            }
        }
    }
}
