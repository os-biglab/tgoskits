#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../../.." && pwd)"
cd "$ROOT_DIR"

echo "[host-memory-allocators-bundle] reproduce"

cargo test -p axallocator
cargo test -p bitmap-allocator
cargo test -p range-alloc-arceos
