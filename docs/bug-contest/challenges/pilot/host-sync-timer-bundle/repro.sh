#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../../.." && pwd)"
cd "$ROOT_DIR"

echo "[host-sync-timer-bundle] reproduce"

cargo test -p timer_list --test exact_deadline
