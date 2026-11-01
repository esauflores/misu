pragma ComponentBehavior: Bound

import QtQuick
import "../../popouts"
import "../.."
import qs.services
import "usage.js" as Calc

// AI usage card: a tab per provider (Claude, Codex) and the selected one's
// windows below, each
// leading with the percent used, a gauge with a faint tick where even spending
// would be by now, then the pace in points beside the countdown to the reset.
// Colour follows the CLI's severity: amber at high, red at critical. Windows
// that report a balance show the amount instead of a gauge. Never grabs the
// keyboard; dismisses on an outside click.
Item {
    id: root

    property real s: 1
    property bool open: false

    readonly property real pad: 14 * root.s
    readonly property color ink: Theme.ink(Theme.effectiveSurface)
    readonly property color dim: Theme.inkOn(Theme.effectiveSurface, Theme.surfaceVariantText, 4.5)
    readonly property color line: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.16)
    readonly property color warn: Qt.tint(Theme.primary, Qt.rgba(Theme.error.r, Theme.error.g, Theme.error.b, 0.55))

    // the card's two text styles: small figures (dim by default) and heads.
    component Small: Text {
        color: root.dim
        font.family: Theme.fontPrimary
        font.pixelSize: 11 * root.s
    }
    component Head: Text {
        color: root.ink
        font.family: Theme.fontPrimary
        font.pixelSize: 12 * root.s
        font.weight: Font.DemiBold
    }

    // fresh numbers on open, and a seconds clock so "updated 12s ago" and the
    // countdowns move.
    onOpenChanged: if (root.open) AiUsage.refresh()
    Timer {
        interval: 1000
        running: root.open
        repeat: true
        triggeredOnStart: true
        onTriggered: AiUsage.now = Date.now() / 1000
    }

    function levelColor(severity) {
        return severity === "critical" ? Theme.error : severity === "high" ? root.warn : Theme.primary;
    }

    function paceLabel(p) {
        if (!p)
            return "";
        if (p.points === 0)
            return "On pace";
        return p.points > 0 ? "%1 pts over pace".arg(p.points) : "%1 pts under pace".arg(-p.points);
    }

    function resetLabel(ts) {
        const left = Calc.countdown(ts, AiUsage.now);
        return left === "" ? "" : "resets in %1".arg(left);
    }

    // selected tab; falls back to the first provider reported.
    property string tab: ""
    readonly property var current: AiUsage.providers.find(p => p.id === root.tab) || AiUsage.providers[0] || null

    // a missing credentials file means the CLI was never signed in.
    function errorLabel(p) {
        if (/No such file/.test(p.error))
            return "Not signed in. Run `%1` to log in.".arg(p.id === "openai" ? "codex" : "claude");
        return p.error;
    }

    function brand(id) {
        return /anthropic|claude/.test(id) ? "assets/claude.svg"
             : /openai|codex/.test(id) ? "assets/codex.svg" : "";
    }

    implicitWidth: 300 * root.s
    implicitHeight: content.implicitHeight + root.pad * 2

    PopoutCard { anchors.fill: parent }

    Column {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: root.pad
        spacing: 0

        // header: the 力 seal and title, and how long ago the numbers were fetched
        Item {
            width: parent.width
            height: title.implicitHeight
            BrandMark {
                id: seal
                anchors.verticalCenter: title.verticalCenter
                size: 13 * root.s
                color: Theme.vermLit
            }
            Head {
                id: title
                anchors.left: seal.right
                anchors.leftMargin: 7 * root.s
                text: "AI usage"
                font.pixelSize: 14 * root.s
            }
            Small {
                anchors.right: reload.left
                anchors.rightMargin: 6 * root.s
                anchors.baseline: title.baseline
                text: AiUsage.refreshing ? "updating…"
                    : (AiUsage.stale ? "stale · " : "") + "updated %1".arg(Calc.ago(AiUsage.updated, AiUsage.now))
                visible: AiUsage.updated > 0 || AiUsage.refreshing
                color: AiUsage.stale ? root.warn : root.dim
                font.pixelSize: 10 * root.s
            }
            PopoutHeadButton {
                id: reload
                anchors.right: parent.right
                anchors.verticalCenter: title.verticalCenter
                s: root.s
                glyph: "restart_alt"
                spin: AiUsage.refreshing
                onClicked: AiUsage.refresh()
            }
        }

        Item { width: 1; height: 12 * root.s }

        // tabs: one pill per provider, the selected one filled
        Row {
            visible: AiUsage.providers.length > 0
            width: parent.width
            spacing: 6 * root.s
            bottomPadding: 12 * root.s

            Repeater {
                model: AiUsage.providers
                delegate: Rectangle {
                    id: tabItem
                    required property var modelData
                    readonly property bool active: root.current !== null && root.current.id === modelData.id

                    width: (content.width - 6 * root.s * (AiUsage.providers.length - 1)) / AiUsage.providers.length
                    height: 28 * root.s
                    radius: height / 2
                    color: tabItem.active ? root.line : tabArea.containsMouse ? Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.07) : "transparent"
                    border.width: Theme.borderWidth
                    border.color: root.line

                    Row {
                        anchors.centerIn: parent
                        spacing: 6 * root.s
                        Image {
                            visible: source != ""
                            anchors.verticalCenter: parent.verticalCenter
                            width: 13 * root.s
                            height: width
                            source: root.brand(tabItem.modelData.id)
                            fillMode: Image.PreserveAspectFit
                            smooth: true
                            opacity: tabItem.active ? 1 : 0.6
                        }
                        Head {
                            anchors.verticalCenter: parent.verticalCenter
                            text: tabItem.modelData.name
                            color: tabItem.active ? root.ink : root.dim
                        }
                    }
                    MouseArea {
                        id: tabArea
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.tab = tabItem.modelData.id
                    }
                }
            }
        }

        Small {
            width: parent.width
            visible: !AiUsage.installed || (AiUsage.providers.length === 0 && !AiUsage.refreshing)
            wrapMode: Text.Wrap
            text: !AiUsage.installed ? "Install ai-usagebar (AUR: ai-usagebar-bin)."
                : AiUsage.error !== "" ? AiUsage.error
                : "No usage yet. Sign in to claude or codex."
        }

        Repeater {
            model: root.current ? [root.current] : []
            delegate: Column {
                id: prov
                required property var modelData

                width: content.width
                spacing: 8 * root.s

                // plan · account
                Small {
                    visible: text !== ""
                    width: parent.width
                    elide: Text.ElideLeft
                    text: [prov.modelData.plan, prov.modelData.account].filter(t => t !== "").join(" · ")
                    font.pixelSize: 10 * root.s
                }

                Small {
                    visible: prov.modelData.error !== ""
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: root.errorLabel(prov.modelData)
                    color: root.warn
                }

                Repeater {
                    model: prov.modelData.metrics
                    delegate: Column {
                        id: row
                        required property var modelData

                        readonly property bool gauge: modelData.pct >= 0
                        readonly property var pace: Calc.pace(modelData.pct, modelData.reset, modelData.window, AiUsage.now)
                        readonly property color accent: root.levelColor(modelData.severity)

                        width: prov.width
                        spacing: 6 * root.s

                        // window label on the left; percent (or amount) on the right
                        Item {
                            width: parent.width
                            height: big.implicitHeight
                            Head {
                                anchors.left: parent.left
                                anchors.right: big.left
                                anchors.rightMargin: 8 * root.s
                                anchors.verticalCenter: parent.verticalCenter
                                elide: Text.ElideRight
                                text: row.modelData.label
                                font.weight: Font.Medium
                            }
                            Small {
                                id: big
                                anchors.right: parent.right
                                text: row.gauge ? Math.round(row.modelData.pct) + "%" : row.modelData.value
                                color: row.accent
                                font.pixelSize: 18 * root.s
                                font.weight: Font.Medium
                                font.features: ({ "tnum": 1 })
                            }
                        }

                        // gauge, with a faint tick where even spending would be by now
                        Item {
                            visible: row.gauge
                            width: parent.width
                            height: 12 * root.s
                            Rectangle {
                                anchors.verticalCenter: parent.verticalCenter
                                width: parent.width
                                height: 6 * root.s
                                radius: height / 2
                                color: root.line
                                Rectangle {
                                    width: Math.max(parent.height, parent.width * Math.min(1, row.modelData.pct / 100))
                                    visible: row.modelData.pct > 0
                                    height: parent.height
                                    radius: height / 2
                                    color: row.accent
                                    Behavior on width { NumberAnimation { duration: 300; easing.type: Easing.OutCubic } }
                                }
                            }
                            Rectangle {
                                visible: !!row.pace
                                x: row.pace ? Math.max(0, Math.min(parent.width - width, parent.width * row.pace.expected / 100 - width / 2)) : 0
                                width: 2 * root.s
                                height: parent.height
                                radius: width / 2
                                color: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.55)
                            }
                        }

                        // pace in points on the left, countdown to the reset on the right
                        Item {
                            visible: paceText.text !== "" || resetText.text !== ""
                            width: parent.width
                            height: paceText.implicitHeight
                            Small {
                                id: paceText
                                text: root.paceLabel(row.pace)
                                // under pace is good news (accent); over is neutral
                                // until the severity says otherwise.
                                color: !row.pace || row.pace.points <= 0 ? Theme.primary
                                    : row.modelData.severity === "high" || row.modelData.severity === "critical" ? row.accent : root.ink
                                font.weight: Font.Medium
                            }
                            Small {
                                id: resetText
                                anchors.right: parent.right
                                anchors.baseline: paceText.baseline
                                text: root.resetLabel(row.modelData.reset)
                            }
                        }
                    }
                }

                // label/value notes: credit balances, extra usage
                Repeater {
                    model: prov.modelData.notes
                    delegate: Item {
                        id: note
                        required property var modelData
                        width: prov.width
                        height: noteValue.implicitHeight
                        Small {
                            anchors.left: parent.left
                            anchors.right: noteValue.left
                            anchors.rightMargin: 8 * root.s
                            elide: Text.ElideRight
                            text: note.modelData.label
                        }
                        Small {
                            id: noteValue
                            anchors.right: parent.right
                            text: note.modelData.value
                            color: root.ink
                        }
                    }
                }
            }
        }
    }
}
