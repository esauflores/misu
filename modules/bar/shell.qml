//@ pragma UseQApplication
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import qs.services
import "frame"
import "lock"
import "plugins/notifications" as Notifications
import "plugins/osd" as Osd
import "plugins/dictate" as Dictate
import "plugins/share" as Share

// Misu: an ink rail on the right edge of every output, for wm. Run with
// `qs -c misu` (or `qs -p <this dir>`). IPC:
//   qs -c misu ipc call bar toggle
//   qs -c misu ipc call menu toggle <card>  (any plugins/<card>: audio, battery,
//                                           bluetooth, calendar, aiusage,
//                                           mpris, network, notifications,
//                                           launcher, clipboard, wallpaper,
//                                           session)
ShellRoot {
    id: shell

    Component.onCompleted: console.info("misu: loaded")

    readonly property var screens: Array.from(Quickshell.screens)

    Variants {
        model: shell.screens
        Frame {}
    }

    Variants {
        model: shell.screens
        Notifications.Popups {}
    }

    Variants {
        model: shell.screens
        Osd.Popups {}
    }

    // Dictation subtitles (misu-dictate, Super+Shift+Z).
    Variants {
        model: shell.screens
        Dictate.Subtitles {}
    }

    // The lock screen (IPC target "lock"; misu-lock calls it).
    Lock {}

    IpcHandler {
        target: "menu"
        function toggle(id: string): void { ShellState.toggleMenu(id); }
    }
    IpcHandler {
        target: "bar"
        function toggle(): void { ShellState.toggleBar(); }
    }
    IpcHandler {
        target: "osd"
        function brightness(): void { Feedback.readBrightness(); }
    }
    // A screen share asking what to share (wm's misu-share-pick).
    IpcHandler {
        target: "share"
        function ask(dir: string): void { Share.ShareRequest.ask(dir); }
    }
}
