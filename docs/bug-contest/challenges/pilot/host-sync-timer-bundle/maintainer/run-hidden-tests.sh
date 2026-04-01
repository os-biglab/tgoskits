#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../../../.." && pwd)"
SOURCE="$ROOT_DIR/docs/bug-contest/challenges/pilot/host-sync-timer-bundle/maintainer/hidden-tests/timer_list_hidden.rs"
TARGET="$ROOT_DIR/components/timer_list/tests/__contest_hidden_timer_list.rs"

cleanup() {
  rm -f "$TARGET"
}

trap cleanup EXIT

cp "$SOURCE" "$TARGET"
cd "$ROOT_DIR"
cargo test -p timer_list --test __contest_hidden_timer_list
