import QtQuick
import QtQuick.Shapes
import "FrameChromeGeometry.js" as FrameGeometry

// The frame's painted chrome: one static odd-even scene-graph path, the surface
// band around a rounded desktop hole. Animated menu panels are ordinary
// scene-graph rectangles; keeping them out of this path avoids rebuilding curved
// geometry on every reveal frame. misu: no outline along the hole edge.
Shape {
    id: chrome

    property real reserveLeft: 0
    property real reserveTop: 0
    property real reserveRight: 0
    property real reserveBottom: 0
    property real holeRadius: 8
    property color surface: "#101315"

    readonly property var holePoints: [
        { x: chrome.reserveLeft, y: chrome.reserveTop },
        { x: chrome.width - chrome.reserveRight, y: chrome.reserveTop },
        { x: chrome.width - chrome.reserveRight, y: chrome.height - chrome.reserveBottom },
        { x: chrome.reserveLeft, y: chrome.height - chrome.reserveBottom }
    ]
    // misu: a hole corner is rounded when either edge beside it carries a bar,
    // so a lone rail still curves into the screen edges at its ends; a corner
    // between two bare edges stays square (no dark wedge in the screen corner).
    // Order matches holePoints: top-left, top-right, bottom-right, bottom-left.
    readonly property var holeRadii: [
        (chrome.reserveTop > 0 || chrome.reserveLeft > 0) ? chrome.holeRadius : 0,
        (chrome.reserveTop > 0 || chrome.reserveRight > 0) ? chrome.holeRadius : 0,
        (chrome.reserveBottom > 0 || chrome.reserveRight > 0) ? chrome.holeRadius : 0,
        (chrome.reserveBottom > 0 || chrome.reserveLeft > 0) ? chrome.holeRadius : 0
    ]

    preferredRendererType: Shape.CurveRenderer

    ShapePath {
        fillColor: chrome.surface
        strokeColor: "transparent"
        fillRule: ShapePath.OddEvenFill
        PathSvg { path: FrameGeometry.framePath(chrome.width, chrome.height, chrome.holePoints, chrome.holeRadii) }
    }
}
