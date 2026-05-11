# OpenTypeless + OpenLess Fusion Plan

## Decision

Use this repository as the experimental fusion workspace.

- Base runtime: OpenLess.
- Agent and learning layer: OpenTypeless VIH, optional and off by default for the first trial.
- Edit-feedback layer: OpenWhispr-style target text-field monitoring, reimplemented for this Tauri/Rust codebase.

Do not replace or heavily mutate `/Users/lichenxin/proj/hub_edison/opentypeless` until this workspace proves the runtime experience is better.

## Why This Workspace Exists

The current OpenTypeless repo already has substantial uncommitted VIH, ASR, dictionary, and learning work. Replacing its recorder/hotkey/output stack in-place would mix two risky changes:

- Runtime stabilization.
- Agent/learning behavior.

This workspace isolates runtime migration. If it works, we can either continue here as the new product branch or port the proven runtime modules back.

## Target Architecture

```text
OpenLess hotkey/recorder/ASR/polish/insertion runtime
  -> raw transcript
optional OpenTypeless VIH rewrite/intent/dictionary layer
  -> rewritten text
OpenLess insertion runtime
  -> target app input
OpenWhispr-style edit monitor
  -> observed final text
OpenTypeless learning probe
  -> candidate wrong -> correct record
future promotion layer
  -> correction memory and industry dictionary
```

## First User-Experience Goal

Make voice input feel more product-grade before improving agent intelligence.

Success criteria for the first milestone:

- Hotkey press/release feels reliable.
- Recording starts without losing the first words.
- ASR receives audio continuously while recording.
- Final text lands in the intended input field.
- If paste fails, text stays recoverable in clipboard.
- Timing logs show where delay comes from: recording, ASR finalization, VIH rewrite, insertion.

## What We Will Copy Conceptually From OpenLess

- `coordinator.rs` style session state machine.
- `recorder.rs` style cpal recording and deferred ASR bridge.
- `insertion.rs` style clipboard-first insertion and CGEvent paste on macOS.
- permission and failure reporting flow.
- capsule state transitions tied to runtime states.

## What We Will Keep From OpenTypeless Later

- VIH rewrite pipeline.
- industry dictionary and correction memory.
- `interaction_log`.
- automatic learning from `raw_transcript -> rewritten_text -> final_text`.
- product direction around personalized voice input, not just generic dictation.

## What We Will Borrow From OpenWhispr

- Capture target app PID before showing overlay.
- After insertion, monitor the target focused text field for a short window.
- Learn from the actual final field value instead of only keyboard events.

OpenWhispr code is Electron/Node-oriented, so the implementation should be rewritten in this workspace rather than pasted directly. The current macOS prototype uses native Accessibility FFI (`AXUIElementCreateApplication`, `AXFocusedUIElement`, `AXValue`) instead of polling through `osascript`. It first asks the captured target app for its focused element, then falls back to the system-wide focused element and verifies the element PID still matches the captured target PID.

OpenWhispr's native macOS helper uses `AXObserver` for value-change notifications after resolving
the same PID-scoped focused element. This prototype now mirrors that shape: it registers
`AXValueChanged` through `AXObserver`, and still keeps a 500ms polling fallback inside the observer
loop for controls that do not emit the notification reliably. It also falls back to
`AXNumberOfCharacters` + `AXStringForRange` for controls that expose parameterized text rather than
plain `AXValue`. Real target-field validation is still required before promoting the observed edits
into learning memory.

## First Implementation Slice

1. Keep OpenLess running unchanged.
2. Add OpenWhispr-style edit monitoring after OpenLess insertion.
3. Emit/log `originalText`, `insertedText`, `newFieldValue`, and target PID when the field changes.
4. Keep OpenLess polish as the default behavior.
5. Add a VIH bridge that is disabled by default and only runs when `OPENTYPELESS_VIH_ENABLED=1`.
6. Add a thin learning probe that records final correction candidates only.

Only after this works should we add learning promotion from monitor output. The current probe does
not mutate dictionaries, inject prompt context, or auto-promote memory.

## Current Trial Flags

- `OPENTYPELESS_EDIT_MONITOR_ENABLED=0` disables the post-insertion monitor.
- Edit monitor observations are appended locally to `~/.openless/opentypeless-edit-monitor.jsonl`.
- `OPENTYPELESS_EDIT_MONITOR_JSONL=0` disables that local JSONL observation log.
- `OPENTYPELESS_EDIT_MONITOR_JSONL_PATH=/path/to/file.jsonl` overrides the JSONL log path.
- Learning candidates are appended locally to
  `~/.openless/opentypeless-learning-candidates.jsonl`.
- `OPENTYPELESS_LEARNING_CANDIDATES=0` disables candidate recording.
- `OPENTYPELESS_LEARNING_CANDIDATES_PATH=/path/to/file.jsonl` overrides the candidate log path.
- `OPENTYPELESS_VIH_ENABLED=1` enables optional VIH rewrite before insertion.
- `OPENTYPELESS_VIH_ROOT=/path/to/vih` overrides the default VIH path.

## Current Source Repositories

- OpenLess source: `/Users/lichenxin/proj/hub_audio_type/openless`
- OpenTypeless source: `/Users/lichenxin/proj/hub_edison/opentypeless`
- OpenWhispr reference: `/Users/lichenxin/proj/hub_audio_type/openwhispr`
