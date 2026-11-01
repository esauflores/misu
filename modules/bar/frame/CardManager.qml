pragma ComponentBehavior: Bound

import QtQuick
import qs.services
import "../plugins/index.js" as Plugins

// This monitor's rail cards (plugins/<name>/Popout.qml). At most one is open:
// it slides out of the rail next to the button that asked for it, or centred
// on the rail when opened over IPC. Asking for the open card again closes it.
// The frame unions `mask` into its input region while a card shows.
Item {
    id: root

    anchors.fill: parent

    required property string monitorName
    required property real scale
    required property var group
    // Distance from the screen edge to the inside of the rail.
    required property real railClearance
    property bool active: true

    readonly property var records: Plugins.surfaceRecords()

    property string openId: ""
    property real openAlong: -1
    property var trigger: null

    readonly property bool anyOpen: root.openId !== ""
    readonly property bool anyVisible: {
        if (root.anyOpen) return true;
        for (let i = 0; i < cards.count; ++i) {
            const c = cards.itemAt(i);
            if (c && (c.heldOpen || c.prog > 0.004)) return true;
        }
        return false;
    }
    // The open card's keyboard need ("ondemand" for the network password
    // field), else none.
    readonly property string keyboardMode: {
        for (const r of root.records)
            if (r.id === root.openId) return r.keyboard || "none";
        return "none";
    }

    // Trigger and body rects of the showing card, in window coordinates.
    readonly property var mask: {
        const t = root.trigger;
        const out = { tx: t ? t.x : 0, ty: t ? t.y : 0, tw: t ? t.width : 0, th: t ? t.height : 0, bx: 0, by: 0, bw: 0, bh: 0 };
        for (let i = 0; i < cards.count; ++i) {
            const c = cards.itemAt(i);
            if (!c || c.maskW <= 0 || c.maskH <= 0) continue;
            out.bx = c.maskX; out.by = c.maskY; out.bw = c.maskW; out.bh = c.maskH;
        }
        return out;
    }

    // ownerRect is the button's global rect, or null for an IPC open.
    function openSurface(id, ownerRect) {
        if (root.openId === id) {
            root.closeAll();
            return;
        }
        if (!root.records.some(r => r.id === id)) {
            console.warn("misu: no card named " + id);
            return;
        }
        if (ownerRect && ownerRect.width > 0) {
            const p = root.mapFromGlobal(ownerRect.x, ownerRect.y);
            root.trigger = { x: p.x, y: p.y, width: ownerRect.width, height: ownerRect.height };
            root.openAlong = p.y + ownerRect.height / 2;
        } else {
            root.trigger = null;
            root.openAlong = root.height / 2;
        }
        root.openId = id;
    }
    function closeAll() {
        root.openId = "";
        root.trigger = null;
    }

    onActiveChanged: if (!root.active) root.closeAll()

    Repeater {
        id: cards
        model: root.records
        delegate: FrameSurface {
            required property var modelData
            group: root.group
            frameThickness: root.railClearance
            radius: Theme.radiusWindow
            s: root.scale
            active: root.active
            record: modelData
            anchor: "right"
            menuOpen: root.active && root.openId === modelData.id
            triggerAlong: root.openId === modelData.id ? root.openAlong : -1
            onRequestClose: root.closeAll()
        }
    }
}
