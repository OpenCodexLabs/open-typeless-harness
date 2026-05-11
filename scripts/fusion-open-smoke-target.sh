#!/usr/bin/env bash
set -euo pipefail

duration_secs="${1:-300}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

open "$repo_root/tools/fusion-edit-monitor-target.html"
exec "$repo_root/scripts/fusion-smoke-watch.sh" "$duration_secs"
