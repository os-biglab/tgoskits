#!/usr/bin/sh
# Local end-to-end: ensure riscv64 rootfs, inject probe, run starryos-test in QEMU via xtask.
# Not run in default CI (downloads rootfs, needs qemu-system from xtask stack).
#
# Prereq: Rust workspace + cargo xtask; riscv64-linux-musl-gcc (or set CC); network for first rootfs fetch.
# Usage (from repo root):
#   ./test-suit/starryos/scripts/run-e2e-probe-smoke.sh [probe_basename]
# Default probe: write_stdout
set -eu
WS="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$WS"

probe="${1:-write_stdout}"

echo "== [e2e] cargo xtask starry rootfs --arch riscv64 (may download) =="
cargo xtask starry rootfs --arch riscv64

echo "== [e2e] inject probe + QEMU test (probe=$probe) =="
exec "$WS/test-suit/starryos/scripts/run-starry-probe-qemu.sh" "$probe"
