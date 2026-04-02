#!/usr/bin/env bash
# Regenerate test-suit/starryos/probes/expected/guest-alpine323/*.line for every
# built probe ELF (requires STARRY_LINUX_GUEST_IMAGE + qemu-system-riscv64).
#
# Usage (from repo root):
#   export STARRY_LINUX_GUEST_IMAGE=/path/to/riscv64/Image
#   CC=riscv64-linux-musl-gcc ./scripts/refresh_guest_oracle_expected.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

KERNEL="${STARRY_LINUX_GUEST_IMAGE:?set STARRY_LINUX_GUEST_IMAGE (see docs/starryos-linux-guest-oracle-pin.md)}"
GUEST_SCRIPT="$ROOT/scripts/run_linux_guest_oracle.sh"
OUT="${PROBE_OUT:-$ROOT/test-suit/starryos/probes/build-riscv64}"
DEST="$ROOT/test-suit/starryos/probes/expected/guest-alpine323"
CC="${CC:-riscv64-linux-musl-gcc}"

test -f "$GUEST_SCRIPT" || { echo "missing $GUEST_SCRIPT" >&2; exit 1; }
command -v "$CC" >/dev/null || { echo "missing $CC" >&2; exit 1; }
mkdir -p "$DEST"

sh "$ROOT/test-suit/starryos/scripts/build-probes.sh"

n=0
for elf in "$OUT"/*; do
  [ -f "$elf" ] && [ -x "$elf" ] || continue
  base=$(basename "$elf")
  # Skip non-probe junk if any
  case "$base" in
    *.o) continue ;;
  esac
  got="$(env STARRY_LINUX_GUEST_IMAGE="$KERNEL" bash "$GUEST_SCRIPT" "$elf" 2>/dev/null | tr -d '\r' | grep -m1 '^CASE ' || true)"
  if [ -z "$got" ]; then
    echo "SKIP $base: no CASE line (guest boot failure?)" >&2
    continue
  fi
  printf '%s\n' "$got" >"$DEST/${base}.line"
  echo "OK $base -> $DEST/${base}.line"
  n=$((n + 1))
done

echo "Wrote $n guest oracle lines under $DEST"
