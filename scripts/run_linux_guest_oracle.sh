#!/usr/bin/env bash
# Run a static riscv64 probe ELF as PID 1 in an initramfs under qemu-system-riscv64.
# Requires: STARRY_LINUX_GUEST_IMAGE (riscv64 Linux Image or vmlinuz), cpio+gzip, timeout(1), qemu-system-riscv64.
#
# Usage: STARRY_LINUX_GUEST_IMAGE=/path/to/Image scripts/run_linux_guest_oracle.sh /path/to/probe_elf
# Env: QEMU_SYSTEM_RISCV64 (default qemu-system-riscv64), STARRY_LINUX_GUEST_TIMEOUT sec (default 90),
#      STARRY_LINUX_GUEST_APPEND (extra kernel cmdline, optional)
#
# Serial output (including kernel + probe) is printed to stdout; callers typically grep -m1 '^CASE '.
set -euo pipefail

ELF="${1:?usage: $0 <static-riscv64-probe-elf>}"
KERNEL="${STARRY_LINUX_GUEST_IMAGE:?STARRY_LINUX_GUEST_IMAGE is not set (see docs/starryos-linux-guest-oracle-pin.md)}"
QEMU="${QEMU_SYSTEM_RISCV64:-qemu-system-riscv64}"
TIMEOUT="${STARRY_LINUX_GUEST_TIMEOUT:-90}"
EXTRA_APPEND="${STARRY_LINUX_GUEST_APPEND:-}"

if [ ! -f "$ELF" ]; then
  echo "run_linux_guest_oracle: not a file: $ELF" >&2
  exit 1
fi
if [ ! -f "$KERNEL" ]; then
  echo "run_linux_guest_oracle: kernel not found: $KERNEL" >&2
  exit 1
fi
if ! command -v "$QEMU" >/dev/null 2>&1; then
  echo "run_linux_guest_oracle: missing $QEMU" >&2
  exit 1
fi

td="$(mktemp -d)"
trap 'rm -rf "$td"' EXIT
cp "$ELF" "$td/init"
chmod +x "$td/init"
(
  cd "$td"
  # List archive member names on stdin (portable vs find+cpio --null).
  echo init | cpio -o -H newc 2>/dev/null | gzip -9 >"$td/initrd.gz"
)

append="console=ttyS0 earlycon=sbi quiet loglevel=3 rdinit=/init"
if [ -n "$EXTRA_APPEND" ]; then
  append="$append $EXTRA_APPEND"
fi

set +e
out="$(
  timeout "$TIMEOUT" "$QEMU" \
    -machine virt \
    -cpu rv64 \
    -smp 1 \
    -m 256M \
    -nographic \
    -kernel "$KERNEL" \
    -initrd "$td/initrd.gz" \
    -append "$append" 2>&1
)"
rc=$?
set -e

printf '%s\n' "$out"

if [ "$rc" -eq 124 ]; then
  echo "run_linux_guest_oracle: qemu timed out after ${TIMEOUT}s" >&2
  exit 124
fi
# Guest probe may exit non-zero; still print output for CASE extraction.
exit 0
