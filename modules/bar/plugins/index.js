.pragma library

// Misu's built-in plugins, one folder each under plugins/. A plugin provides a
// Rail.qml for the rail ids it claims (rail/RailWidget loads it in place of a
// built-in widget) and, when `popout` is set, a Popout.qml card that
// FrameSurface mounts for the surface named after the folder. `minWidth` is
// the card's narrowest width before scaling; `keyboard` the focus the card
// needs while open ("exclusive" to take it on open), none when unset; a card
// with a text field it opens only sometimes leaves it unset and sets
// `wantsKeyboard` on its body instead (network).
// Adding a plugin = a new folder plus one line here.
const plugins = {
    "quicksettings": { rail: ["quick-settings"] },
    "audio": { rail: ["audio-output", "audio-input"], popout: { minWidth: 272 } },
    "battery": { rail: ["battery"], popout: { minWidth: 244 } },
    "calendar": { rail: ["clock"], popout: { minWidth: 272 } },
    "bluetooth": { rail: ["bluetooth"], popout: { minWidth: 272 } },
    "network": { rail: ["network"], popout: { minWidth: 250 } },
    "notifications": { rail: ["notifications"], popout: { minWidth: 300 } },
    "mpris": { rail: ["mpris"], popout: { minWidth: 340 } },
    "aiusage": { rail: ["ai-usage"], popout: { minWidth: 280 } },
    // Live subtitles (misu-dictate): computer audio, mic or both; Super+Shift+Z.
    "dictate": { rail: ["dictate"], popout: { minWidth: 260 } },
    // No rail button: Super+Space opens it over IPC.
    "launcher": { rail: [], popout: { minWidth: 360, keyboard: "exclusive" } },
    // Clipboard history (cliphist); no rail button, Super+Shift+V opens it.
    "clipboard": { rail: [], popout: { minWidth: 360, keyboard: "exclusive" } },
    // Wallpaper grid (misu-wall); no rail button, Super+Shift+Q opens it.
    "wallpaper": { rail: [], popout: { minWidth: 400, keyboard: "exclusive" } },
    // Lock, suspend, log out, restart, power off; Super+Esc.
    "session": { rail: [], popout: { minWidth: 260, keyboard: "exclusive" } },
    // What to share on a screen share; no rail button, the portal opens it.
    "share": { rail: [], popout: { minWidth: 360, keyboard: "exclusive" } }
};

// Folder of the plugin that draws rail widget `id`, or "".
function railPlugin(id) {
    for (const name in plugins)
        if (plugins[name].rail.indexOf(id) >= 0)
            return name;
    return "";
}

// True when surface `kind` is a plugin card (plugins/<kind>/Popout.qml).
function hasPopout(kind) {
    return !!(plugins[kind] && plugins[kind].popout);
}

// One record per plugin card (CardManager slides it out of the rail).
function surfaceRecords() {
    const out = [];
    for (const name in plugins)
        if (plugins[name].popout)
            out.push({ id: name, kind: name, minWidth: plugins[name].popout.minWidth, keyboard: plugins[name].popout.keyboard || "" });
    return out;
}
