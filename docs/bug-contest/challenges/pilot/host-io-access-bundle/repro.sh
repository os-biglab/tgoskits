#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../../.." && pwd)"
cd "$ROOT_DIR"

echo "[host-io-access-bundle] reproduce"

cargo test -p cap_access --test required_capabilities
