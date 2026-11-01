pragma ComponentBehavior: Bound

import QtQuick
import qs.services
import "../plugins/index.js" as Plugins

// One rail slot: the built-in widget for `widgetId` (workspaces, tray), or the
// Rail.qml of the plugin that claims it (plugins/index.js). Forwards the
// widget's card request up to the frame. A self-hiding widget collapses the
// slot to nothing so its zone leaves no gap.
Item {
    id: root

    property string widgetId: ""
    required property string edge
    required property real scale
    signal menuRequested(string id, rect ownerRect)

    implicitWidth: loader.item ? loader.item.implicitWidth : 0
    implicitHeight: loader.item ? loader.item.implicitHeight : 0
    visible: loader.item ? loader.item.implicitWidth > 0 : false

    // Plugin rail components by plugin name, built once. A var property keeps
    // the JS-owned components alive past the GC.
    property var _railComponents: ({})
    function pluginRail(name) {
        if (!root._railComponents[name])
            root._railComponents[name] = Qt.createComponent(Qt.resolvedUrl("../plugins/" + name + "/Rail.qml"));
        // a component built from a URL fails silently, so say why.
        if (root._railComponents[name].status === Component.Error)
            console.error("misu plugin " + name + ": " + root._railComponents[name].errorString());
        return root._railComponents[name];
    }

    readonly property string plugin: Plugins.railPlugin(root.widgetId)

    // resolves from `id`, not root.plugin: the sourceComponent binding can run
    // before `plugin` catches up with a new widgetId.
    function componentFor(id) {
        const plugin = Plugins.railPlugin(id);
        if (plugin !== "") return root.pluginRail(plugin);
        if (id === "workspaces") return workspacesComponent;
        if (id === "tray") return trayComponent;
        if (id !== "") console.error("misu: no rail widget " + id);
        return null;
    }

    Loader {
        id: loader
        anchors.centerIn: parent
        sourceComponent: root.componentFor(root.widgetId)
        // A plugin rail is built from a URL, so it gets edge, scale and id here.
        onLoaded: if (root.plugin !== "") {
            item.edge = Qt.binding(() => root.edge);
            item.scale = Qt.binding(() => root.scale);
            item.widgetId = root.widgetId;
        }
    }
    Connections {
        target: loader.item
        ignoreUnknownSignals: true
        function onMenuRequested(id, ownerRect) {
            const topLeft = loader.item.mapToGlobal(ownerRect.x, ownerRect.y);
            root.menuRequested(id, { x: topLeft.x, y: topLeft.y, width: ownerRect.width, height: ownerRect.height });
        }
    }

    Component { id: workspacesComponent; Workspaces { edge: root.edge; scale: root.scale } }
    Component { id: trayComponent; Tray { edge: root.edge; scale: root.scale } }
}
