#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../../.." && pwd)"
cd "$ROOT_DIR"

echo "[arceos-axtask-dedicated] reproduce"

cargo arceos test qemu --target riscv64gc-unknown-none-elf
