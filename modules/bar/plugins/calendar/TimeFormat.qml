pragma Singleton

import QtQuick
import Quickshell
import qs.services

// Clock text, 24h unless config.json "clock24h" is false.
Singleton {
    id: root
    readonly property bool use24h: Config.clock24h

    function pad(n) { return n < 10 ? "0" + n : String(n); }
    function hours(d) {
        const h = d.getHours();
        return root.use24h ? h : ((h % 12) || 12);
    }
    // "HHMM" (four digits, for the stacked vertical clock)
    function digits(d) { return root.pad(root.hours(d)) + root.pad(d.getMinutes()); }
    // "HH:MM"
    function format(d) { return root.pad(root.hours(d)) + ":" + root.pad(d.getMinutes()); }
}
