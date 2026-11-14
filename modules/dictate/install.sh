# dictate: live subtitles of speech, saved as text (misu-dictate, Super+Shift+Z),
# transcribed on the laptop by Kroko's streaming models (misu-dictate-server,
# run through uv from apps' mise). Not installed by default:
#   sudo ./install.sh dictate
# The bar's plugins/dictate shows its rail button only once misu-dictate is on
# PATH (after the next login or bar reload).
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" "${d##*/}" "$@"; }

install_packages dictate

for f in "$M"/dictate/bin/*; do link "$f" "$home/.local/bin/${f##*/}"; done

# Super+Shift+Z is in wm/config.json and checks whether misu-dictate exists.
