#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../../../.." && pwd)"
SOURCE="$ROOT_DIR/docs/bug-contest/challenges/pilot/host-io-access-bundle/maintainer/hidden-tests/cap_access_hidden.rs"
TARGET="$ROOT_DIR/components/cap_access/tests/__contest_hidden_cap_access.rs"

cleanup() {
  rm -f "$TARGET"
}

trap cleanup EXIT

cp "$SOURCE" "$TARGET"
cd "$ROOT_DIR"
cargo test -p cap_access --test __contest_hidden_cap_access
