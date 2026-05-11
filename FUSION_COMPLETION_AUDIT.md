# OpenTypeless Fusion Completion Audit

## Objective

Build a separate experimental workspace that uses OpenLess as the runtime base, adds an
OpenWhispr-style focused-field edit monitor, and wires OpenTypeless VIH in lightly without
making VIH the main dependency yet.

The first milestone is not full learning promotion. It is proving that the OpenLess runtime and
the OpenWhispr-style monitor can run together in a real input field.

The current learning slice is intentionally thinner than promotion: record a final correction
candidate after the monitor closes, without mutating dictionaries or injecting prompt context.

## Checklist

| Requirement | Evidence | Status |
| --- | --- | --- |
| Create a new folder instead of mutating the original OpenTypeless repo | Workspace exists at `/Users/lichenxin/proj/hub_edison/opentypeless-openless-fusion`; original repo remains separate | Done |
| Use OpenLess as runtime/UI base | `openless-all/app` is the base app; `npm run tauri dev` starts `target/debug/openless` | Done |
| Add OpenWhispr-style focused-field monitor | `openless-all/app/src-tauri/src/edit_monitor.rs` captures target PID, resolves the target focused element, registers `AXObserver`, and keeps polling fallback after insertion | Code done |
| Start monitor after successful insertion | `coordinator.rs` calls `edit_monitor::start_after_insert` for `Inserted` and `PasteSent` | Code done |
| Record monitor evidence locally | monitor writes JSONL to `~/.openless/opentypeless-edit-monitor.jsonl` by default | Code done |
| Diagnose insertion-to-monitor boundary | `coordinator.rs` logs insertion status, target PID, and focus readiness before monitor startup | Done |
| Handle non-trivial AX focus trees | monitor falls back from focused element value to `AXStringForRange`, skips empty `AXValue` before giving up, and scans descendant text values under the focused element/window | Code done |
| Keep VIH lightweight and default off | `vih_bridge.rs` runs only when `OPENTYPELESS_VIH_ENABLED=1` | Done |
| Keep OpenLess polish as fallback | `polish_or_passthrough` falls back to OpenLess polish when VIH is disabled, unavailable, or fails | Done |
| Provide smoke-test instructions | `FUSION_SMOKE_TEST.md` and `scripts/fusion-smoke-watch.sh` document and watch the manual path | Done |
| Provide a clean manual target field | `tools/fusion-edit-monitor-target.html` gives textarea/input/contenteditable fields for browser-based manual validation | Done |
| Provide a one-command manual smoke helper | `scripts/fusion-open-smoke-target.sh` opens the clean target page and starts the watcher | Done |
| Provide monitor-only probe | `src/bin/edit_monitor_probe.rs` and `scripts/fusion-edit-monitor-probe.sh` test the monitor without ASR | Done |
| Keep Tauri dev working after adding the probe binary | `Cargo.toml` sets `default-run = "openless"` | Done |
| Verify code builds | `cargo check -q --bin openless` and `cargo check -q --bin edit_monitor_probe` pass with pre-existing warnings | Done |
| Verify monitor JSONL unit tests | `cargo test -q edit_monitor::macos::tests -- --test-threads=1` passes | Done |
| Verify real voice insertion starts the monitor | Manual left-Control voice input into `Fusion Edit Monitor Target` inserted `这是一次融合测试。` and logged `monitor_start` for target PID 702 | Done |
| Verify user edit is observed as `text_changed` | Manual edit within 30 seconds produced multiple `text_changed` JSONL events, ending with `这是一次整合测试。` | Done |
| Add thin learning candidate recorder | `learning_probe.rs` records final `correction_candidate` JSONL only after a monitor-observed edit closes | Code done |
| Keep learning candidate-only | `learning_probe.rs` does not promote corrections, mutate dictionaries, or inject future prompts | Done |
| Verify learning candidate output | Edge PID 702 monitor-only probe wrote `融 -> 整` to `~/.openless/opentypeless-learning-candidates.jsonl` | Done |

## Prompt-To-Artifact Audit

| User requirement | Artifact / evidence | Coverage |
| --- | --- | --- |
| "再建一个文件夹" | `/Users/lichenxin/proj/hub_edison/opentypeless-openless-fusion` exists and is on branch `fusion/opentypeless-runtime` | Covered |
| "以 OpenLess 为底座" | `openless-all/app` is the runnable Tauri/OpenLess app; `Cargo.toml` keeps `default-run = "openless"` after adding the probe binary | Covered |
| "OpenLess 和 OpenWhispr 拼起来版本先试一下" | `coordinator.rs` captures target PID and calls `edit_monitor::start_after_insert` after `Inserted` / `PasteSent`; manual smoke produced `monitor_start` and `text_changed` | Covered |
| "OpenWhispr 的监控部分" | `edit_monitor.rs` reimplements the macOS PID-scoped focused-field monitor with native AX APIs and JSONL observations; manual Edge textarea edit was observed | Covered |
| "VIH 先轻量地加，不用太重" | `vih_bridge.rs` is default-off and only runs when `OPENTYPELESS_VIH_ENABLED=1`; OpenLess polish remains fallback | Covered |
| "不要一步全部做完" | Only candidate recording is implemented; learning promotion from `text_changed` into correction memory is intentionally not implemented yet | Covered |
| "先试一下可不可以" | Real Edge textarea smoke produced `text_changed` in `~/.openless/opentypeless-edit-monitor.jsonl` | Covered |

## Verification Commands Run

```bash
cargo fmt --check
cargo test -q learning_probe -- --test-threads=1
cargo check -q --bin openless
cargo check -q --bin edit_monitor_probe
cargo test -q edit_monitor::macos::tests -- --test-threads=1
bash -n scripts/fusion-edit-monitor-probe.sh scripts/fusion-smoke-watch.sh
git diff --check
```

All pass, with pre-existing dead-code / unused warnings from the OpenLess codebase.
These checks prove the prototype builds and the JSONL writer works. The manual smoke below proves
the real focused-field edit loop works in Microsoft Edge.

## Manual Smoke Evidence

Manual test completed on 2026-05-08:

- Target: Microsoft Edge, page title `Fusion Edit Monitor Target`, textarea target.
- Runtime log captured front app `Microsoft Edge (com.microsoft.edgemac)` and `target_pid captured: 702`.
- Voice insertion completed with `status=Inserted`, `target_pid=Some(702)`, and `focus_ready_for_paste=true`.
- Edit monitor logged `monitor_start`, then `initial field value chars=9`.
- JSONL recorded the original inserted value `这是一次融合测试。`.
- Manual edit produced multiple `text_changed` records, with the final observed value `这是一次整合测试。`.

The decisive JSONL evidence exists at `~/.openless/opentypeless-edit-monitor.jsonl`:

```json
{"details":{"insertedChars":9},"event":"monitor_start","insertedText":"这是一次融合测试。","originalText":"这是一次融合测试。","targetPid":702,"timestampMs":1778232760968}
{"details":{"fieldChars":9,"fieldValue":"这是一次融合测试。"},"event":"initial_value","insertedText":"这是一次融合测试。","originalText":"这是一次融合测试。","targetPid":702,"timestampMs":1778232761530}
{"event":"text_changed","insertedText":"这是一次融合测试。","newFieldValue":"这是一次整合测试。","originalText":"这是一次融合测试。","targetPid":702,"timestampMs":1778232773479}
```

Earlier automated probe attempts could start the monitor and write `monitor_start` / `initial_value`,
but did not produce `text_changed`. Local desktop automation was not treated as completion evidence.
The completed manual Edge smoke is the acceptance evidence.

The local OpenWhispr reference was checked at:

- `/Users/lichenxin/proj/hub_audio_type/openwhispr/resources/macos-text-monitor.swift`
- `/Users/lichenxin/proj/hub_audio_type/openwhispr/src/helpers/textEditMonitor.js`

OpenWhispr's macOS native monitor is also PID-scoped and reads `AXFocusedUIElement` / `AXValue`;
its native path uses `AXObserver` for change events. This Rust prototype now uses the same
`AXObserver` / `AXValueChanged` pattern, keeps 500ms polling as a fallback inside the observer
loop, and tries `AXStringForRange` when `AXValue` is empty. The real Edge textarea smoke validated
this path end to end.

The expected evidence is a JSONL line in
`~/.openless/opentypeless-edit-monitor.jsonl` with:

```json
{
  "event": "text_changed",
  "originalText": "...",
  "insertedText": "...",
  "newFieldValue": "..."
}
```

## Thin Learning Probe Evidence

Monitor-only validation completed on 2026-05-08:

- Target: Microsoft Edge PID 702, page title `Fusion Edit Monitor Target`, textarea target.
- Probe used `insertedText` / initial value `这是一次融合测试。`.
- The textarea was changed to `这是一次整合测试。`.
- The probe wrote one final `correction_candidate` only after monitor timeout.
- The candidate remained `status=candidate`; no dictionary mutation or prompt injection happened.

The decisive candidate evidence exists at
`~/.openless/opentypeless-learning-candidates.jsonl`:

```json
{"correction":{"confidence":"medium","from":"融","fromChars":1,"kind":"replacement","prefixChars":4,"suffixChars":4,"to":"整","toChars":1},"event":"correction_candidate","finalText":"这是一次整合测试。","insertedText":"这是一次融合测试。","originalText":"这是一次融合测试。","source":"edit_monitor_final","status":"candidate","targetPid":702,"timestampMs":1778243371515}
```

## Manual Gate

1. Focus a real text field.
2. Hold the configured OpenLess hotkey, currently left Control in local preferences.
3. Speak one sentence.
4. Release the hotkey to submit.
5. After text appears, manually edit it within 30 seconds.
6. Check `~/.openless/opentypeless-edit-monitor.jsonl`.

If no runtime log appears, debug hotkey/ASR first.
If insertion happens but no monitor JSONL appears, debug the coordinator-to-monitor hook.
If monitor starts but only writes `timeout`, the target field monitor is alive but did not observe a manual edit.
