# wm: the compositor, fuzzel, the misu-* commands and their agent skills, and the
# wayland-sessions entry.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" "${d##*/}" "$@"; }

# The repo's 0.8.4 portal starts two ext-capture frames at once. Upstream
# fixes the startup sequence; transition to its AUR package until extra catches up.
if installed xdg-desktop-portal-wlr; then as_user yay -R --noconfirm xdg-desktop-portal-wlr; fi
install_packages wm

# The session starts from the checkout, so it works for every user.
echo "+ install $M/wm/misu.desktop -> /usr/share/wayland-sessions/misu.desktop (@SESSION@=$M/wm/bin/misu-session)"
(( DRY_RUN )) || sed "s|@SESSION@|$M/wm/bin/misu-session|g" "$M/wm/misu.desktop" | install -Dm644 /dev/stdin /usr/share/wayland-sessions/misu.desktop

# Build as the user; put build artifacts outside the shared checkout (9p).
as_user env CARGO_TARGET_DIR="$home/.cache/misu-wm-build" cargo build --release --locked --manifest-path "$ROOT/wm/Cargo.toml"
run install -Dm755 "$home/.cache/misu-wm-build/release/wm" /usr/local/bin/wm

# Retire the old compositor configuration and virtual-input setup.
unlink_ours "$home/.config/scroll"
run rm -f /etc/udev/rules.d/70-misu-uinput.rules /etc/modules-load.d/misu-uinput.conf
run udevadm control --reload
link "$M/wm/config.json" "$home/.config/wm/config.json"
link "$M/wm/fuzzel.ini" "$home/.config/fuzzel/fuzzel.ini"
link "$M/wm/xdpw.conf" "$home/.config/xdg-desktop-portal-wlr/config"
link "$M/wm/portals.conf" "$home/.config/xdg-desktop-portal/portals.conf"
# Links to commands the checkout no longer has (renamed or removed) go first.
for link_ in "$home"/.local/bin/*; do
  if [[ -L $link_ && $(readlink "$link_") == "$M/wm/bin/"* && ! -e $link_ ]]; then as_user rm "$link_"; fi
done
for f in "$M"/wm/bin/*; do
  [[ -f $f && -x $f ]] || continue
  link "$f" "$home/.local/bin/${f##*/}"
done
# Agent skills (skills/<name>/SKILL.md): how AI agents use the misu-* commands,
# e.g. misu-agent-seat for independent input and screenshots. ~/.agents/skills holds them; ~/.claude/skills and
# ~/.codex/skills get a link per skill to it, so the rest of those folders
# stays the user's. Links to skills the checkout no longer has go first.
for link_ in "$home"/.agents/skills/*; do
  [[ -L $link_ && $(readlink "$link_") == "$M/wm/skills/"* && ! -e $link_ ]] || continue
  name=${link_##*/}
  for agent_dir in "$home/.claude/skills" "$home/.codex/skills"; do
    if [[ -L $agent_dir/$name && $(readlink "$agent_dir/$name") == "$link_" ]]; then as_user rm "$agent_dir/$name"; fi
  done
  as_user rm "$link_"
done
for skill in "$M"/wm/skills/*/; do
  skill=${skill%/}
  name=${skill##*/}
  link "$skill" "$home/.agents/skills/$name"
  for agent_dir in "$home/.claude/skills" "$home/.codex/skills"; do link "$home/.agents/skills/$name" "$agent_dir/$name"; done
done
