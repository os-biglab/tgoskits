#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
CHALLENGE_ID="${CHALLENGE_ID:-challenge-id}"
TRACK="${TRACK:-host}"

cd "$ROOT_DIR"

echo "[${CHALLENGE_ID}] reproduce in track: ${TRACK}"

case "$TRACK" in
  host)
    cargo test -p example-crate
    ;;
  arceos)
    cargo arceos test qemu --target riscv64gc-unknown-none-elf
    ;;
  starry)
    cargo starry test qemu --target riscv64
    ;;
  axvisor)
    cargo axvisor test qemu --target aarch64
    ;;
  *)
    echo "unknown track: ${TRACK}" >&2
    exit 1
    ;;
esac
