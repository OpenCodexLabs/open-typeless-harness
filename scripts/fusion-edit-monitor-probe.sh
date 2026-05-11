#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root/openless-all/app/src-tauri"

if [[ "${1:-}" == "--wait" ]]; then
  wait_secs="${2:-}"
  if [[ -z "$wait_secs" || ! "$wait_secs" =~ ^[0-9]+$ ]]; then
    echo "usage: $0 [--wait SECONDS] [--pid PID] [original_text] [inserted_text]" >&2
    exit 2
  fi
  shift 2
  echo "Focus the target text field now. Starting probe in ${wait_secs}s..."
  sleep "$wait_secs"
fi

exec cargo run -q --bin edit_monitor_probe -- "$@"
