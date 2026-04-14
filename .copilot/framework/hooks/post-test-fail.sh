#!/usr/bin/env bash
set -euo pipefail

run_dir="${RUN_DIR:-}"
stage_name="${STAGE_NAME:-unknown}"

if [[ -z "${run_dir}" ]]; then
  echo "RUN_DIR is required" >&2
  exit 1
fi

echo "post-test-fail hook: stage=${stage_name} run_dir=${run_dir}"

