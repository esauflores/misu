# apps: herdr, Zed (defaults merged into its settings), default applications, Brave Origin's flags and policies,
# AI CLIs (from mise) and the Archify skill.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" "${d##*/}" "$@"; }

# bw and the AI CLIs moved to mise (below); drop the packages that used to
# provide them.
for pkg in bitwarden-cli-bin claude-code openai-codex deepseek-harness-bin; do
  if installed "$pkg"; then as_user yay -Rns --noconfirm "$pkg"; fi
done
install_packages apps

link "$M/apps/herdr/config.toml" "$home/.config/herdr/config.toml"
link "$M/apps/mimeapps.list" "$home/.config/mimeapps.list"
# Runtimes (Node LTS, bun, uv, pnpm), Bitwarden's bw and the AI CLIs (claude,
# codex, dsh) come from mise, the shell module's tool manager; `mise up` updates them.
link "$M/apps/mise/config.toml" "$home/.config/mise/config.toml"
as_user env HOME="$home" bash -c 'cd && mise install -y'
# Archify (https://archify.si), the diagram/explainer skill, for Claude Code
# and Codex: a managed copy in ~/.agents/skills (Codex reads it) and
# ~/.claude/skills, tracked in ~/.agents/.skill-lock.json. Re-running updates it.
as_user env HOME="$home" bash -c 'cd && mise exec -- npx -y skills@latest add https://archify.si --skill archify --agent codex --agent claude-code --global --copy --yes'
# Tools other packages pull in, kept out of the launcher: v4l-utils' Qt test
# apps (ffmpeg needs v4l-utils), qt6ct, which the theme module configures,
# qt6-tools' developer apps (QGIS needs the package), and
# foot's client and server entries (Misu runs plain foot); and the boot
# module's snapshot rollback, run from a terminal when needed.
hidden_apps=(qv4l2 qvidcap qt6ct assistant designer linguist qdbusviewer footclient foot-server limine-snapper-restore)
for app in "${hidden_apps[@]}"; do link "$M/apps/hidden.desktop" "$home/.local/share/applications/$app.desktop"; done
# Zed runs on Wayland (wm shows it only its launcher's seat); older installs
# linked an entry over the system one that ran it on X11.
unlink_ours "$home/.local/share/applications/dev.zed.Zed.desktop"
# Zed rewrites settings.json from its UI (replacing a link), so Misu's
# defaults (zed/settings.json: panels on the right, a guide at 120 columns, no
# AI, the extensions it installs, the Misu theme, 0.9 of its font sizes) are
# merged into the user's file; Misu's keys win. The theme is matugen's wallpaper palette.
if (( DRY_RUN )); then
  echo "+ merge $M/apps/zed/settings.json into $home/.config/zed/settings.json"
else
  as_user env HOME="$home" bash -c 'cd && mise exec -- node "$1" "$2" "$3"' _ \
    "$M/apps/zed/merge-settings.js" "$M/apps/zed/settings.json" "$home/.config/zed/settings.json"
fi
link "$home/.local/state/rice/zed-theme.json" "$home/.config/zed/themes/misu.json"
# Brave Origin's policies win over every profile's settings: no built-in
# password manager or card autofill (Bitwarden does both), and these
# extensions, force-installed from the Chrome Web Store: Bitwarden, uBlock Origin
# Lite (the store has no uBlock Origin since MV2 ended; Brave's hosted copy is
# switched on by hand at brave://settings/extensions/v2), Enhancer for YouTube
# and Always active Window.
run install -Dm644 "$M/apps/brave/policies.json" /etc/brave/policies/managed/misu.json
link "$M/apps/brave/brave-origin-flags.conf" "$home/.config/brave-origin-flags.conf"

# btop rewrites btop.conf whenever it quits, so the file stays the user's: set
# only the wallpaper theme and foot's background, adding the keys when btop
# hasn't written its config yet. btop only takes themes from its themes
# folders, so the theme module's matugen output is linked into one.
btop_conf="$home/.config/btop/btop.conf"
btop_theme="$home/.config/btop/themes/misu.theme"
link "$home/.local/state/rice/btop.theme" "$btop_theme"
if (( DRY_RUN )); then
  echo "+ set color_theme=$btop_theme, theme_background=False in $btop_conf"
else
  as_user touch "$btop_conf"
  for kv in "color_theme = \"$btop_theme\"" "theme_background = False"; do
    key=${kv%% *}
    if grep -q "^$key *=" "$btop_conf"; then as_user sed -i "s|^$key *=.*|$kv|" "$btop_conf"
    else printf '%s\n' "$kv" | as_user tee -a "$btop_conf" >/dev/null; fi
  done
fi
