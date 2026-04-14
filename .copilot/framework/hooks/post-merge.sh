#!/usr/bin/env bash
set -euo pipefail

run_dir="${RUN_DIR:-}"

if [[ -z "${run_dir}" ]]; then
  echo "RUN_DIR is required" >&2
  exit 1
fi

echo "post-merge hook for ${run_dir}"

