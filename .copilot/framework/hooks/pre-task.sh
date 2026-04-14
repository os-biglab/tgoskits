#!/usr/bin/env bash
set -euo pipefail

task_manifest="${TASK_MANIFEST:-}"

if [[ -z "${task_manifest}" ]]; then
  echo "TASK_MANIFEST is required" >&2
  exit 1
fi

if [[ ! -f "${task_manifest}" ]]; then
  echo "manifest not found: ${task_manifest}" >&2
  exit 1
fi

echo "pre-task ok: ${task_manifest}"

