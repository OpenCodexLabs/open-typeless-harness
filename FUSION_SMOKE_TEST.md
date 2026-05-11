# OpenTypeless Fusion Smoke Test

Use this checklist to verify the OpenLess runtime + OpenWhispr-style edit monitor prototype.

## Start

```bash
cd /Users/lichenxin/proj/hub_edison/opentypeless-openless-fusion/openless-all/app
npm run tauri dev
```

The prototype should log:

- `Accessibility status = Granted`
- `CGEventTap 已启动`
- `hotkey listener installed`
- `AVAudioApplication.recordPermission ... Granted`

## Voice Input Path

1. Open TextEdit, WeChat, Notion, a browser input box, or any normal text field.
   If you just need a clean target, open `tools/fusion-edit-monitor-target.html` in a browser.
2. Put the cursor in the target field.
3. Press the OpenLess dictation hotkey.
   - macOS default: right Option.
   - Current local preference is `hold`: hold right Option while speaking, then release to submit.
   - If you switch to `toggle`, press once to start and once again to submit.
4. Speak one short sentence.
5. Confirm text is inserted into the target field.

## Edit Monitor Path

The macOS monitor reads the target app through native Accessibility APIs, not `osascript`.
It tries app-scoped `AXFocusedUIElement` first, then system-wide focused element as a fallback, while rejecting elements whose PID does not match the captured target app.
If the focused element itself has an empty value, it also scans a bounded set of descendant
nodes under the focused element / focused window for a non-empty text value.
After resolving the field, it registers `AXObserver` / `AXValueChanged` and also polls every 500ms
inside that observer loop as a fallback. For controls that expose text through parameterized AX
attributes instead of plain `AXValue`, it also tries `AXNumberOfCharacters` + `AXStringForRange`.

Optional watcher:

```bash
cd /Users/lichenxin/proj/hub_edison/opentypeless-openless-fusion
./scripts/fusion-smoke-watch.sh 120
```

Open the clean browser target and start the watcher in one step:

```bash
cd /Users/lichenxin/proj/hub_edison/opentypeless-openless-fusion
./scripts/fusion-open-smoke-target.sh 300
```

Monitor-only probe, useful when you want to test the OpenWhispr-style focused-field monitor
without ASR:

```bash
cd /Users/lichenxin/proj/hub_edison/opentypeless-openless-fusion
./scripts/fusion-edit-monitor-probe.sh
```

Run it while a real text field is focused, then edit that field within 30 seconds.
This uses the same native Accessibility monitor as the post-insertion path and writes to the
same JSONL file, but it does not prove the ASR/insertion chain.

If you need time to switch from terminal to the target field:

```bash
./scripts/fusion-edit-monitor-probe.sh --wait 5
```

If the probe is launched from a non-GUI shell and captures `loginwindow` instead of the real
target app, pass a PID explicitly:

```bash
TEXTEDIT_PID="$(pgrep -x TextEdit | head -n 1)"
./scripts/fusion-edit-monitor-probe.sh --pid "$TEXTEDIT_PID" "probe raw text" "probe inserted text"
```

After the text is inserted:

1. Stay in the same target field.
2. Edit the inserted text manually within 30 seconds.
3. Check whether this file gets a new JSONL line:

```bash
tail -n 5 ~/.openless/opentypeless-edit-monitor.jsonl
```

Expected payload shape:

```json
{
  "event": "text_changed",
  "timestampMs": 1770000000000,
  "targetPid": 1234,
  "originalText": "raw ASR transcript",
  "insertedText": "polished inserted text",
  "newFieldValue": "final user-edited field value"
}
```

## Result Interpretation

- If text inserts and JSONL appears, the first OpenLess + OpenWhispr monitor fusion is working.
- If the runtime log has `insertion completed status=... target_pid=...`, the insert-to-monitor boundary has enough data to debug the next hop.
- If JSONL only has `monitor_start` / `initial_value` / `timeout`, the monitor started but did not see a manual edit.
- If JSONL has `initial_query_failed` or `target_field_disappeared`, the runtime is OK but the focused-field monitor needs fixing for that app.
- If JSONL has `monitor_skipped`, the insertion path ran but the monitor did not have enough target metadata to start.
- If text inserts but no JSONL appears at all, the monitor was not started after insertion.
- If text does not insert, debug OpenLess insertion/focus first before touching VIH.
- If voice recognition or polish is slow, keep VIH disabled and profile ASR/LLM latency first.

## Current Flags

- `OPENTYPELESS_EDIT_MONITOR_ENABLED=0` disables post-insertion monitoring.
- `OPENTYPELESS_EDIT_MONITOR_JSONL=0` disables local JSONL observation logging.
- `OPENTYPELESS_EDIT_MONITOR_JSONL_PATH=/path/to/file.jsonl` overrides the JSONL path.
- `OPENTYPELESS_VIH_ENABLED=1` enables optional VIH rewrite before OpenLess polish fallback.
- `OPENTYPELESS_VIH_ROOT=/path/to/vih` overrides the default VIH root.
