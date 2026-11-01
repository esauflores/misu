# apps: herdr, default applications, Zen defaults for every profile, VS Code
# defaults merged into existing settings, AI CLIs.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" "${d##*/}" "$@"; }

# The repo's `code` conflicts with visual-studio-code-bin.
if installed code; then as_user yay -Rns --noconfirm code; fi
install_packages apps

link "$M/apps/herdr/config.toml" "$home/.config/herdr/config.toml"
link "$M/apps/mimeapps.list" "$home/.config/mimeapps.list"
run install -Dm644 "$M/apps/zen/misu.js" /opt/zen-browser-bin/browser/defaults/preferences/misu.js

vscode_settings="$home/.config/Code/User/settings.json"
if (( DRY_RUN )); then
  echo "+ merge $M/apps/vscode/settings.json into $vscode_settings"
elif [[ -f $vscode_settings ]]; then
  if as_user jq -e . "$vscode_settings" >/dev/null 2>&1; then
    as_user bash -c 'tmp=$(mktemp); jq -s ".[0] * .[1]" "$1" "$2" >"$tmp" && cat "$tmp" >"$1"; status=$?; rm -f "$tmp"; exit "$status"' _ "$vscode_settings" "$M/apps/vscode/settings.json"
  else
    echo "Skipping VS Code defaults: $vscode_settings is not valid JSON" >&2
  fi
else
  as_user install -Dm644 "$M/apps/vscode/settings.json" "$vscode_settings"
fi
