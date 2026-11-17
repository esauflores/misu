# apps: removes the links and Brave Origin's policies.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" --uninstall "${d##*/}" "$@"; }

# Archify's skill copies (Claude Code and Codex), while mise's npx is still there.
as_user env HOME="$home" bash -c 'cd && mise exec -- npx -y skills@latest remove archify --global --yes' || true
unlink_ours "$home/.config/herdr/config.toml"
unlink_ours "$home/.config/mimeapps.list"
unlink_ours "$home/.config/mise/config.toml"
for app in qv4l2 qvidcap qt6ct assistant designer linguist qdbusviewer footclient foot-server limine-snapper-restore; do unlink_ours "$home/.local/share/applications/$app.desktop"; done
unlink_ours "$home/.local/share/applications/dev.zed.Zed.desktop"
# Zed's theme link; its settings.json is the user's, Misu's merged keys stay.
if [[ -L $home/.config/zed/themes/misu.json ]]; then as_user rm "$home/.config/zed/themes/misu.json"; fi
# btop's palette theme (the link into ~/.local/state/rice); btop.conf is the user's.
if [[ -L $home/.config/btop/themes/misu.theme ]]; then as_user rm "$home/.config/btop/themes/misu.theme"; fi
unlink_ours "$home/.config/brave-origin-flags.conf"
run rm -f /etc/brave/policies/managed/misu.json

remove_packages apps
