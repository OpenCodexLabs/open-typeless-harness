#!/usr/bin/env bash
set -euo pipefail

duration_secs="${1:-120}"
jsonl_path="${OPENTYPELESS_EDIT_MONITOR_JSONL_PATH:-$HOME/.openless/opentypeless-edit-monitor.jsonl}"
log_path="$HOME/Library/Logs/OpenTypelessHarness/open-typeless-harness.log"
started_at="$(date +%s)"

line_count() {
  local path="$1"
  if [[ -f "$path" ]]; then
    wc -l < "$path" | tr -d ' '
  else
    echo 0
  fi
}

initial_jsonl_lines="$(line_count "$jsonl_path")"
initial_log_lines="$(line_count "$log_path")"

echo "Watching OpenTypeless fusion smoke signals for ${duration_secs}s"
echo "JSONL: ${jsonl_path}"
echo "Log:   ${log_path}"
echo
echo "Manual step: focus a real text field, hold the configured Open Typeless Harness hotkey, speak, release, then edit the inserted text within 30s."
echo

while true; do
  now="$(date +%s)"
  if (( now - started_at >= duration_secs )); then
    break
  fi

  echo "== $(date '+%H:%M:%S') edit monitor JSONL =="
  if [[ -f "$jsonl_path" ]] && (( "$(line_count "$jsonl_path")" > initial_jsonl_lines )); then
    tail -n +"$((initial_jsonl_lines + 1))" "$jsonl_path"
  else
    echo "no new JSONL lines yet"
  fi

  echo "== $(date '+%H:%M:%S') Open Typeless Harness runtime log =="
  if [[ -f "$log_path" ]] && (( "$(line_count "$log_path")" > initial_log_lines )); then
    new_log="$(
      tail -n +"$((initial_log_lines + 1))" "$log_path" \
        | grep -E '\[coord\]|\[edit-monitor\]|\[recorder\]|\[mic\]|ASR|Inserted|PasteSent|polish|vih|hotkey' \
        || true
    )"
    if [[ -n "$new_log" ]]; then
      printf '%s\n' "$new_log"
    else
      echo "no new matching log lines yet"
    fi
  else
    echo "no new log lines yet"
  fi

  echo
  sleep 5
done
