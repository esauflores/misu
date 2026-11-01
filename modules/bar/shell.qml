//@ pragma UseQApplication
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import qs.services
import "frame"
import "plugins/notifications" as Notifications
import "plugins/osd" as Osd

// Misu: an ink rail on the right edge of every output, for scroll. Run with
// `qs -c misu` (or `qs -p <this dir>`). IPC:
//   qs -c misu ipc call bar toggle
//   qs -c misu ipc call menu toggle <card>  (any plugins/<card>: audio, battery,
//                                           bluetooth, calendar, aiusage,
//                                           mpris, network, notifications)
ShellRoot {
    id: shell

    Component.onCompleted: console.info("misu: loaded")

    Variants {
        model: Quickshell.screens
        Frame {}
    }

    Variants {
        model: Quickshell.screens
        Notifications.Popups {}
    }

    Variants {
        model: Quickshell.screens
        Osd.Popups {}
    }

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
}
