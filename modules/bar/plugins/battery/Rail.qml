pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Shapes
import qs.services
import "../../rail"

// Battery rail button: charge level when present, an unknown-battery mark when
// no battery is connected.
Item {
    id: root

    property string edge: "right"
    property real scale: 1
    property string widgetId: "battery"
    signal menuRequested(string id, rect ownerRect)

    readonly property bool charging: Battery.present && (Battery.charging || Battery.full)

    implicitWidth: btn.implicitWidth
    implicitHeight: btn.implicitHeight

    // The charge level as bars, the same glyph charging or not; a bolt beside
    // it says it's charging (Material's charging glyphs hide the level).
    function glyph() {
        if (!Battery.present) return "battery_unknown";
        const pct = Battery.pct;
        return pct > 95 ? "battery_full" : "battery_" + Math.round(pct / 100 * 6) + "_bar";
    }

    RailButton {
        id: btn
        anchors.centerIn: parent
        // nudged left while the bolt shows, so the pair sits centred
        anchors.horizontalCenterOffset: root.charging ? -btn.glyphPx * 0.2 : 0
        edge: root.edge
        scale: root.scale
        icon: root.glyph()
        iconColor: Battery.present ? (Battery.low ? Theme.error : btn.foreground) : Theme.surfaceVariantText
        onClicked: root.menuRequested("battery", Qt.rect(btn.x, btn.y, btn.width, btn.height))

        // the bolt, drawn as a path (Material's bolt glyphs blur at this
        // size); a zero-size anchor keeps it from widening the button.
        Item {
            visible: root.charging
            x: btn.glyphPx * 0.98
            y: btn.glyphPx * 0.5
            // a 6×10 bolt, scaled to the glyph
            Shape {
                x: -width / 2
                y: -height / 2
                width: 6
                height: 10
                scale: btn.glyphPx * 0.07
                preferredRendererType: Shape.CurveRenderer
                ShapePath {
                    strokeWidth: 0
                    strokeColor: "transparent"
                    fillColor: btn.iconColor
                    PathSvg { path: "M 3.7 0 L 0 5.8 L 2.7 5.8 L 1.9 10 L 6 4 L 3.3 4 Z" }
                }
            }
        }
    }
}
