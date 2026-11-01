// Zen defaults for Misu, installed into Zen's defaults/pref so they apply to
// every profile, new ones included. Changing a setting in Zen still wins.
pref("zen.tabs.vertical.right-side", true);
pref("zen.view.compact.enable-at-startup", true);
pref("zen.view.compact.hide-toolbar", true);
// scroll draws no title bars, so no close/minimize buttons either.
pref("zen.view.experimental-no-window-controls", true);
// Dark window scheme (0 dark, 1 light, 2 system).
pref("zen.view.window.scheme", 0);
pref("zen.welcome-screen.seen", true);
// Without this Zen produced no audio stream to PipeWire.
pref("media.cubeb.sandbox", false);
