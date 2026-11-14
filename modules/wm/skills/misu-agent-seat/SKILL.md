---
name: misu-agent-seat
description: Drive GUI apps on Misu through an independent wm seat, with its own focus, pointer, workspace, input and screenshots. Use when a task needs a GUI app.
---

# An independent seat

You are seat 1 (`MISU_AGENT_SEAT=1`) unless the user or whoever started you
says you have another seat; then set `MISU_AGENT_SEAT` to that number. Seats
count from 0 and seat 0 belongs to the user (the user's "first seat" is seat 0,
yours is seat 1). Check `wm msg tree` before acting. Coordinate seat allocation with other agents;
use only your assigned seat. For more concurrent seats, increase `seats` in
`~/.config/wm/config.json` and restart the session. Do not change the user's
seat or run `next-seat`. No headless output, uinput, or pointer daemon is needed.

Move your seat to a workspace the user is not using before opening apps:

```sh
misu-ai-workspace 9
misu-ai-app open foot
misu-ai-app open nautilus
misu-ai-app list
misu-ai-app focus WINDOW_ID
misu-ai-shot /tmp/seat.png
misu-ai-pointer click 300 200
misu-ai-pointer type 'hello world'
misu-ai-pointer key ctrl+shift+t
misu-ai-pointer scroll 0 3
misu-ai-pointer drag 100 100 400 300
misu-agent-seat animations off
```

Inspect screenshots before choosing coordinates; they show this seat's view,
including windows off the user's screen. Focus windows by their IDs from tree.
`open` takes an executable command (not an app search or a desktop entry name).
IPC replies are JSON: check `error` and the command's exit status. Spawn replies
confirm process launch, not that an app opened a window; inspect tree afterward.

An app sees only the seat that launched it: what you open with `misu-ai-app
open` takes your seat's input and only yours, and the user's apps never get
your clicks or keys (the user cannot type into yours either). So open every app
you need to drive yourself; you can see the user's windows in screenshots but
not operate them. Apps that bind one seat (Electron, kitty, Zed) work this way
too. Apps nobody's seat launched (systemd services, portals) see every seat.
X11 through xwayland-satellite has one seat: use native Wayland apps.
Single-instance apps (Zed, Brave, Nautilus) may hand your request to the
user's running process, which then opens the window as the user's; start a
separate instance (e.g. a separate profile or `--new`/user-data-dir flag) or
pick another app, and do not take over a user's existing window.

Over SSH, import the session environment from the user's systemd manager:
`export $(systemctl --user show-environment | grep -E '^(WAYLAND_DISPLAY|WM_SOCK|DISPLAY)=')`.
