# intel: graphics drivers and firmware for Intel hardware (Iris Xe GPU, Wi-Fi,
# Bluetooth, SOF audio). Swap this module out on other hardware.
[[ ${BASH_SOURCE[0]} != "$0" ]] || { d=$(cd -- "$(dirname -- "$0")" && pwd -P); exec "$d/../../install.sh" "${d##*/}" "$@"; }

install_packages intel
