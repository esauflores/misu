pragma ComponentBehavior: Bound

import QtQuick

// One zone of the rail. A start zone hugs the top, an end zone the bottom, a
// centre zone sits on the midpoint, sliding just enough to clear a grown
// neighbour. Widgets centre on the rail's width.
Item {
    id: root

    required property var ids
    required property string align   // "start" | "center" | "end"
    property real spacing: 0
    property Component delegate: null
    // start/end zone heights, fed to the centre zone only
    property real leadExtent: 0
    property real tailExtent: 0

    implicitWidth: column.implicitWidth
    implicitHeight: column.implicitHeight

    function centreShift() { return (root.leadExtent - root.tailExtent) / 2; }

    Column {
        id: column
        spacing: root.spacing
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: root.align === "start" ? parent.top : undefined
        anchors.bottom: root.align === "end" ? parent.bottom : undefined
        anchors.verticalCenter: root.align === "center" ? parent.verticalCenter : undefined
        anchors.verticalCenterOffset: root.align === "center" ? root.centreShift() : 0

        Repeater {
            model: root.ids
            Loader {
                required property string modelData
                anchors.horizontalCenter: parent ? parent.horizontalCenter : undefined
                active: root.delegate !== null
                sourceComponent: root.delegate
                onLoaded: if (item) item.widgetId = modelData
            }
        }
    }
}
