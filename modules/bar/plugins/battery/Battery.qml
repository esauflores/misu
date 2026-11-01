pragma Singleton

import QtQuick
import Quickshell
import Quickshell.Services.UPower

// The laptop battery from UPower's display device.
Singleton {
    id: root

    readonly property var dev: UPower.displayDevice
    readonly property bool present: !!dev && dev.isLaptopBattery
    readonly property real frac: present ? Math.max(0, Math.min(1, dev.percentage)) : 0
    readonly property int pct: Math.round(frac * 100)
    readonly property bool charging: present && (dev.state === UPowerDeviceState.Charging
        || dev.state === UPowerDeviceState.PendingCharge)
    readonly property bool full: present && (dev.state === UPowerDeviceState.FullyCharged || pct >= 100)
    readonly property bool low: present && !charging && pct <= 15
    readonly property real rateW: present ? Math.abs(dev.changeRate) : 0
    readonly property real capacityWh: present ? dev.energyCapacity : 0
    readonly property bool healthSupported: present && dev.healthSupported
    readonly property int health: healthSupported ? Math.round(dev.healthPercentage) : 0

    readonly property real seconds: !present ? 0 : charging ? dev.timeToFull : dev.timeToEmpty
    readonly property bool hasTime: seconds > 0 && !full
    readonly property string timeStr: {
        if (!hasTime) return "";
        const h = Math.floor(seconds / 3600), m = Math.round(seconds % 3600 / 60);
        return h > 0 ? h + "h " + m + "m" : m + "m";
    }
    readonly property string stateLabel: !present ? ""
        : full ? "Full" : charging ? "Charging"
        : dev.state === UPowerDeviceState.Discharging ? "Discharging" : "Not charging"
}
