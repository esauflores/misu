pragma ComponentBehavior: Bound

import QtQuick
import "../../rail"
import "../.."
import qs.services

// Decorative Misu brand mark.
Item {
    id: root

    property string edge: "right"
    property real scale: 1
    property string widgetId: "quick-settings"
    implicitWidth: btn.implicitWidth
    implicitHeight: btn.implicitHeight

    RailButton {
        id: btn
        anchors.centerIn: parent
        edge: root.edge
        scale: root.scale
        interactive: false

        // Sized off the button's glyph box, so the mark tracks every other
        // rail icon instead of shrinking with a thin bar on its own.
        Item {
            width: btn.glyphPx
            height: btn.glyphPx
            BrandMark {
                anchors.centerIn: parent
                size: btn.glyphPx * (15 / Theme.iconSm)
                color: Theme.surfaceText
            }
        }
    }
}
