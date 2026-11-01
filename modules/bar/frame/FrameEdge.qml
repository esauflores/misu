pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.services

// The right edge's reservation: an invisible, input-less surface whose
// exclusive zone keeps tiled windows clear of the rail. The zone animates
// with the rail's reveal. It stays mapped at a 1px floor so hiding the rail
// only drops the zone instead of unmapping the surface.
PanelWindow {
    required property real reserve

    property real zone: reserve
    Behavior on zone {
        NumberAnimation { duration: Motion.barReveal; easing.type: Motion.barRevealCurve }
    }

    color: "transparent"
    WlrLayershell.layer: WlrLayer.Background
    WlrLayershell.namespace: "misu-edge"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    exclusionMode: ExclusionMode.Normal
    exclusiveZone: Math.round(zone)
    anchors { top: true; bottom: true; right: true }
    implicitWidth: Math.max(1, Math.round(zone))
    mask: Region {}
}
