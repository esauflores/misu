pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Effects
import qs.services

// The brand seal, 力 by default; config.json markText (glyph/text) or
// markImage (an image path) swaps it. Pass the pixel size as `size` and the
// colour as `color`.
Item {
    id: mark

    property real size: 13
    property color color: Theme.primary
    property int weight: Font.Medium
    // recolour a single-colour image mark to `color` (matches the tinted 力
    // idiom); off shows a full-colour logo as-is. no effect in text mode.
    property bool tint: Theme.markTint

    readonly property bool image: Theme.markSource.length > 0
    readonly property string imageSource: !mark.image ? ""
        : (/^(file|qrc|image|https?):/.test(Theme.markSource) ? Theme.markSource
                                                              : "file://" + Theme.markSource)

    implicitWidth:  mark.image ? mark.size : glyph.implicitWidth
    implicitHeight: mark.image ? mark.size : glyph.implicitHeight

    Text {
        id: glyph
        visible: !mark.image
        anchors.centerIn: parent
        text: Theme.mark
        color: mark.color
        font.family: Theme.fontJp
        font.weight: mark.weight
        font.pixelSize: mark.size
    }

    // custom image mark. tint recolours a single-colour logo to `color` (the 力
    // idiom) via an alpha-preserving overlay, so a dark OR light mark both take
    // the accent; tint off shows a full-colour logo as-is.
    Image {
        id: img
        anchors.centerIn: parent
        width: mark.size
        height: mark.size
        visible: mark.image && !mark.tint
        source: mark.imageSource
        sourceSize.width: Math.round(mark.size * 2)
        sourceSize.height: Math.round(mark.size * 2)
        fillMode: Image.PreserveAspectFit
        smooth: true
    }

    MultiEffect {
        anchors.fill: img
        visible: mark.image && mark.tint
        source: img
        // lift the glyph to white, then tint: a flat recolour that keeps alpha
        brightness: 1
        colorization: 1
        colorizationColor: mark.color
    }
}
