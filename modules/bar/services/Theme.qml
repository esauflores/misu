pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io

// Misu's palette and type scale. Colours are Material roles read from the
// matugen output at $XDG_STATE_HOME/rice/misu-colors.json (template in the
// rice repo: modules/theme/matugen/templates/misu-colors.json), reloaded live when
// matugen rewrites it; any role the file lacks falls back to the defaults below.
Singleton {
    id: root

    readonly property var defaults: ({
        "surface": "#121318",
        "inverseSurface": "#e3e1e9",
        "inverseOnSurface": "#2f3036",
        "primary": "#b5c4ff",
        "error": "#ffb4ab",
        "outline": "#8f909a",
        "outlineVariant": "#45464f",
        "onSurface": "#e3e1e9",
        "onSurfaceVariant": "#c6c6d0",
        "onPrimary": "#1c2d61"
    })
    property var palette: ({})

    function role(key) {
        const v = root.palette[key];
        return (typeof v === "string" && v.length > 0) ? v : root.defaults[key];
    }

    readonly property string colorsPath: (Quickshell.env("XDG_STATE_HOME") || (Quickshell.env("HOME") + "/.local/state"))
        + "/rice/misu-colors.json"

    FileView {
        path: root.colorsPath
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: {
            try {
                root.palette = JSON.parse(text());
            } catch (e) {
                console.warn("misu: bad colors file " + root.colorsPath + ": " + e);
            }
        }
        onLoadFailed: root.palette = ({})
    }

    readonly property color surface: role("surface")
    readonly property color inverseSurface: role("inverseSurface")
    readonly property color inverseOnSurface: role("inverseOnSurface")
    readonly property color primary: role("primary")
    readonly property color error: role("error")
    readonly property color outline: role("outline")
    readonly property color outlineVariant: role("outlineVariant")
    readonly property color surfaceText: role("onSurface")
    readonly property color surfaceVariantText: role("onSurfaceVariant")
    readonly property color primaryText: role("onPrimary")

    // Accent tints derived from primary.
    readonly property color vermLit:     Qt.lighter(primary, 1.22)
    readonly property color vermDim:     Qt.darker(primary, 1.4)
    readonly property color vermDimDeep: Qt.darker(primary, 2.2)
    readonly property color threadBg:    Qt.rgba(primary.r, primary.g, primary.b, 0.13)
    readonly property color ghost:       Qt.darker(surfaceVariantText, 1.6)

    readonly property real windowOpacity: Config.frameOpacity
    // The colour actually under the bar's glyphs: the surface, composited
    // over black when the frame is translucent.
    readonly property color effectiveSurface: windowOpacity >= 0.999 ? surface
        : Qt.rgba(surface.r * windowOpacity, surface.g * windowOpacity, surface.b * windowOpacity, 1)

    function luminance(c) {
        const f = v => v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
        return 0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b);
    }
    function contrast(a, b) {
        const la = luminance(a), lb = luminance(b);
        return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
    }
    // Readable ink for a background: surfaceText when it contrasts, else black/white.
    function ink(bg) {
        if (contrast(surfaceText, bg) >= 4.5) return surfaceText;
        return luminance(bg) > 0.4 ? "#101010" : "#f0f0f0";
    }
    // `want` on `bg` when it reaches `minRatio` (default 4.5), else ink(bg).
    function inkOn(bg, want, minRatio) {
        return contrast(want, bg) >= (minRatio || 4.5) ? want : ink(bg);
    }

    readonly property int radiusWidget: 8
    readonly property int radiusWindow: 8
    readonly property int borderWidth: 2
    readonly property int paddingMd: 8
    readonly property int iconSm: 16

    readonly property string fontPrimary: Config.fontFamily
    readonly property string fontJp: "Noto Sans CJK JP"
    readonly property int fontSm: 14
    readonly property int fontMd: 16

    // The brand seal drawn by BrandMark: text, or an image path when set.
    readonly property string mark: Config.markText
    readonly property string markSource: Config.markImage
    readonly property bool markTint: true
}
