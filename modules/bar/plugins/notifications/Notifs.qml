pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Services.Notifications

// Misu is the session's notification daemon. Every notification is kept
// (tracked) until dismissed: `history` is the list newest first, `popups` the
// ones still inside their on-screen timeout (none while `dnd` is on).
Singleton {
    id: root

    property var arrivals: ({})        // notification id -> arrival ms
    // Do not disturb: new notifications go to history without a popup.
    property bool dnd: false
    property real newestArrivalMs: 0
    property real seenMs: 0
    property var popups: []

    readonly property var history: {
        const vals = server.trackedNotifications ? server.trackedNotifications.values : [];
        return vals.slice().reverse();
    }
    readonly property bool hasUnread: root.history.length > 0 && root.newestArrivalMs > root.seenMs

    function markSeen() { root.seenMs = Date.now(); }
    function dismiss(n) {
        if (!n) return;
        root.dropPopup(n);
        n.dismiss();
    }
    function clearAll() {
        const all = root.history.slice();
        for (const n of all) n.dismiss();
        root.popups = [];
    }
    function dropPopup(n) { root.popups = root.popups.filter(p => p !== n); }

    function timeLabel(n) {
        const t = n ? root.arrivals[n.id] : 0;
        if (!t) return "";
        const s = Math.floor((Date.now() - t) / 1000);
        if (s < 60) return "now";
        if (s < 3600) return Math.floor(s / 60) + "m";
        if (s < 86400) return Math.floor(s / 3600) + "h";
        const d = new Date(t);
        return (d.getMonth() + 1) + "/" + d.getDate();
    }

    // Popup lifetime in ms: the sender's timeout, 5 s by default, never for
    // critical ones (they stay until clicked).
    function popupTimeout(n) {
        if (n.urgency === NotificationUrgency.Critical) return 0;
        return n.expireTimeout > 0 ? n.expireTimeout : 5000;
    }

    NotificationServer {
        id: server
        keepOnReload: true
        bodySupported: true
        bodyMarkupSupported: true
        bodyHyperlinksSupported: true
        actionsSupported: true
        imageSupported: true
        persistenceSupported: true

        onNotification: n => {
            n.tracked = true;
            const now = Date.now();
            const a = Object.assign({}, root.arrivals);
            a[n.id] = now;
            root.arrivals = a;
            root.newestArrivalMs = now;
            if (!root.dnd)
                root.popups = [n].concat(root.popups.filter(p => p.id !== n.id)).slice(0, 5);
            n.closed.connect(() => root.dropPopup(n));
        }
    }
}
