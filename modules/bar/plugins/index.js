.pragma library

// Misu's built-in plugins, one folder each under plugins/. A plugin provides a
// Rail.qml for the rail ids it claims (rail/RailWidget loads it in place of a
// built-in widget) and, when `popout` is set, a Popout.qml card that
// FrameSurface mounts for the surface named after the folder. `minWidth` is
// the card's narrowest width before scaling; `keyboard` the focus the card
// needs while open ("ondemand" for a text field), none when unset.
// Adding a plugin = a new folder plus one line here.
const plugins = {
    "quicksettings": { rail: ["quick-settings"] },
    "audio": { rail: ["audio-output", "audio-input"], popout: { minWidth: 300 } },
    "battery": { rail: ["battery"], popout: { minWidth: 244 } },
    "calendar": { rail: ["clock"], popout: { minWidth: 272 } },
    "bluetooth": { rail: ["bluetooth"], popout: { minWidth: 272 } },
    "network": { rail: ["network"], popout: { minWidth: 250, keyboard: "ondemand" } },
    "notifications": { rail: ["notifications"], popout: { minWidth: 300 } },
    "mpris": { rail: ["mpris"], popout: { minWidth: 340 } },
    "aiusage": { rail: ["ai-usage"], popout: { minWidth: 280 } }
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
