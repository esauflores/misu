pragma Singleton

import QtQuick
import Quickshell

// Animation durations (ms) and curves. `reduce` (config reduceMotion) zeroes
// every duration.
Singleton {
    id: root

    readonly property bool reduce: Config.reduceMotion
    function dur(ms) { return root.reduce ? 0 : ms; }

    readonly property int fast: dur(120)
    readonly property int standard: dur(200)
    readonly property int spatial: dur(300)
    readonly property int effects: dur(150)
    readonly property int crossfade: dur(180)
    readonly property int morph: dur(260)
    readonly property int diagonal: dur(200)
    readonly property int barReveal: dur(250)
    readonly property int menuSlide: dur(250)
    readonly property int rowReveal: dur(200)
    readonly property int rowFade: dur(150)
    readonly property int chevronRotate: dur(160)
    readonly property int sidebarEnter: dur(400)
    readonly property int sidebarExit: dur(200)
    readonly property int weatherFade: dur(220)

    // easing.type values
    readonly property int easeStandard: Easing.OutCubic
    readonly property int easeType: Easing.BezierSpline
    readonly property int barRevealCurve: Easing.OutCubic
    readonly property int menuSlideCurve: Easing.OutCubic
    readonly property int diagonalCurve: Easing.OutCubic
    readonly property int crossfadeCurve: Easing.InOutQuad
    readonly property int rowRevealCurve: Easing.OutCubic

    // easing.bezierCurve values (BezierSpline control points)
    readonly property var easeCurve: [0.2, 0, 0, 1, 1, 1]
    readonly property var spatialCurve: [0.38, 1.21, 0.22, 1, 1, 1]
    readonly property var effectsCurve: [0.34, 0.8, 0.34, 1, 1, 1]
    readonly property var sidebarEnterCurve: [0.05, 0.7, 0.1, 1, 1, 1]
    readonly property var sidebarExitCurve: [0.3, 0, 0.8, 0.15, 1, 1]
    readonly property var sidebarFadeInCurve: [0.2, 0, 0, 1, 1, 1]
    readonly property var sidebarFadeOutCurve: [0.4, 0, 1, 1, 1, 1]
}
