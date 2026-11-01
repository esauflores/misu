import QtQuick
import qs.services

// One Material Symbols Rounded glyph by ligature name ("volume_up"), centred
// in a `size` px square. `fill` drives the FILL axis: 0 outline, 1 solid.
Item {
    id: root

    property string name: ""
    property color color: Theme.surfaceVariantText
    property real size: 16
    property real fill: 0

    implicitWidth: size
    implicitHeight: size

    Text {
        anchors.centerIn: parent
        text: root.name
        color: root.color
        font.family: "Material Symbols Rounded"
        font.pixelSize: root.size
        font.weight: 500
        font.variableAxes: ({ "FILL": root.fill, "opsz": Math.max(20, Math.min(48, root.size)) })
    }
}
