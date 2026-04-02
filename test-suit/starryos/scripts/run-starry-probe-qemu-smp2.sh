#!/usr/bin/sh
# Same as run-starry-probe-qemu.sh but uses test-suit/starryos/qemu-riscv64-smp2.toml (-smp 2).
# Usage: ./run-starry-probe-qemu-smp2.sh <probe_basename> [extra cargo xtask args...]
set -eu
WS="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$WS"

probe="${1:?usage: $0 <probe_basename e.g. write_stdout>}"
shift

if [ "$probe" = write_stdout ]; then
  IMG="$WS/target/riscv64gc-unknown-none-elf/rootfs-riscv64-probe.img"
else
  IMG="$WS/target/riscv64gc-unknown-none-elf/rootfs-riscv64-probe-${probe}.img"
fi

"$WS/test-suit/starryos/scripts/prepare-rootfs-with-probe.sh" "$probe"

exec cargo xtask starry test qemu --target riscv64 \
  --qemu-config "$WS/test-suit/starryos/qemu-riscv64-smp2.toml" \
  --test-disk-image "$IMG" \
  --shell-init-cmd "$WS/test-suit/starryos/testcases/probe-${probe}-0" \
  --timeout 120 \
  "$@"
