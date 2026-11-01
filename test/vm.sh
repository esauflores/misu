#!/usr/bin/env bash
# Arch test VM under KVM. The first run (or --iso) boots the Arch ISO for archinstall;
# later runs boot the disk. The repo is shared live over 9p (tag "rice"), guest SSH
# is on 127.0.0.1:2222 and QMP on $dir/qmp.sock. Extra arguments go to qemu.
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
dir=${RICE_VM_DIR:-$HOME/.cache/rice-vm}
disk=$dir/arch.qcow2
iso=$dir/archlinux-x86_64.iso
ovmf=/usr/share/edk2/x64

[[ -w /dev/kvm ]] || { echo '/dev/kvm is missing or not writable' >&2; exit 1; }
mkdir -p "$dir"
[[ -f $dir/OVMF_VARS.fd ]] || cp "$ovmf/OVMF_VARS.4m.fd" "$dir/OVMF_VARS.fd"
[[ -f $disk ]] || { qemu-img create -f qcow2 "$disk" 40G; set -- --iso "$@"; }

cdrom=()
if [[ ${1:-} == --iso ]]; then
  shift
  # Downloads to .part and resumes it; a stalled transfer (under 1 KB/s for a
  # minute) retries instead of hanging. RICE_VM_MIRROR picks a closer mirror.
  mirror=${RICE_VM_MIRROR:-https://geo.mirror.pkgbuild.com}/iso/latest
  until [[ -f $iso ]]; do
    curl -fL -C - --speed-limit 1024 --speed-time 60 -o "$iso.part" "$mirror/archlinux-x86_64.iso" || continue
    sum=$(curl -fsS "$mirror/sha256sums.txt" | awk '$2 == "archlinux-x86_64.iso" { print $1 }')
    if [[ $(sha256sum "$iso.part" | cut -d' ' -f1) == "$sum" ]]; then mv "$iso.part" "$iso"
    else echo 'ISO checksum mismatch; downloading again' >&2; rm -f "$iso.part"; fi
  done
  cdrom=(-drive "if=none,id=cd,media=cdrom,readonly=on,file=$iso" -device ide-cd,drive=cd,bootindex=0)
fi

exec qemu-system-x86_64 \
  -name rice-vm -machine q35,accel=kvm -cpu host -smp 4 -m 8G \
  -drive "if=pflash,format=raw,readonly=on,file=$ovmf/OVMF_CODE.4m.fd" \
  -drive "if=pflash,format=raw,file=$dir/OVMF_VARS.fd" \
  -drive "if=none,id=hd,format=qcow2,discard=unmap,file=$disk" -device virtio-blk-pci,drive=hd,bootindex=1 \
  "${cdrom[@]}" \
  -device virtio-vga-gl -display gtk,gl=on \
  -device virtio-tablet-pci -device virtio-keyboard-pci \
  -nic user,model=virtio-net-pci,hostfwd=tcp:127.0.0.1:2222-:22 \
  -virtfs "local,path=$ROOT,mount_tag=rice,security_model=none" \
  -audiodev pipewire,id=snd -device intel-hda -device hda-duplex,audiodev=snd \
  -qmp "unix:$dir/qmp.sock,server=on,wait=off" \
  "$@"
