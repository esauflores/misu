# wm: removes the session entry, the compositor config and fuzzel links, the misu-* commands
# and their agent skills.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" --uninstall "${d##*/}" "$@"; }

run rm -f /usr/share/wayland-sessions/misu.desktop /usr/local/bin/wm
unlink_ours "$home/.config/wm/config.json"
as_user rm -f "$home/.local/state/rice/wm-config.json"
run rm -f /etc/udev/rules.d/70-misu-uinput.rules /etc/modules-load.d/misu-uinput.conf
run udevadm control --reload
unlink_ours "$home/.config/scroll"
unlink_ours "$home/.config/fuzzel/fuzzel.ini"
unlink_ours "$home/.config/xdg-desktop-portal-wlr/config"
unlink_ours "$home/.config/xdg-desktop-portal/portals.conf"
# Include retired commands whose source disappeared before this uninstall.
for f in "$home"/.local/bin/*; do
  [[ -L $f && $(readlink "$f") == "$M/wm/bin/"* ]] || continue
  unlink_ours "$f"
done
unlink_ours "$home/.local/bin/misu-sidecar"
for skill in "$home"/.agents/skills/*; do
  [[ -L $skill && $(readlink "$skill") == "$M/wm/skills/"* ]] || continue
  name=${skill##*/}
  # the agents' links point at ~/.agents/skills, not into the checkout
  for agent_dir in "$home/.claude/skills" "$home/.codex/skills"; do
    link_=$agent_dir/$name
    if [[ -L $link_ && $(readlink "$link_") == "$home/.agents/skills/$name" ]]; then as_user rm "$link_"; fi
  done
  unlink_ours "$home/.agents/skills/$name"
done

remove_packages wm
